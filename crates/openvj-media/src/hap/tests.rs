//! HAP tests. Hand-built frames cover every section layout the decoder reads, and FFmpeg's
//! HAP encoder and decoder cross-check real files when FFmpeg is installed.

use std::process::Command;

use super::*;
use crate::dxv::{bc_alpha_block, bc_color_block, crop, expand_blocks};
use crate::mov::write_test_mov;
use crate::{ffmpeg_available, tool_path};

/// Deterministic bytes with long runs, so Snappy has something to compress.
fn texture(len: usize, seed: u8) -> Vec<u8> {
    (0..len).map(|i| if (i / 32) % 3 == 0 { seed } else { (i as u8).wrapping_mul(31) ^ seed }).collect()
}

fn header(kind: u8, len: usize, long: bool) -> Vec<u8> {
    if long || len >= 1 << 24 {
        let mut h = vec![0, 0, 0, kind];
        h.extend((len as u32).to_le_bytes());
        h
    } else {
        let b = (len as u32).to_le_bytes();
        vec![b[0], b[1], b[2], kind]
    }
}

fn sec(kind: u8, body: &[u8]) -> Vec<u8> {
    let mut out = header(kind, body.len(), false);
    out.extend_from_slice(body);
    out
}

fn snappy(data: &[u8]) -> Vec<u8> {
    snap::raw::Encoder::new().compress_vec(data).unwrap()
}

/// How the test packs a texture.
#[derive(Clone, Copy, Debug)]
enum Pack {
    Raw,
    Snappy,
    /// Chunks alternating Snappy / none, with or without an offset table.
    Chunks {
        n: usize,
        offsets: bool,
    },
}

fn image(tex_nibble: u8, tex: &[u8], pack: Pack) -> Vec<u8> {
    match pack {
        Pack::Raw => sec(0xA0 | tex_nibble, tex),
        Pack::Snappy => sec(0xB0 | tex_nibble, &snappy(tex)),
        Pack::Chunks { n, offsets } => {
            let step = tex.len().div_ceil(n);
            let mut compressors = Vec::new();
            let mut sizes = Vec::new();
            let mut chunks: Vec<Vec<u8>> = Vec::new();
            for (i, part) in tex.chunks(step).enumerate() {
                let data = if i % 2 == 0 { snappy(part) } else { part.to_vec() };
                compressors.push(if i % 2 == 0 { 0x0B } else { 0x0A });
                sizes.extend((data.len() as u32).to_le_bytes());
                chunks.push(data);
            }
            let mut instructions = sec(0x02, &compressors);
            instructions.extend(sec(0x03, &sizes));
            // An unknown section must be skipped.
            instructions.extend(sec(0x7E, b"ignore me"));
            let frame: Vec<u8> = if offsets {
                // Store chunks in reverse so the offsets matter.
                let mut frame = Vec::new();
                let mut offs = vec![0u32; chunks.len()];
                for (i, c) in chunks.iter().enumerate().rev() {
                    offs[i] = frame.len() as u32;
                    frame.extend_from_slice(c);
                }
                instructions.extend(sec(0x04, &offs.iter().flat_map(|o| o.to_le_bytes()).collect::<Vec<_>>()));
                frame
            } else {
                chunks.concat()
            };
            let mut body = sec(0x01, &instructions);
            body.extend(frame);
            sec(0xC0 | tex_nibble, &body)
        }
    }
}

fn len_for(w: u32, h: u32, block: usize) -> usize {
    w.div_ceil(4) as usize * h.div_ceil(4) as usize * block
}

#[test]
fn every_format_and_packing_round_trips() {
    let packs = [
        Pack::Raw,
        Pack::Snappy,
        Pack::Chunks { n: 1, offsets: false },
        Pack::Chunks { n: 4, offsets: false },
        Pack::Chunks { n: 3, offsets: true },
    ];
    let (w, h) = (38, 22); // Not multiples of 4: textures are padded.
    for pack in packs {
        for (nibble, block, format) in [
            (0xB, 8, HapFormat::Hap),
            (0xE, 16, HapFormat::HapAlpha),
            (0xF, 16, HapFormat::HapQ),
            (0xC, 16, HapFormat::HapR),
        ] {
            let tex = texture(len_for(w, h, block), nibble);
            let f = decode_frame(&image(nibble, &tex, pack), w, h).unwrap();
            assert_eq!(f.format, format, "{pack:?}");
            assert_eq!((f.coded_width, f.coded_height), (40, 24));
            assert!(f.texture == tex && f.alpha.is_empty(), "{format:?} {pack:?}");
        }
        // HAP Q Alpha: both images in a multiple-images section, in either order.
        let ycocg = texture(len_for(w, h, 16), 1);
        let alpha = texture(len_for(w, h, 8), 2);
        for swap in [false, true] {
            let (a, b) = (image(0xF, &ycocg, pack), image(0x1, &alpha, pack));
            let body = if swap { [b, a].concat() } else { [a, b].concat() };
            let f = decode_frame(&sec(0x0D, &body), w, h).unwrap();
            assert_eq!(f.format, HapFormat::HapQAlpha);
            assert!(f.texture == ycocg && f.alpha == alpha, "{pack:?}");
        }
    }
}

