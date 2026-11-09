//! Live video inputs: cameras and capture devices, and Syphon servers (macOS).
//!
//! Each open input runs on its own thread and keeps only the newest frame, so a slow
//! consumer never builds up latency. The camera code follows Varda's approach
//! (nokhwa for capture, yuvutils-rs for fast NV12/YUYV conversion). Syphon uses the
//! `syphon-core` crate, which ships Syphon.framework.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::Frame;

/// Where a live input comes from. Names are what the OS or Syphon reports, so a saved
/// composition finds the same device on another machine.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LiveSource {
    /// A camera, found by `id` (the OS's unique id) or else by `name`.
    Camera { name: String, id: String },
    /// A Syphon server, found by its app name and server name.
    Syphon { app: String, server: String },
}

/// A camera the OS reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraDevice {
    pub name: String,
    /// AVFoundation `uniqueID` on macOS, the device path on Linux.
    pub id: String,
}

/// A running Syphon server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyphonServer {
    pub app: String,
    pub server: String,
}

/// State of an open input.
#[derive(Debug, Clone, PartialEq)]
pub enum LiveStatus {
    Connecting,
    /// Frames are arriving; `detail` describes the stream, e.g. "1280×720 · 30 fps".
    Running {
        detail: String,
    },
    Failed(String),
}

/// True when this build can capture from cameras.
pub const CAMERA_SUPPORTED: bool = cfg!(feature = "camera");
/// True when this build can receive Syphon (macOS with the `syphon` feature).
pub const SYPHON_SUPPORTED: bool = cfg!(all(feature = "syphon", target_os = "macos"));

/// Picks the device a saved camera source refers to: the same unique id if it is still
/// there, otherwise the first device with the same name. USB cameras on macOS get a new id
/// when moved to another port, so the name fallback is what usually finds them.
pub fn find_camera(devices: &[CameraDevice], name: &str, id: &str) -> Option<usize> {
    (!id.is_empty())
        .then(|| devices.iter().position(|d| d.id == id))
        .flatten()
        .or_else(|| devices.iter().position(|d| d.name.eq_ignore_ascii_case(name)))
}

/// Picks the server a saved Syphon source refers to: the exact app and server name, or
/// failing that the first server from the same app (apps rename their servers between
/// versions, Synesthesia included).
pub fn find_syphon(servers: &[SyphonServer], app: &str, server: &str) -> Option<usize> {
    servers
        .iter()
        .position(|s| s.app == app && s.server == server)
        .or_else(|| servers.iter().position(|s| s.app.eq_ignore_ascii_case(app)))
}

/// Cameras connected now. Empty when camera support is off or none are found.
pub fn list_cameras() -> Vec<CameraDevice> {
    #[cfg(feature = "camera")]
    {
        camera::list()
    }
    #[cfg(not(feature = "camera"))]
    {
        Vec::new()
    }
}

/// Syphon servers running now. The directory fills in from notifications on the main run
/// loop, so call this from the UI thread, and expect the first call after launch to be empty.
pub fn list_syphon_servers() -> Vec<SyphonServer> {
    #[cfg(all(feature = "syphon", target_os = "macos"))]
    {
        syphon::list()
    }
    #[cfg(not(all(feature = "syphon", target_os = "macos")))]
    {
        Vec::new()
    }
}

/// What an input thread shares with its [`LiveStream`].
#[derive(Default)]
struct Shared {
    frame: Mutex<Option<Frame>>,
    status: Mutex<Option<LiveStatus>>,
    stop: AtomicBool,
}

#[cfg_attr(not(any(feature = "camera", all(feature = "syphon", target_os = "macos"))), allow(dead_code))]
impl Shared {
    fn set_status(&self, status: LiveStatus) {
        *self.status.lock().unwrap() = Some(status);
    }

