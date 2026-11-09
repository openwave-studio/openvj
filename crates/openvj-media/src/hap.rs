//! Vidvox HAP decoding to GPU-ready block-compressed textures.
//!
//! A HAP frame is a DXT / BC7 texture, optionally Snappy-compressed (whole, or in chunks so
//! encoders and decoders can work in parallel), in a small sectioned layout. Undoing Snappy
//! is all the CPU does; the GPU samples the blocks natively, as with [`crate::dxv`].
//! Follows Vidvox's HAP spec (github.com/Vidvox/hap, documentation/HapVideoDRAFT.md).
//!
//! | HAP variant   | fourcc | GPU layout                                     |
//! |---------------|--------|------------------------------------------------|
//! | HAP           | Hap1   | BC1                                            |
//! | HAP Alpha     | Hap5   | BC3, straight alpha                            |
//! | HAP Q         | HapY   | BC3 holding scaled YCoCg                       |
//! | HAP Q Alpha   | HapM   | BC3 scaled YCoCg + BC4 alpha, both full size   |
//! | HAP R         | Hap7   | BC7                                            |

use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail, ensure};

use crate::VideoInfo;
use crate::mov;
use crate::player::{FramePlayer, read_sample};

/// Sample description fourccs of the HAP variants openVJ plays.
const FOURCCS: [&[u8; 4]; 5] = [b"Hap1", b"Hap5", b"HapY", b"HapM", b"Hap7"];

/// How a decoded frame's textures are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HapFormat {
    /// `texture` is BC1; always opaque.
    Hap,
    /// `texture` is BC3 with straight alpha.
    HapAlpha,
    /// `texture` is BC3 holding scaled YCoCg (R = Co, G = Cg, B = scale, A = Y).
    HapQ,
    /// Like [`HapFormat::HapQ`], plus `alpha`: BC4 at the same size.
    HapQAlpha,
    /// `texture` is BC7 (RGBA, straight alpha).
    HapR,
}

impl HapFormat {
    pub fn name(self) -> &'static str {
        match self {
            HapFormat::Hap => "HAP",
            HapFormat::HapAlpha => "HAP alpha",
            HapFormat::HapQ => "HAP Q",
            HapFormat::HapQAlpha => "HAP Q alpha",
            HapFormat::HapR => "HAP R",
        }
    }
}

/// One decoded HAP frame, still block-compressed.
#[derive(Clone)]
pub struct HapFrame {
    pub format: HapFormat,
    /// Visible size.
    pub width: u32,
    pub height: u32,
    /// Size of the textures in pixels: the visible size rounded up to multiples of 4.
    pub coded_width: u32,
    pub coded_height: u32,
    /// Blocks in row-major order.
    pub texture: Vec<u8>,
    /// BC4 alpha blocks for [`HapFormat::HapQAlpha`]; empty otherwise.
    pub alpha: Vec<u8>,
}

impl std::fmt::Debug for HapFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HapFrame({:?} {}x{})", self.format, self.width, self.height)
    }
}

/// Texture formats a HAP image section can hold (low nibble of its type byte).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Texture {
    Dxt1,
    Dxt5,
    ScaledYCoCg,
    Bc7,
    Alpha,
}

impl Texture {
    fn from_type(t: u8) -> Option<Self> {
        Some(match t & 0x0F {
            0xB => Texture::Dxt1,
            0xE => Texture::Dxt5,
            0xF => Texture::ScaledYCoCg,
            0xC => Texture::Bc7,
            0x1 => Texture::Alpha,
            _ => return None,
        })
    }

    fn block_bytes(self) -> usize {
        match self {
            Texture::Dxt1 | Texture::Alpha => 8,
            Texture::Dxt5 | Texture::ScaledYCoCg | Texture::Bc7 => 16,
        }
    }
}

const SECTION_MULTIPLE_IMAGES: u8 = 0x0D;
const SECTION_DECODE_INSTRUCTIONS: u8 = 0x01;
const SECTION_CHUNK_COMPRESSORS: u8 = 0x02;
const SECTION_CHUNK_SIZES: u8 = 0x03;
const SECTION_CHUNK_OFFSETS: u8 = 0x04;

const COMPRESSOR_NONE: u8 = 0x0A;
const COMPRESSOR_SNAPPY: u8 = 0x0B;
const COMPRESSOR_COMPLEX: u8 = 0x0C;

