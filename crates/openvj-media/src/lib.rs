//! Media loading for openVJ.
//!
//! Most video is decoded by an `ffmpeg` child process piping raw RGBA frames, which keeps
//! openVJ free of FFmpeg link-time dependencies. Resolume DXV and Vidvox HAP movies skip
//! FFmpeg: the [`dxv`] and [`hap`] modules read their frames straight from the file and
//! hand block-compressed textures to the GPU.
//! Cameras and Syphon are in [`live`].

use std::collections::VecDeque;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use anyhow::{Context, Result, anyhow, bail};
use image::imageops::FilterType;
use image::{DynamicImage, RgbaImage};

pub mod dxv;
pub mod hap;
pub mod live;
pub mod mov;
mod player;

/// An RGBA8 frame in CPU memory.
#[derive(Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Frame({}x{})", self.width, self.height)
    }
}

impl Frame {
    fn from_image(img: RgbaImage) -> Self {
        Self { width: img.width(), height: img.height(), rgba: img.into_raw() }
    }

    fn to_image(&self) -> RgbaImage {
        RgbaImage::from_raw(self.width, self.height, self.rgba.clone()).expect("frame size matches buffer")
    }

    /// Scales and centre-crops to exactly `w`×`h`.
    pub fn thumbnail(&self, w: u32, h: u32) -> Frame {
        let img = DynamicImage::ImageRgba8(self.to_image()).resize_to_fill(w, h, FilterType::Triangle);
        Frame::from_image(img.into_rgba8())
    }
}

/// Basic stream information from `ffprobe`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration: Option<f64>,
    /// Frame count, when the container records it.
    pub frames: Option<u64>,
    /// FFmpeg decoder to force instead of FFmpeg's default, e.g. `libvpx-vp9` so the alpha
    /// channel of a transparent WebM isn't dropped.
    pub decoder: Option<&'static str>,
}

impl VideoInfo {
    /// Number of frames in one pass through the file, if it can be told.
    pub fn frame_count(&self) -> Option<i64> {
        self.frames
            .map(|n| n as i64)
            .or_else(|| self.duration.map(|d| (d * self.fps).round() as i64))
            .filter(|n| *n > 0)
    }
}

/// Locates an FFmpeg tool (`ffmpeg` or `ffprobe`).
///
/// `OPENVJ_FFMPEG_DIR` overrides everything. Next comes openVJ's own folder, where
/// OpenVJ.app ships its FFmpeg (Contents/MacOS). Apps started from the macOS Finder don't
/// inherit the shell `PATH`, so the usual Homebrew locations are checked too.
pub fn tool_path(tool: &str) -> PathBuf {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(dir) = std::env::var_os("OPENVJ_FFMPEG_DIR") {
        dirs.push(dir.into());
    }
    if let Some(dir) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(PathBuf::from)) {
        dirs.push(dir);
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].map(PathBuf::from));
    let exe = if cfg!(windows) { format!("{tool}.exe") } else { tool.to_owned() };
    dirs.into_iter().map(|d| d.join(&exe)).find(|p| p.is_file()).unwrap_or_else(|| exe.into())
}

/// True if `ffmpeg` and `ffprobe` can be run.
pub fn ffmpeg_available() -> bool {
    ["ffmpeg", "ffprobe"].iter().all(|t| {
        Command::new(tool_path(t))
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    })
}

fn parse_rate(s: &str) -> Option<f64> {
    let (n, d) = s.split_once('/').unwrap_or((s, "1"));
    let (n, d): (f64, f64) = (n.trim().parse().ok()?, d.trim().parse().ok()?);
    (n > 0.0 && d > 0.0).then_some(n / d)
}