    fn publish(&self, frame: Frame) {
        *self.frame.lock().unwrap() = Some(frame);
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

/// An open live input. Dropping it stops the input.
pub struct LiveStream {
    shared: Arc<Shared>,
}

impl LiveStream {
    /// Starts connecting to `source`. Opening happens in the background; watch
    /// [`LiveStream::status`]. Frames larger than `max_w`×`max_h` are avoided where the
    /// device offers a choice.
    pub fn open(source: &LiveSource, max_w: u32, max_h: u32) -> Self {
        let shared = Arc::new(Shared::default());
        shared.set_status(LiveStatus::Connecting);
        let started = match source {
            LiveSource::Camera { name, id } => open_camera(&shared, name, id, (max_w, max_h)),
            LiveSource::Syphon { app, server } => open_syphon(&shared, app, server),
        };
        if let Err(e) = started {
            shared.set_status(LiveStatus::Failed(e));
        }
        Self { shared }
    }

    /// The newest frame since the last call, if any.
    pub fn take_frame(&self) -> Option<Frame> {
        self.shared.frame.lock().unwrap().take()
    }

    pub fn status(&self) -> LiveStatus {
        self.shared.status.lock().unwrap().clone().unwrap_or(LiveStatus::Connecting)
    }
}

impl Drop for LiveStream {
    fn drop(&mut self) {
        // The thread sees this within a frame and exits. It isn't joined: a camera that was
        // unplugged can block its read indefinitely, and that must not freeze the UI.
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}

type Spawned = Result<(), String>;

#[cfg(feature = "camera")]
fn open_camera(shared: &Arc<Shared>, name: &str, id: &str, max: (u32, u32)) -> Spawned {
    let (shared, name, id) = (shared.clone(), name.to_owned(), id.to_owned());
    std::thread::Builder::new()
        .name("openvj-camera".into())
        .spawn(move || camera::run(&shared, &name, &id, max))
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(not(feature = "camera"))]
fn open_camera(_: &Arc<Shared>, _: &str, _: &str, _: (u32, u32)) -> Spawned {
    Err("this build has no camera support".into())
}

#[cfg(all(feature = "syphon", target_os = "macos"))]
fn open_syphon(shared: &Arc<Shared>, app: &str, server: &str) -> Spawned {
    syphon::open(shared, app, server)
}

#[cfg(not(all(feature = "syphon", target_os = "macos")))]
fn open_syphon(_: &Arc<Shared>, _: &str, _: &str) -> Spawned {
    Err(if cfg!(target_os = "macos") {
        "this build has no Syphon support".into()
    } else {
        "Syphon is only available on macOS".into()
    })
}

/// Copies `height` rows of BGRA pixels (rows `stride` bytes apart) into tightly packed RGBA.
/// With `bottom_up`, the source's first row is the bottom of the picture (OpenGL's
/// convention, which Syphon textures follow), so rows are written in reverse.
pub fn bgra_to_rgba(src: &[u8], width: usize, height: usize, stride: usize, bottom_up: bool) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 4];
    for (i, row) in src.chunks(stride).take(height).enumerate() {
        let y = if bottom_up { height - 1 - i } else { i };
        let dst = &mut out[y * width * 4..(y + 1) * width * 4];
        for (d, px) in dst.as_chunks_mut::<4>().0.iter_mut().zip(row[..width * 4].as_chunks::<4>().0) {
            *d = [px[2], px[1], px[0], px[3]];
        }
    }
    out
}

/// Camera format choice, kept free of nokhwa types so it can be tested anywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatOption {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// Compressed formats (MJPEG) cost a JPEG decode per frame.
    pub compressed: bool,
}

/// Index of the best capture format: the largest that fits `max_w`×`max_h` at 24 fps or
/// more, preferring higher frame rates and uncompressed formats among equals. Falls back to
/// the smallest smooth format, then to the fastest of all.
pub fn pick_format(options: &[FormatOption], max_w: u32, max_h: u32) -> Option<usize> {
    let smooth = |o: &FormatOption| o.fps >= 24;
    let fits = |o: &FormatOption| o.width <= max_w && o.height <= max_h;
    let rank = |o: &FormatOption| (o.width as u64 * o.height as u64, o.fps.min(60), !o.compressed);
    let indexed = || options.iter().enumerate();
    indexed()
        .filter(|(_, o)| smooth(o) && fits(o))
        .max_by_key(|(_, o)| rank(o))
        .or_else(|| {
            indexed().filter(|(_, o)| smooth(o)).min_by_key(|(_, o)| (o.width as u64 * o.height as u64, !o.compressed))
        })
        .or_else(|| indexed().max_by_key(|(_, o)| (o.fps, rank(o))))
        .map(|(i, _)| i)
}