/// Splits off the section at the start of `buf`: `(type, body, rest)`.
fn section(buf: &[u8]) -> Result<(u8, &[u8], &[u8])> {
    ensure!(buf.len() >= 4, "truncated HAP section header");
    let mut size = u32::from_le_bytes([buf[0], buf[1], buf[2], 0]) as usize;
    let kind = buf[3];
    let mut header = 4usize;
    if size == 0 {
        ensure!(buf.len() >= 8, "truncated HAP section header");
        size = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
        header = 8;
    }
    let end = header.checked_add(size).filter(|&e| e <= buf.len()).ok_or_else(|| anyhow!("truncated HAP section"))?;
    Ok((kind, &buf[header..end], &buf[end..]))
}

/// Iterates the sections packed in `buf`.
fn sections(mut buf: &[u8]) -> impl Iterator<Item = Result<(u8, &[u8])>> {
    std::iter::from_fn(move || {
        if buf.is_empty() {
            return None;
        }
        Some(section(buf).map(|(kind, body, rest)| {
            buf = rest;
            (kind, body)
        }))
    })
}

/// Appends `data`, decompressed by `compressor`, to `out`, never growing it past `limit`.
fn decompress(compressor: u8, data: &[u8], out: &mut Vec<u8>, limit: usize) -> Result<()> {
    match compressor {
        COMPRESSOR_NONE => {
            ensure!(out.len() + data.len() <= limit, "HAP frame data is too large");
            out.extend_from_slice(data);
        }
        COMPRESSOR_SNAPPY => {
            let len = snap::raw::decompress_len(data)?;
            ensure!(out.len() + len <= limit, "HAP frame data is too large");
            let at = out.len();
            out.resize(at + len, 0);
            snap::raw::Decoder::new().decompress(data, &mut out[at..])?;
        }
        c => bail!("unknown HAP compressor {c:#x}"),
    }
    Ok(())
}

/// Decodes a frame whose data is split into chunks, as described by its decode instructions.
fn decompress_chunks(body: &[u8], out: &mut Vec<u8>, limit: usize) -> Result<()> {
    let (kind, instructions, frame) = section(body)?;
    ensure!(kind == SECTION_DECODE_INSTRUCTIONS, "HAP decode instructions missing");
    let (mut compressors, mut sizes, mut offsets) = (None, None, None);
    for s in sections(instructions) {
        match s? {
            (SECTION_CHUNK_COMPRESSORS, b) => compressors = Some(b),
            (SECTION_CHUNK_SIZES, b) => sizes = Some(b),
            (SECTION_CHUNK_OFFSETS, b) => offsets = Some(b),
            _ => {} // Unknown sections are skipped, as the spec asks.
        }
    }
    let compressors = compressors.ok_or_else(|| anyhow!("HAP chunk compressor table missing"))?;
    let sizes = sizes.ok_or_else(|| anyhow!("HAP chunk size table missing"))?;
    let n = compressors.len();
    ensure!(sizes.len() == n * 4, "HAP chunk tables disagree");
    ensure!(offsets.is_none_or(|o| o.len() == n * 4), "HAP chunk tables disagree");
    let le32 = |b: &[u8], i: usize| u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap()) as usize;
    let mut running = 0usize;
    for (i, &compressor) in compressors.iter().enumerate() {
        let size = le32(sizes, i);
        let start = offsets.map_or(running, |o| le32(o, i));
        running = start.saturating_add(size);
        let chunk = frame.get(start..running).ok_or_else(|| anyhow!("HAP chunk {i} is out of bounds"))?;
        decompress(compressor, chunk, out, limit)?;
    }
    Ok(())
}

/// Decodes one top-level image section to its texture blocks.
fn decode_image(kind: u8, body: &[u8], width: u32, height: u32) -> Result<(Texture, Vec<u8>)> {
    let texture = Texture::from_type(kind).ok_or_else(|| anyhow!("unsupported HAP texture format {kind:#04x}"))?;
    let blocks = width.div_ceil(4) as usize * height.div_ceil(4) as usize;
    let expected = blocks * texture.block_bytes();
    let mut out = Vec::with_capacity(expected);
    match kind >> 4 {
        COMPRESSOR_COMPLEX => decompress_chunks(body, &mut out, expected)?,
        c => decompress(c, body, &mut out, expected)?,
    }
    ensure!(out.len() == expected, "HAP texture is {} bytes, expected {expected}", out.len());
    Ok((texture, out))
}