pub fn probe(path: &Path) -> Result<VideoInfo> {
    let out = Command::new(tool_path("ffprobe"))
        .args(["-v", "error", "-select_streams", "v:0"])
        .args(["-show_entries", "stream=codec_name,width,height,avg_frame_rate,r_frame_rate,nb_frames:stream_tags=alpha_mode:format=duration"])
        .args(["-of", "default=nw=1"])
        .arg(path)
        .output()
        .context("could not run ffprobe (is FFmpeg installed?)")?;
    if !out.status.success() {
        bail!("ffprobe failed: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let (mut width, mut height, mut avg, mut real, mut duration, mut frames) = (0, 0, None, None, None, None);
    let (mut codec, mut alpha) = (String::new(), false);
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else { continue };
        match k {
            "width" => width = v.parse().unwrap_or(0),
            "height" => height = v.parse().unwrap_or(0),
            "avg_frame_rate" => avg = parse_rate(v),
            "r_frame_rate" => real = parse_rate(v),
            "duration" => duration = v.parse().ok(),
            "nb_frames" => frames = v.parse().ok().filter(|n| *n > 0),
            "codec_name" => codec = v.to_owned(),
            "TAG:alpha_mode" => alpha = v.trim() == "1",
            _ => {}
        }
    }
    if width == 0 || height == 0 {
        bail!("no video stream in {}", path.display());
    }
    let fps = avg.or(real).filter(|f| (1.0..=240.0).contains(f)).unwrap_or(30.0);
    Ok(VideoInfo { width, height, fps, duration, frames, decoder: alpha_decoder(&codec, alpha) })
}

/// FFmpeg's own VP8/VP9 decoders ignore the alpha channel WebM stores alongside the
/// picture, so a transparent WebM would play opaque. libvpx's decoders keep it.
fn alpha_decoder(codec: &str, alpha: bool) -> Option<&'static str> {
    let decoder = match (codec, alpha) {
        ("vp9", true) => "libvpx-vp9",
        ("vp8", true) => "libvpx",
        _ => return None,
    };
    has_decoder(decoder).then_some(decoder)
}

/// True if this FFmpeg build has the decoder `name` (checked once per name).
fn has_decoder(name: &'static str) -> bool {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static KNOWN: OnceLock<Mutex<HashMap<&'static str, bool>>> = OnceLock::new();
    let mut known = KNOWN.get_or_init(Default::default).lock().unwrap();
    *known.entry(name).or_insert_with(|| {
        Command::new(tool_path("ffmpeg"))
            .args(["-hide_banner", "-h", &format!("decoder={name}")])
            .output()
            .is_ok_and(|o| !String::from_utf8_lossy(&o.stdout).contains("not recognized"))
    })
}

/// Largest even size with the same aspect ratio that fits in `max_w`×`max_h` (never upscales).
pub fn fit_within(w: u32, h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    let scale = (max_w as f64 / w as f64).min(max_h as f64 / h as f64).min(1.0);
    let even = |v: f64| ((v.round() as u32) & !1).max(2);
    (even(w as f64 * scale), even(h as f64 * scale))
}

/// `ffmpeg` with flags shared by every invocation. Rotation metadata is ignored so the
/// decoded size always matches what `ffprobe` reported.
fn ffmpeg_command() -> Command {
    let mut cmd = Command::new(tool_path("ffmpeg"));
    cmd.args(["-nostdin", "-loglevel", "error", "-noautorotate"]);
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    cmd
}

fn raw_rgba_output(cmd: &mut Command, w: u32, h: u32) {
    cmd.args(["-an", "-vf", &format!("scale={w}:{h}:flags=bilinear")]);
    cmd.args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"]);
}

/// Forces `decoder` for the next input, if there is one. Otherwise, on macOS, asks for
/// VideoToolbox so H.264, HEVC and ProRes decode on the media engine instead of the CPU.
/// FFmpeg falls back to software for codecs VideoToolbox can't decode.
/// `OPENVJ_NO_HWACCEL=1` turns this off, for comparing or working around a driver bug.
fn input_decoder(cmd: &mut Command, decoder: Option<&str>) {
    if let Some(d) = decoder {
        cmd.args(["-c:v", d]);
    } else if cfg!(target_os = "macos") && std::env::var_os("OPENVJ_NO_HWACCEL").is_none() {
        cmd.args(["-hwaccel", "videotoolbox"]);
    }
}

/// Grabs one frame at `seconds`, scaled to `w`×`h`, decoded with `decoder` if one is given
/// (see [`VideoInfo::decoder`]).
pub fn grab_frame(path: &Path, seconds: f64, w: u32, h: u32, decoder: Option<&str>) -> Result<Frame> {
    let mut cmd = ffmpeg_command();
    cmd.args(["-ss", &format!("{seconds:.3}")]);
    input_decoder(&mut cmd, decoder);
    cmd.arg("-i").arg(path).args(["-frames:v", "1"]);
    raw_rgba_output(&mut cmd, w, h);
    let out = cmd.output().context("could not run ffmpeg (is FFmpeg installed?)")?;
    let size = (w * h * 4) as usize;
    if out.stdout.len() < size {
        bail!("ffmpeg returned no frame for {}", path.display());
    }
    Ok(Frame { width: w, height: h, rgba: out.stdout[..size].to_vec() })
}