#[test]
fn long_section_headers_decode() {
    let tex = texture(len_for(8, 8, 8), 3);
    let mut packet = header(0xAB, tex.len(), true);
    packet.extend_from_slice(&tex);
    assert_eq!(decode_frame(&packet, 8, 8).unwrap().texture, tex);
}

#[test]
fn corrupt_frames_fail_cleanly() {
    let tex = texture(len_for(16, 16, 16), 4);
    let good = image(0xF, &tex, Pack::Chunks { n: 4, offsets: true });
    assert!(decode_frame(&good, 16, 16).is_ok());
    // Every truncation, and every single-byte change, errors or decodes without panicking.
    for n in 0..good.len() {
        assert!(decode_frame(&good[..n], 16, 16).is_err(), "truncated to {n}");
    }
    for i in 0..good.len() {
        for v in [0x00, 0x01, 0x7F, 0xFF] {
            let mut bad = good.clone();
            bad[i] = v;
            let _ = decode_frame(&bad, 16, 16);
        }
    }
    // Wrong size for the movie, unknown formats, alpha-only.
    assert!(decode_frame(&good, 32, 16).is_err());
    assert!(decode_frame(&sec(0xA7, &tex), 16, 16).is_err());
    assert!(decode_frame(&sec(0xD1, &tex), 16, 16).is_err());
    assert!(decode_frame(&image(0x1, &texture(len_for(16, 16, 8), 0), Pack::Raw), 16, 16).is_err());
    // Two images that don't make HAP Q Alpha.
    let two = [
        image(0xB, &texture(len_for(16, 16, 8), 0), Pack::Raw),
        image(0x1, &texture(len_for(16, 16, 8), 0), Pack::Raw),
    ];
    assert!(decode_frame(&sec(0x0D, &two.concat()), 16, 16).is_err());
    // A Snappy stream claiming a huge size is refused before allocating.
    let mut huge = vec![0xFF, 0xFF, 0xFF, 0xFF, 0x0F];
    huge.extend([0; 16]);
    assert!(decode_frame(&sec(0xBB, &huge), 16, 16).is_err());
}

/// Expands a frame to straight RGBA on the CPU, as the GPU shader does. Not for HAP R.
fn to_rgba(f: &HapFrame) -> Vec<u8> {
    let (cw, ch) = (f.coded_width as usize, f.coded_height as usize);
    let mut full = vec![0u8; cw * ch * 4];
    let mut alpha = vec![255u8; cw * ch];
    if f.format == HapFormat::HapQAlpha {
        expand_blocks(&f.alpha, cw, ch, 8, |b, x, y| {
            let mut v = [0u8; 16];
            bc_alpha_block(b, &mut v);
            for (i, v) in v.into_iter().enumerate() {
                alpha[(y + i / 4) * cw + x + i % 4] = v;
            }
        });
    }
    let block = if f.format == HapFormat::Hap { 8 } else { 16 };
    expand_blocks(&f.texture, cw, ch, block, |b, x, y| {
        let mut px = [[0u8; 4]; 16];
        let mut a = [255u8; 16];
        if f.format == HapFormat::Hap {
            bc_color_block(b, &mut px, false);
        } else {
            bc_alpha_block(&b[..8], &mut a);
            bc_color_block(&b[8..], &mut px, true);
        }
        for i in 0..16 {
            let at = (y + i / 4) * cw + x + i % 4;
            let p = px[i];
            let rgba = match f.format {
                HapFormat::Hap => [p[0], p[1], p[2], 255],
                HapFormat::HapAlpha => [p[0], p[1], p[2], a[i]],
                HapFormat::HapQ | HapFormat::HapQAlpha => {
                    let scale = p[2] as f32 / 8.0 + 1.0;
                    let co = (p[0] as f32 - 128.0) / scale;
                    let cg = (p[1] as f32 - 128.0) / scale;
                    let luma = a[i] as f32;
                    let q = |v: f32| v.round().clamp(0.0, 255.0) as u8;
                    [q(luma + co - cg), q(luma + cg), q(luma - co - cg), alpha[at]]
                }
                HapFormat::HapR => unreachable!("no CPU BC7 decoder"),
            };
            full[at * 4..at * 4 + 4].copy_from_slice(&rgba);
        }
    });
    crop(&full, cw, f.width as usize, f.height as usize, 4)
}