/// Decodes one HAP frame of a `width`×`height` movie.
pub fn decode_frame(packet: &[u8], width: u32, height: u32) -> Result<HapFrame> {
    ensure!(width > 0 && height > 0 && width <= 16384 && height <= 16384, "bad HAP frame size {width}x{height}");
    let (kind, body, _) = section(packet)?;
    let mut images = Vec::new();
    if kind == SECTION_MULTIPLE_IMAGES {
        for s in sections(body) {
            let (kind, body) = s?;
            images.push(decode_image(kind, body, width, height)?);
        }
    } else {
        images.push(decode_image(kind, body, width, height)?);
    }
    let take = |images: &mut Vec<(Texture, Vec<u8>)>, t: Texture| {
        images.iter().position(|(k, _)| *k == t).map(|i| images.swap_remove(i).1)
    };
    let (format, texture, alpha) = match images.as_slice() {
        [(Texture::Dxt1, _)] => (HapFormat::Hap, images.remove(0).1, Vec::new()),
        [(Texture::Dxt5, _)] => (HapFormat::HapAlpha, images.remove(0).1, Vec::new()),
        [(Texture::ScaledYCoCg, _)] => (HapFormat::HapQ, images.remove(0).1, Vec::new()),
        [(Texture::Bc7, _)] => (HapFormat::HapR, images.remove(0).1, Vec::new()),
        [_, _] => {
            let ycocg = take(&mut images, Texture::ScaledYCoCg);
            let alpha = take(&mut images, Texture::Alpha);
            match (ycocg, alpha) {
                (Some(ycocg), Some(alpha)) => (HapFormat::HapQAlpha, ycocg, alpha),
                _ => bail!("unsupported HAP image combination"),
            }
        }
        [(Texture::Alpha, _)] => bail!("HAP Alpha-Only isn't supported"),
        _ => bail!("unsupported HAP frame layout"),
    };
    Ok(HapFrame {
        format,
        width,
        height,
        coded_width: width.next_multiple_of(4),
        coded_height: height.next_multiple_of(4),
        texture,
        alpha,
    })
}

// ---------------------------------------------------------------- player

fn hap_track(path: &Path) -> Option<mov::VideoTrack> {
    mov::read_video_track(path).ok().filter(|t| FOURCCS.contains(&&t.fourcc))
}

/// True if `path` is a QuickTime movie whose first video track is a HAP variant openVJ plays.
pub fn is_hap(path: &Path) -> bool {
    hap_track(path).is_some()
}

/// The HAP variant of a movie, from its sample description; `None` if it isn't HAP.
pub fn probe(path: &Path) -> Option<HapFormat> {
    Some(match &hap_track(path)?.fourcc {
        b"Hap1" => HapFormat::Hap,
        b"Hap5" => HapFormat::HapAlpha,
        b"HapY" => HapFormat::HapQ,
        b"HapM" => HapFormat::HapQAlpha,
        _ => HapFormat::HapR,
    })
}

/// Plays a HAP movie as block-compressed frames, looping; see [`crate::player`].
pub struct HapDecoder {
    pub info: VideoInfo,
    pub format: HapFormat,
    player: FramePlayer<HapFrame>,
}

impl HapDecoder {
    /// Opens a HAP movie. Fails if it isn't HAP or its first frame doesn't decode, so
    /// callers can fall back to FFmpeg.
    pub fn open(path: &Path) -> Result<Self> {
        let track = mov::read_video_track(path)?;
        if !FOURCCS.contains(&&track.fourcc) {
            bail!("not a HAP movie ({})", String::from_utf8_lossy(&track.fourcc));
        }
        ensure!(!track.samples.is_empty(), "HAP movie has no frames");
        let mut file = File::open(path).with_context(|| format!("could not open {}", path.display()))?;
        let mut buf = Vec::new();
        read_sample(&mut file, track.samples[0], &mut buf)?;
        let first = decode_frame(&buf, track.width, track.height)?;
        let info = track.video_info();
        let format = first.format;
        let player = FramePlayer::spawn("HAP", track, file, first, decode_frame)?;
        Ok(Self { info, format, player })
    }

    /// Returns the frame for clip time `t` (seconds) if it differs from the last one returned.
    pub fn frame_at(&mut self, t: f64) -> Option<HapFrame> {
        self.player.frame_at(t)
    }
}

#[cfg(test)]
mod tests;