/// A media file prepared for the clip grid: a full-size first frame plus a thumbnail.
#[derive(Debug)]
pub struct LoadedMedia {
    pub frame: Frame,
    pub thumb: Frame,
    pub video: Option<VideoInfo>,
    /// Codec name for codecs openVJ decodes itself (e.g. "DXV HQ").
    pub codec: Option<&'static str>,
}

pub fn load_image(path: &Path, max_w: u32, max_h: u32) -> Result<Frame> {
    let img = image::open(path).with_context(|| format!("could not open {}", path.display()))?;
    let (w, h) = fit_within(img.width(), img.height(), max_w, max_h);
    let img = if (w, h) == (img.width(), img.height()) { img } else { img.resize_exact(w, h, FilterType::Triangle) };
    Ok(Frame::from_image(img.into_rgba8()))
}

/// Writes RGBA8 pixels as a PNG.
pub fn save_png(path: &Path, width: u32, height: u32, rgba: Vec<u8>) -> Result<()> {
    let img = RgbaImage::from_raw(width, height, rgba).ok_or_else(|| anyhow!("pixel buffer size mismatch"))?;
    img.save(path).with_context(|| format!("could not write {}", path.display()))
}

/// Loads an image or the first frame of a video, plus a `thumb.0`×`thumb.1` thumbnail.
pub fn load_media(path: &Path, is_video: bool, max: (u32, u32), thumb: (u32, u32)) -> Result<LoadedMedia> {
    let (frame, video) = if is_video {
        let info = probe(path)?;
        let (w, h) = fit_within(info.width, info.height, max.0, max.1);
        // Some files have no decodable frame at exactly 0; fall back to a little later.
        let frame = grab_frame(path, 0.0, w, h, info.decoder).or_else(|_| grab_frame(path, 0.5, w, h, info.decoder))?;
        (frame, Some(info))
    } else {
        (load_image(path, max.0, max.1)?, None)
    };
    let thumb = frame.thumbnail(thumb.0, thumb.1);
    let codec = if is_video {
        dxv::probe(path).map(dxv::DxvFormat::name).or_else(|| hap::probe(path).map(hap::HapFormat::name))
    } else {
        None
    };
    Ok(LoadedMedia { frame, thumb, video, codec })
}

/// Plays RGBA frames from a video file at any position, forwards or backwards.
///
/// An `ffmpeg` process streams frames (looping) from where playback is, and a reader thread
/// keeps a few of them buffered. [`VideoDecoder::frame_at`] takes them as clip time moves
/// on, dropping frames when playback runs faster than decoding so the clip never drifts.
/// Recent frames are cached, so bouncing back over them costs nothing. A jump, or playing
/// backwards past the cache, restarts the stream just before the frame that's needed.
pub struct VideoDecoder {
    pub info: VideoInfo,
    pub width: u32,
    pub height: u32,
    path: PathBuf,
    /// Frames in one pass through the file, if known.
    count: Option<i64>,
    stream: Option<Stream>,
    /// Recently decoded frames by index, oldest first.
    cache: VecDeque<(i64, Vec<u8>)>,
    cache_cap: usize,
    /// Index of the last frame handed out (-1 before the first).
    shown: i64,
}

struct Stream {
    frames: Receiver<Vec<u8>>,
    /// Buffers the cache no longer needs go back to the reader thread, so playback doesn't
    /// allocate (and page in) a fresh full-size frame for every frame.
    recycle: Sender<Vec<u8>>,
    child: Child,
    /// Frame index (within the file) of the next frame the stream will deliver.
    next: i64,
}

impl Drop for Stream {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Memory for a decoder's frame cache. Playing backwards decodes this much at a time.
const CACHE_BYTES: usize = 96 << 20;

impl VideoDecoder {
    pub fn open(path: &Path, max_w: u32, max_h: u32) -> Result<Self> {
        Self::open_with_info(path, probe(path)?, max_w, max_h)
    }

    /// Like [`VideoDecoder::open`] but skips probing when the stream info is already known.
    pub fn open_with_info(path: &Path, info: VideoInfo, max_w: u32, max_h: u32) -> Result<Self> {
        let (width, height) = fit_within(info.width, info.height, max_w, max_h);
        let frame_size = (width * height * 4) as usize;
        let mut dec = Self {
            info,
            width,
            height,
            path: path.to_owned(),
            count: info.frame_count(),
            stream: None,
            cache: VecDeque::new(),
            cache_cap: (CACHE_BYTES / frame_size).clamp(4, 240),
            shown: -1,
        };
        dec.restart(0)?;
        Ok(dec)
    }