#[cfg(feature = "camera")]
mod camera {
    use std::sync::mpsc;
    use std::time::Duration;

    use nokhwa::Camera;
    use nokhwa::pixel_format::RgbAFormat;
    use nokhwa::utils::{ApiBackend, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType};
    use yuvutils_rs::{
        YuvBiPlanarImage, YuvConversionMode, YuvPackedImage, YuvRange, YuvStandardMatrix, yuv_nv12_to_rgba,
        yuyv422_to_rgba,
    };

    use super::{CameraDevice, FormatOption, LiveStatus, Shared, find_camera, pick_format};
    use crate::Frame;

    pub fn list() -> Vec<CameraDevice> {
        let Ok(cameras) = nokhwa::query(ApiBackend::Auto) else { return Vec::new() };
        cameras.iter().map(|c| CameraDevice { name: c.human_name(), id: c.misc() }).collect()
    }

    /// Asks for camera access (macOS shows its permission prompt once) and waits for the answer.
    fn ensure_permission() -> Result<(), String> {
        if nokhwa::nokhwa_check() {
            return Ok(());
        }
        let (tx, rx) = mpsc::channel();
        nokhwa::nokhwa_initialize(move |granted| {
            let _ = tx.send(granted);
        });
        match rx.recv_timeout(Duration::from_secs(120)) {
            Ok(true) => Ok(()),
            _ => Err("camera access was denied. Allow it in System Settings → Privacy & Security → Camera".into()),
        }
    }