fn ffmpeg(args: &[&str], path: &Path) -> Vec<u8> {
    let out = Command::new(tool_path("ffmpeg")).args(["-v", "error", "-y"]).args(args).arg(path).output().unwrap();
    assert!(out.status.success(), "ffmpeg failed: {}", String::from_utf8_lossy(&out.stderr));
    out.stdout
}

fn ffmpeg_decode(path: &Path) -> Vec<u8> {
    let out = Command::new(tool_path("ffmpeg"))
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgba", "-"])
        .output()
        .unwrap();
    assert!(out.status.success(), "ffmpeg failed: {}", String::from_utf8_lossy(&out.stderr));
    out.stdout
}

/// True if FFmpeg has its HAP encoder, which needs libsnappy (Homebrew's FFmpeg lacks it).
fn ffmpeg_has_hap_encoder() -> bool {
    Command::new(tool_path("ffmpeg"))
        .args(["-hide_banner", "-encoders"])
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).lines().any(|l| l.split_whitespace().nth(1) == Some("hap")))
}

/// HAP files made by FFmpeg's encoder decode to the pixels FFmpeg's decoder gives.
/// Runs where FFmpeg has the HAP encoder (CI's Linux job does).
#[test]
fn matches_ffmpeg() {
    if !ffmpeg_available() || !ffmpeg_has_hap_encoder() {
        eprintln!("skipping: ffmpeg without the HAP encoder");
        return;
    }
    let dir = std::env::temp_dir().join(format!("openvj-hap-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = "testsrc2=s=72x44:d=0.2:r=10,format=rgba,geq=r='r(X,Y)':g='g(X,Y)':b='b(X,Y)':a='X*3.5'";
    for (name, want) in [("hap", HapFormat::Hap), ("hap_alpha", HapFormat::HapAlpha), ("hap_q", HapFormat::HapQ)] {
        for (compressor, chunks) in [("snappy", "1"), ("snappy", "4"), ("none", "1")] {
            let path = dir.join(format!("{name}-{compressor}-{chunks}.mov"));
            ffmpeg(
                &[
                    "-f",
                    "lavfi",
                    "-i",
                    src,
                    "-c:v",
                    "hap",
                    "-format",
                    name,
                    "-compressor",
                    compressor,
                    "-chunks",
                    chunks,
                ],
                &path,
            );
            assert_eq!(probe(&path), Some(want));
            let track = mov::read_video_track(&path).unwrap();
            let mut file = File::open(&path).unwrap();
            let mut buf = Vec::new();
            read_sample(&mut file, track.samples[0], &mut buf).unwrap();
            let frame = decode_frame(&buf, track.width, track.height).unwrap();
            assert_eq!(frame.format, want);
            let ours = to_rgba(&frame);
            let theirs = ffmpeg_decode(&path);
            assert_eq!(ours.len(), theirs.len());
            // FFmpeg rounds HAP Q's chroma with integer division; allow for that.
            let tolerance = if want == HapFormat::HapQ { 2 } else { 0 };
            let worst = ours.iter().zip(&theirs).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
            assert!(worst <= tolerance, "{name} {compressor} x{chunks}: off by {worst}");
            if want == HapFormat::HapAlpha {
                assert!(ours.chunks(4).any(|p| p[3] < 128) && ours.chunks(4).any(|p| p[3] > 128), "alpha varies");
            }
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn decoder_plays_a_movie() {
    let dir = std::env::temp_dir().join(format!("openvj-hap-play-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("play.mov");
    // Each frame's first texture byte is its index.
    let packets: Vec<Vec<u8>> = (0..10u8)
        .map(|i| {
            let mut t = vec![0u8; len_for(32, 16, 16)];
            t[0] = i;
            image(0xF, &t, Pack::Snappy)
        })
        .collect();
    write_test_mov(&path, b"HapY", 32, 16, 10, &packets);
    assert!(is_hap(&path));
    assert_eq!(probe(&path), Some(HapFormat::HapQ));
    let mut dec = HapDecoder::open(&path).unwrap();
    assert_eq!(dec.format, HapFormat::HapQ);
    assert_eq!(dec.info.fps, 10.0);
    let reaches = |dec: &mut HapDecoder, t: f64, want: u8| {
        for _ in 0..500 {
            if dec.frame_at(t).is_some_and(|f| f.texture[0] == want) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        panic!("never showed frame {want} at t={t}");
    };
    reaches(&mut dec, 0.0, 0);
    reaches(&mut dec, 0.55, 5);
    reaches(&mut dec, 1.25, 2);
    drop(dec);

    // Not HAP: DXV and garbage are refused, so the app can try other players.
    write_test_mov(&path, b"DXD3", 32, 16, 10, &packets);
    assert!(!is_hap(&path) && probe(&path).is_none() && HapDecoder::open(&path).is_err());
    let _ = std::fs::remove_dir_all(dir);
}