    /// Starts streaming from frame `index`.
    fn restart(&mut self, index: i64) -> Result<()> {
        self.stream = None; // Stops the old process first.
        let mut cmd = ffmpeg_command();
        if index > 0 {
            // Before -i: a fast keyframe seek, then decoding up to the exact frame.
            cmd.args(["-ss", &format!("{:.6}", index as f64 / self.info.fps)]);
        }
        cmd.args(["-stream_loop", "-1"]);
        input_decoder(&mut cmd, self.info.decoder);
        cmd.arg("-i").arg(&self.path);
        raw_rgba_output(&mut cmd, self.width, self.height);
        let mut child = cmd.spawn().context("could not run ffmpeg (is FFmpeg installed?)")?;
        let mut stdout = child.stdout.take().ok_or_else(|| anyhow!("ffmpeg has no stdout"))?;
        let frame_size = (self.width * self.height * 4) as usize;
        let (tx, frames) = mpsc::sync_channel::<Vec<u8>>(3);
        let (recycle, spare) = mpsc::channel::<Vec<u8>>();
        thread::Builder::new().name("openvj-video".into()).spawn(move || {
            loop {
                let mut buf = spare.try_recv().unwrap_or_else(|_| vec![0u8; frame_size]);
                if stdout.read_exact(&mut buf).is_err() || tx.send(buf).is_err() {
                    break;
                }
            }
        })?;
        self.stream = Some(Stream { frames, recycle, child, next: index });
        Ok(())
    }

    /// Wraps a frame index into the file when its length is known.
    fn wrap(&self, index: i64) -> i64 {
        match self.count {
            Some(n) => index.rem_euclid(n),
            None => index.max(0),
        }
    }

    /// How many frames the stream must deliver to reach `target`, or `None` if it's behind.
    fn ahead(&self, next: i64, target: i64) -> Option<i64> {
        let d = match self.count {
            Some(n) => (target - next).rem_euclid(n),
            None => target - next,
        };
        (d >= 0).then_some(d)
    }

    /// Returns the frame for clip time `t` (seconds into the file) if it differs from the
    /// last one returned. Never blocks: while a frame is still decoding, nothing is returned
    /// and the last frame stays up. The frame is borrowed from the cache, so it isn't copied.
    pub fn frame_at(&mut self, t: f64) -> Option<&[u8]> {
        let mut target = self.wrap((t.max(0.0) * self.info.fps).floor() as i64);
        if let Some(n) = self.count {
            target = target.min(n - 1);
        }
        if target == self.shown {
            return None;
        }
        if let Some(i) = self.cache.iter().position(|(idx, _)| *idx == target) {
            self.shown = target;
            return Some(&self.cache[i].1);
        }
        // Keep streaming if the target is a little way ahead; otherwise seek.
        let max_skip = (self.info.fps * 2.0) as i64;
        let reachable = self.stream.as_ref().and_then(|s| self.ahead(s.next, target)).is_some_and(|d| d <= max_skip);
        if !reachable {
            let backwards = self.shown >= 0 && self.ahead(target, self.shown).is_some_and(|d| d < max_skip);
            // Going backwards, start a cache's worth earlier so the next frames are ready.
            let start = if backwards { (target - self.cache_cap as i64 + 1).max(0) } else { target };
            if self.restart(start).is_err() {
                return None;
            }
        }
        self.pull(target)
    }