    pub fn run(shared: &Shared, name: &str, id: &str, max: (u32, u32)) {
        let mut camera = match open(name, id, max) {
            Ok(c) => c,
            Err(e) => return shared.set_status(LiveStatus::Failed(e)),
        };
        let res = camera.resolution();
        let (w, h) = (res.width(), res.height());
        let detail = format!("{w}×{h} · {} fps · {}", camera.frame_rate(), camera.frame_format());
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        let mut errors = 0;
        while !shared.stopped() {
            let ok = match camera.frame() {
                Ok(buf) => {
                    convert(buf.buffer(), buf.source_frame_format(), w, h, &mut rgba)
                        || buf.decode_image_to_buffer::<RgbAFormat>(&mut rgba).is_ok()
                }
                Err(_) => false,
            };
            if ok {
                if errors > 0 || matches!(shared.status.lock().unwrap().as_ref(), Some(LiveStatus::Connecting)) {
                    shared.set_status(LiveStatus::Running { detail: detail.clone() });
                }
                errors = 0;
                shared.publish(Frame { width: w, height: h, rgba: rgba.clone() });
            } else {
                errors += 1;
                if errors == 60 {
                    shared.set_status(LiveStatus::Failed(format!("{name} stopped sending frames")));
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        let _ = camera.stop_stream();
    }

    fn open(name: &str, id: &str, max: (u32, u32)) -> Result<Camera, String> {
        ensure_permission()?;
        let devices = nokhwa::query(ApiBackend::Auto).map_err(|e| format!("could not list cameras: {e}"))?;
        let found: Vec<CameraDevice> =
            devices.iter().map(|c| CameraDevice { name: c.human_name(), id: c.misc() }).collect();
        let i = find_camera(&found, name, id).ok_or_else(|| format!("camera \"{name}\" is not connected"))?;
        let index: CameraIndex = devices[i].index().clone();

        let any = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
        let mut camera = Camera::new(index, any).map_err(|e| format!("could not open {name}: {e}"))?;
        if let Ok(formats) = camera.compatible_camera_formats() {
            let options: Vec<FormatOption> = formats
                .iter()
                .map(|f| FormatOption {
                    width: f.width(),
                    height: f.height(),
                    fps: f.frame_rate(),
                    compressed: f.format() == FrameFormat::MJPEG,
                })
                .collect();
            if let Some(best) = pick_format(&options, max.0, max.1) {
                let exact = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::Exact(formats[best]));
                // Keep the camera's own choice if the exact format is refused.
                let _ = camera.set_camera_requset(exact);
            }
        }
        camera.open_stream().map_err(|e| format!("could not start {name}: {e}"))?;
        Ok(camera)
    }

    /// Fast paths for the raw formats cameras usually deliver. Returns false for anything
    /// else (MJPEG, grey), which nokhwa decodes instead.
    fn convert(raw: &[u8], format: FrameFormat, w: u32, h: u32, rgba: &mut [u8]) -> bool {
        let y_size = (w * h) as usize;
        match format {
            FrameFormat::NV12 if raw.len() >= y_size * 3 / 2 => {
                let image = YuvBiPlanarImage {
                    y_plane: &raw[..y_size],
                    y_stride: w,
                    uv_plane: &raw[y_size..],
                    uv_stride: w,
                    width: w,
                    height: h,
                };
                let (range, matrix) = (YuvRange::Limited, YuvStandardMatrix::Bt709);
                yuv_nv12_to_rgba(&image, rgba, w * 4, range, matrix, YuvConversionMode::Balanced).is_ok()
            }
            FrameFormat::YUYV if raw.len() >= y_size * 2 => {
                let image = YuvPackedImage { yuy: &raw[..y_size * 2], yuy_stride: w * 2, width: w, height: h };
                yuyv422_to_rgba(&image, rgba, w * 4, YuvRange::Limited, YuvStandardMatrix::Bt709).is_ok()
            }
            _ => false,
        }
    }
}

#[cfg(all(feature = "syphon", target_os = "macos"))]
mod syphon {
    use std::sync::Arc;
    use std::time::Duration;

    use syphon_core::{SyphonClient, SyphonServerDirectory};

    use super::{LiveStatus, Shared, Spawned, SyphonServer, bgra_to_rgba, find_syphon};
    use crate::Frame;

    pub fn list() -> Vec<SyphonServer> {
        if !syphon_core::is_available() {
            return Vec::new();
        }
        SyphonServerDirectory::servers().into_iter().map(|s| SyphonServer { app: s.app_name, server: s.name }).collect()
    }

    /// Connects on the calling (UI) thread, where the server directory lives, then
    /// receives on a worker thread.
    pub fn open(shared: &Arc<Shared>, app: &str, server: &str) -> Spawned {
        if !syphon_core::is_available() {
            return Err("Syphon.framework could not be loaded".into());
        }
        let infos = SyphonServerDirectory::servers();
        let servers: Vec<SyphonServer> =
            infos.iter().map(|s| SyphonServer { app: s.app_name.clone(), server: s.name.clone() }).collect();
        let label = openvj_label(app, server);
        let i =
            find_syphon(&servers, app, server).ok_or_else(|| format!("Syphon server \"{label}\" is not running"))?;
        let (client, frames) = SyphonClient::connect_by_info_with_channel(&infos[i])
            .map_err(|e| format!("could not connect to {label}: {e}"))?;
        let shared = shared.clone();
        std::thread::Builder::new()
            .name("openvj-syphon".into())
            .spawn(move || {
                let mut running = false;
                while !shared.stopped() {
                    // Wakes on each new frame; the timeout lets it notice stop and server exit.
                    let _ = frames.recv_timeout(Duration::from_millis(100));
                    if !client.is_connected() {
                        shared.set_status(LiveStatus::Failed(format!("{label} stopped")));
                        break;
                    }
                    let Ok(Some(mut frame)) = client.try_receive() else { continue };
                    let (w, h, stride) = (frame.width, frame.height, frame.bytes_per_row());
                    let Ok(bgra) = frame.to_vec() else { continue };
                    if !running {
                        running = true;
                        shared.set_status(LiveStatus::Running { detail: format!("{w}×{h} · Syphon") });
                    }
                    // Syphon shares OpenGL textures, whose rows run bottom to top.
                    let rgba = bgra_to_rgba(&bgra, w as usize, h as usize, stride, true);
                    shared.publish(Frame { width: w, height: h, rgba });
                }
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn openvj_label(app: &str, server: &str) -> String {
        if server.is_empty() || server == app { app.to_owned() } else { format!("{app} - {server}") }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam(name: &str, id: &str) -> CameraDevice {
        CameraDevice { name: name.into(), id: id.into() }
    }

    #[test]
    fn cameras_are_found_by_id_then_name() {
        let devices = [cam("FaceTime HD Camera", "47B4"), cam("HD Pro Webcam C920", "0x2114400046d082d")];
        assert_eq!(find_camera(&devices, "HD Pro Webcam C920", "0x2114400046d082d"), Some(1));
        // Same webcam on another USB port: new id, same name.
        assert_eq!(find_camera(&devices, "HD Pro Webcam C920", "0x114400046d082d"), Some(1));
        assert_eq!(find_camera(&devices, "facetime hd camera", ""), Some(0));
        assert_eq!(find_camera(&devices, "OBS Virtual Camera", "7626"), None);
    }

    #[test]
    fn syphon_servers_are_found_by_name_then_app() {
        let servers = [
            SyphonServer { app: "TouchDesigner".into(), server: "TDSyphonSpoutOut".into() },
            SyphonServer { app: "Synesthesia".into(), server: "Synesthesia".into() },
        ];
        assert_eq!(find_syphon(&servers, "Synesthesia", "Synesthesia"), Some(1));
        assert_eq!(find_syphon(&servers, "Synesthesia", "Main Output"), Some(1));
        assert_eq!(find_syphon(&servers, "TouchDesigner", "TDSyphonSpoutOut"), Some(0));
        assert_eq!(find_syphon(&servers, "ISFEditor", "ISF Test App"), None);
    }

    #[test]
    fn swaps_bgra_and_drops_row_padding() {
        let src = [1, 2, 3, 4, 5, 6, 7, 8, 0, 0, 9, 10, 11, 12, 13, 14, 15, 16, 0, 0];
        assert_eq!(bgra_to_rgba(&src, 2, 2, 10, false), [3, 2, 1, 4, 7, 6, 5, 8, 11, 10, 9, 12, 15, 14, 13, 16]);
        // Bottom-up rows (Syphon) come out the right way up.
        assert_eq!(bgra_to_rgba(&src, 2, 2, 10, true), [11, 10, 9, 12, 15, 14, 13, 16, 3, 2, 1, 4, 7, 6, 5, 8]);
    }

    #[test]
    fn picks_the_largest_smooth_format_that_fits() {
        let f = |width, height, fps, compressed| FormatOption { width, height, fps, compressed };
        let c920 = [f(640, 480, 30, false), f(1920, 1080, 5, false), f(1920, 1080, 30, true), f(1280, 720, 30, true)];
        assert_eq!(pick_format(&c920, 1920, 1080), Some(2));
        assert_eq!(pick_format(&c920, 1280, 720), Some(3));
        // Nothing fits a tiny composition: take the smallest smooth format.
        assert_eq!(pick_format(&c920, 320, 240), Some(0));
        // Uncompressed wins a tie.
        assert_eq!(pick_format(&[f(1280, 720, 30, true), f(1280, 720, 30, false)], 1920, 1080), Some(1));
        assert_eq!(pick_format(&[f(4000, 3000, 10, false)], 1920, 1080), Some(0));
        assert_eq!(pick_format(&[], 1920, 1080), None);
    }

    /// Linux only: on macOS this would ask for camera permission.
    #[cfg(all(feature = "camera", target_os = "linux"))]
    #[test]
    fn a_missing_camera_reports_why() {
        let stream = LiveStream::open(&LiveSource::Camera { name: "No Such Camera".into(), id: String::new() }, 64, 64);
        for _ in 0..500 {
            if let LiveStatus::Failed(e) = stream.status() {
                assert!(e.contains("No Such Camera") || e.contains("camera"), "{e}");
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("still {:?}", stream.status());
    }
}