    /// Takes decoded frames from the stream up to `target` and returns the newest.
    fn pull(&mut self, target: i64) -> Option<&[u8]> {
        let mut newest = None;
        while let Some(next) = self.stream.as_ref().map(|s| s.next) {
            if self.ahead(next, target).is_none() || next == self.wrap(target + 1) {
                break;
            }
            let following = self.wrap(next + 1);
            let stream = self.stream.as_mut().expect("stream");
            let Ok(frame) = stream.frames.try_recv() else { break };
            stream.next = following;
            if self.cache.len() >= self.cache_cap
                && let Some((_, old)) = self.cache.pop_front()
            {
                let _ = stream.recycle.send(old);
            }
            self.cache.push_back((next, frame));
            newest = Some(next);
            if next == target {
                break;
            }
        }
        let index = newest?;
        // Only show frames at or just before the target, never ones from earlier in a seek.
        if self.ahead(index, target).is_some_and(|d| d > (self.info.fps / 4.0) as i64) && self.shown >= 0 {
            return None;
        }
        self.shown = index;
        self.cache.back().map(|(_, f)| f.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_keeps_aspect_and_even_sizes() {
        assert_eq!(fit_within(3840, 2160, 1280, 720), (1280, 720));
        assert_eq!(fit_within(1080, 1920, 1280, 720), (404, 720));
        assert_eq!(fit_within(640, 480, 1280, 720), (640, 480));
        assert_eq!(fit_within(641, 481, 1280, 720), (640, 480));
    }

    #[test]
    fn parses_frame_rates() {
        assert_eq!(parse_rate("30000/1001").map(|f| (f * 100.0).round()), Some(2997.0));
        assert_eq!(parse_rate("25"), Some(25.0));
        assert_eq!(parse_rate("0/0"), None);
    }

    #[test]
    fn thumbnail_has_exact_size() {
        let f = Frame { width: 100, height: 50, rgba: vec![255; 100 * 50 * 4] };
        let t = f.thumbnail(32, 32);
        assert_eq!((t.width, t.height, t.rgba.len()), (32, 32, 32 * 32 * 4));
    }

    /// Needs `ffmpeg` with libvpx on PATH; generates a fully transparent VP9 WebM.
    #[test]
    fn webm_keeps_alpha() {
        if !ffmpeg_available() || !has_decoder("libvpx-vp9") {
            eprintln!("skipping: ffmpeg or libvpx not installed");
            return;
        }
        let dir = std::env::temp_dir().join(format!("openvj-alpha-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("clear.webm");
        let ok = Command::new(tool_path("ffmpeg"))
            .args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "color=c=red@0.0:s=64x64:d=1,format=yuva420p"])
            .args(["-c:v", "libvpx-vp9"])
            .arg(&path)
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            eprintln!("skipping: this ffmpeg can't encode VP9");
            return;
        }
        let media = load_media(&path, true, (64, 64), (16, 16)).unwrap();
        assert_eq!(media.video.unwrap().decoder, Some("libvpx-vp9"));
        assert!(media.frame.rgba.chunks(4).all(|p| p[3] == 0), "transparent pixels stay transparent");

        let mut dec = VideoDecoder::open(&path, 64, 64).unwrap();
        let mut got = None;
        for _ in 0..200 {
            got = got.or(dec.frame_at(0.0).map(<[u8]>::to_vec));
            if got.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(got.unwrap().chunks(4).all(|p| p[3] == 0), "playback keeps the alpha too");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Needs `ffmpeg` on PATH; generates a short test clip.
    #[test]
    fn decodes_generated_video() {
        if !ffmpeg_available() {
            eprintln!("skipping: ffmpeg not installed");
            return;
        }
        let dir = std::env::temp_dir().join(format!("openvj-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.mp4");
        let ok = Command::new(tool_path("ffmpeg"))
            .args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc=size=320x240:rate=25:duration=1"])
            .args(["-pix_fmt", "yuv420p"])
            .arg(&path)
            .status()
            .unwrap()
            .success();
        assert!(ok);
        let media = load_media(&path, true, (160, 160), (48, 27)).unwrap();
        assert_eq!((media.frame.width, media.frame.height), (160, 120));
        assert_eq!(media.video.unwrap().fps, 25.0);

        let mut dec = VideoDecoder::open(&path, 160, 160).unwrap();
        let mut got = None;
        for _ in 0..200 {
            got = got.or(dec.frame_at(0.1).map(<[u8]>::to_vec));
            if got.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(got.unwrap().len(), 160 * 120 * 4);
        assert!(dec.frame_at(0.1).is_none(), "the same frame isn't handed out twice");
        assert!(dec.frame_at(0.0).is_some(), "going back to a decoded frame comes from the cache");

        // Jumping ahead and back again seeks.
        let wait_for = |dec: &mut VideoDecoder, t: f64| {
            for _ in 0..300 {
                if dec.frame_at(t).is_some() {
                    return true;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            false
        };
        assert_eq!(dec.info.frame_count(), Some(25));
        assert!(wait_for(&mut dec, 0.9));
        assert!(wait_for(&mut dec, 0.5), "plays backwards");
        assert!(wait_for(&mut dec, 1.2), "wraps around the end");
        drop(dec);
        let _ = std::fs::remove_dir_all(dir);
    }
}
