//! DXV tests. Small encoders below produce every compression path the decoder supports,
//! and FFmpeg's own DXV decoder cross-checks the results when it is installed.

use std::process::Command;

use super::*;
use crate::mov::write_test_mov;
use crate::{ffmpeg_available, tool_path};

/// Deterministic pseudo-random bytes with plenty of repeats, so back-references get used.
fn texture(len: usize, seed: u32, block: usize) -> Vec<u8> {
    let mut state = seed.wrapping_mul(0x9E37_79B9).wrapping_add(1);
    let mut rand = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    };
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        let r = rand();
        if out.len() >= block * 3 && r % 4 == 0 {
            // Repeat one of the last few blocks.
            let back = block * (1 + (r as usize >> 8) % 3);
            let start = out.len() - back;
            for i in 0..block {
                out.push(out[start + i]);
            }
        } else {
            for _ in 0..block {
                out.push(rand() as u8);
            }
        }
    }
    out.truncate(len);
    out
}

/// Packs 2-bit ops into 32-bit control words placed where the decoder reads them.
struct OpWriter {
    out: Vec<u8>,
    ctrl_at: usize,
    used: u32,
}

impl OpWriter {
    fn new() -> Self {
        Self { out: Vec::new(), ctrl_at: 0, used: 16 }
    }
    fn op(&mut self, op: u32) {
        if self.used == 16 {
            self.ctrl_at = self.out.len();
            self.out.extend_from_slice(&[0; 4]);
            self.used = 0;
        }
        let mut word = rd32(&self.out, self.ctrl_at);
        word |= op << (2 * self.used);
        wr32(&mut self.out, self.ctrl_at, word);
        self.used += 1;
    }
    fn u8(&mut self, v: u8) {
        self.out.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
}

fn words(tex: &[u8]) -> Vec<u32> {
    tex.chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect()
}

/// DXT1 with the 2-bit back-reference scheme, using every op.
fn encode_dxt1(tex: &[u8]) -> Vec<u8> {
    let w = words(tex);
    let mut e = OpWriter::new();
    e.u32(w[0]);
    e.u32(w[1]);
    let mut pos = 2;
    // Distance for one word using the CHECKPOINT ops (in words, x = 2).
    let find = |pos: usize, len: usize| -> Option<(u32, usize)> {
        let ok = |idx: usize| idx <= pos && (0..len).all(|k| w[pos + k] == w[pos + k - idx]);
        if ok(2) {
            return Some((1, 2));
        }
        for b in 0..256usize {
            if ok((b + 2) * 2) {
                return Some((2, b));
            }
        }
        for s in [0usize, 5, 300] {
            if ok((s + 0x102) * 2) {
                return Some((3, s));
            }
        }
        None
    };
    let emit = |e: &mut OpWriter, (op, arg): (u32, usize)| {
        e.op(op);
        match op {
            2 => e.u8(arg as u8),
            3 => e.u16(arg as u16),
            _ => {}
        }
    };
    while pos + 2 <= w.len() {
        if let Some(m) = find(pos, 2) {
            emit(&mut e, m);
        } else {
            e.op(0);
            for k in 0..2 {
                match find(pos + k, 1) {
                    Some(m) => emit(&mut e, m),
                    None => {
                        e.op(0);
                        e.u32(w[pos + k]);
                    }
                }
            }
        }
        pos += 2;
    }
    e.out
}

/// DXT5 using long copies, runs, back-references and literals. `far_refs` allows
/// back-references beyond the previous block, which FFmpeg before 7.0 decoded wrongly;
/// turning it off makes test data FFmpeg 6 compatible.
fn encode_dxt5(tex: &[u8], far_refs: bool) -> Vec<u8> {
    let w = words(tex);
    let mut e = OpWriter::new();
    for &v in &w[..4] {
        e.u32(v);
    }
    let mut pos = 4;
    let same_as_prev = |pos: usize| pos + 4 <= w.len() && (0..4).all(|k| w[pos + k] == w[pos + k - 4]);
    while pos + 2 <= w.len() {
        // Whole blocks repeating the previous one: long copy.
        let mut n = 0;
        while same_as_prev(pos + 4 * n) && n < 300 {
            n += 1;
        }
        if n > 0 {
            e.op(0);
            if n < 256 {
                e.u8((n - 1) as u8);
            } else {
                e.u8(255);
                e.u16((n - 256) as u16);
            }
            pos += 4 * n;
            continue;
        }
        // First half of the block.
        let back = (0..if far_refs { 16 } else { 1 })
            .map(|k| 8 + 4 * k)
            .find(|&idx| idx <= pos && w[pos] == w[pos - idx] && w[pos + 1] == w[pos + 1 - idx]);
        match back {
            Some(idx) => {
                e.op(2);
                e.u16(((idx - 8) / 4) as u16);
            }
            None => {
                e.op(3);
                e.u32(w[pos]);
                e.u32(w[pos + 1]);
            }
        }
        pos += 2;
        // Second half.
        if w[pos] == w[pos - 4] && w[pos + 1] == w[pos - 3] {
            e.op(1);
        } else {
            e.op(0);
            for k in 0..2 {
                if w[pos + k] == w[pos + k - 4] {
                    e.op(1);
                } else {
                    e.op(0);
                    e.u32(w[pos + k]);
                }
            }
        }
        pos += 2;
    }
    e.out
}

/// RGTC block chains (FFmpeg's "cgo" opcodes): returns the data stream and per-chain opcodes.
/// `chains` is 1 for BC4 and 2 for BC5 (interleaved 8-byte halves).
fn encode_rgtc(tex: &[u8], chains: usize) -> (Vec<u8>, Vec<Vec<u8>>) {
    let mut data = Vec::new();
    let mut ops = vec![Vec::new(); chains];
    let blocks: Vec<&[u8]> = tex.chunks(8).collect();
    data.extend(blocks[..chains].concat());
    let mut runs = vec![0usize; chains];
    for (i, b) in blocks.iter().enumerate().skip(chains) {
        let c = i % chains;
        if runs[c] > 0 {
            runs[c] -= 1;
            continue;
        }
        let prev = blocks[i - chains];
        if *b == prev {
            // Count the run of copies of the previous block in this chain.
            let mut n = 1;
            while i + n * chains < blocks.len() && blocks[i + n * chains] == prev && n < 300 {
                n += 1;
            }
            if n >= 4 {
                // Opcode 0 writes 1 + (v + 3) blocks.
                let v = n - 4;
                ops[c].push(0);
                if v < 255 {
                    data.push(v as u8);
                } else {
                    data.push(255);
                    data.extend_from_slice(&((v - 255) as u16).to_le_bytes());
                }
                runs[c] = n - 1;
            } else {
                ops[c].push(1);
            }
            continue;
        }
        let far = (2..20).find(|&k| i >= k * chains && blocks[i - k * chains] == *b);
        if let Some(k) = far {
            ops[c].push(2);
            data.extend_from_slice(&((k - 1) as u16).to_le_bytes());
        } else {
            ops[c].push(3);
            data.extend_from_slice(b);
        }
    }
    (data, ops)
}

/// Opcode list stored verbatim (flag 0), or as a constant (flag 1) when all equal.
fn store_ops(ops: &[u8]) -> Vec<u8> {
    if !ops.is_empty() && ops.iter().all(|&o| o == ops[0]) { vec![1, ops[0]] } else { [&[0u8][..], ops].concat() }
}

fn encode_yo(tex: &[u8]) -> Vec<u8> {
    let (data, ops) = encode_rgtc(tex, 1);
    let mut out = Vec::new();
    out.extend_from_slice(&((8 + data.len()) as u32).to_le_bytes());
    out.extend_from_slice(&(ops[0].len() as u32).to_le_bytes());
    out.extend(data);
    out.extend(store_ops(&ops[0]));
    out
}

fn encode_cocg(tex: &[u8]) -> Vec<u8> {
    let (data, ops) = encode_rgtc(tex, 2);
    let mut out = Vec::new();
    out.extend_from_slice(&((12 + data.len()) as u32).to_le_bytes());
    out.extend_from_slice(&(ops[0].len() as u32).to_le_bytes());
    out.extend_from_slice(&(ops[1].len() as u32).to_le_bytes());
    out.extend(data);
    out.extend(store_ops(&ops[0]));
    out.extend(store_ops(&ops[1]));
    out
}

fn packet(tag: &[u8; 4], raw: bool, body: &[u8]) -> Vec<u8> {
    let mut out = u32::from_be_bytes(*tag).to_le_bytes().to_vec();
    out.extend_from_slice(&[4, 0, raw as u8, 0]);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    out
}

/// A frame of `format` at `w`×`h`: the textures it should decode to, and its packet.
struct Case {
    texture: Vec<u8>,
    chroma: Vec<u8>,
    packet: Vec<u8>,
}

fn sizes(format: DxvFormat, w: u32, h: u32) -> (usize, usize) {
    let (cw, ch) = (w.next_multiple_of(16) as usize, h.next_multiple_of(4) as usize);
    let tex = cw / 4 * ch / 4 * format.block_bytes();
    let ctex = match format {
        DxvFormat::YCoCg | DxvFormat::YCoCgAlpha => cw / 2 / 4 * ch / 2 / 4 * 16,
        _ => 0,
    };
    (tex, ctex)
}

fn case(format: DxvFormat, w: u32, h: u32, raw: bool, seed: u32) -> Case {
    case_with(format, w, h, raw, seed, true)
}

fn case_with(format: DxvFormat, w: u32, h: u32, raw: bool, seed: u32, far_refs: bool) -> Case {
    let (tex_size, ctex_size) = sizes(format, w, h);
    let block = format.block_bytes();
    let mut texture = texture(tex_size, seed, block);
    let mut chroma = texture_chroma(ctex_size, seed);
    if !far_refs && matches!(format, DxvFormat::YCoCg | DxvFormat::YCoCgAlpha) {
        // FFmpeg before 7.0 read RGTC blocks with equal endpoints as flat; GPUs don't.
        for b in texture.chunks_mut(8).chain(chroma.chunks_mut(8)) {
            if b[0] == b[1] {
                b[1] ^= 1;
            }
        }
    }
    let tag = match format {
        DxvFormat::Dxt1 => b"DXT1",
        DxvFormat::Dxt5 => b"DXT5",
        DxvFormat::YCoCg => b"YCG6",
        DxvFormat::YCoCgAlpha => b"YG10",
    };
    let body = if raw {
        [texture.clone(), chroma.clone()].concat()
    } else {
        match format {
            DxvFormat::Dxt1 => encode_dxt1(&texture),
            DxvFormat::Dxt5 => encode_dxt5(&texture, far_refs),
            DxvFormat::YCoCg => [encode_yo(&texture), encode_cocg(&chroma)].concat(),
            DxvFormat::YCoCgAlpha => [encode_cocg(&texture), encode_cocg(&chroma)].concat(),
        }
    };
    Case { texture, chroma, packet: packet(tag, raw, &body) }
}

fn texture_chroma(len: usize, seed: u32) -> Vec<u8> {
    if len == 0 { Vec::new() } else { texture(len, seed ^ 0x5555, 16) }
}

const FORMATS: [DxvFormat; 4] = [DxvFormat::Dxt1, DxvFormat::Dxt5, DxvFormat::YCoCg, DxvFormat::YCoCgAlpha];

#[test]
fn every_format_round_trips() {
    for format in FORMATS {
        for (w, h) in [(64, 40), (60, 36), (1920 / 8, 1080 / 8)] {
            for raw in [false, true] {
                let c = case(format, w, h, raw, w ^ h);
                let f = decode_frame(&c.packet, w, h).unwrap_or_else(|e| panic!("{format:?} {w}x{h} raw={raw}: {e}"));
                assert_eq!(f.format, format);
                assert_eq!((f.width, f.height), (w, h));
                assert_eq!(f.coded_width, w.next_multiple_of(16));
                assert!(f.texture == c.texture, "{format:?} {w}x{h} raw={raw}: texture differs");
                assert!(f.chroma.starts_with(&c.chroma), "{format:?} {w}x{h} raw={raw}: chroma differs");
                if !c.chroma.is_empty() {
                    assert_eq!(f.chroma.len(), (f.coded_width / 2 / 4 * f.chroma_height / 4 * 16) as usize);
                }
            }
        }
    }
}

#[test]
fn compression_actually_compresses_repeats() {
    let c = case(DxvFormat::Dxt1, 64, 40, false, 7);
    assert!(c.packet.len() < c.texture.len() + 12);
}

#[test]
fn dxv2_headers_decode() {
    // DXV 2: 24-bit size plus a type byte. 0x80 = raw, 0x40 = DXT5, 0x20 = DXT1.
    let (w, h) = (32, 16);
    let (dxt1, _) = sizes(DxvFormat::Dxt1, w, h);
    let tex = texture(dxt1, 3, 8);
    let mut p = ((0x80 | 0x20) << 24 | tex.len() as u32).to_le_bytes().to_vec();
    p.extend_from_slice(&tex);
    let f = decode_frame(&p, w, h).unwrap();
    assert_eq!((f.format, &f.texture), (DxvFormat::Dxt1, &tex));

    // LZF: literal runs of up to 32 bytes, then one back-reference copying 8 bytes.
    let (dxt5, _) = sizes(DxvFormat::Dxt5, w, h);
    let tex = texture(dxt5 - 8, 4, 16);
    let mut body = Vec::new();
    for chunk in tex.chunks(32) {
        body.push((chunk.len() - 1) as u8);
        body.extend_from_slice(chunk);
    }
    // len = 2 + (s >> 5) = 8 → s >> 5 = 6; off = ((s & 0x1f) << 8) + 1 + next = 16.
    body.extend_from_slice(&[6 << 5, 15]);
    body.extend_from_slice(&[0, 0, 0]); // trailing bytes the LZF loop never reads
    let mut p = ((0x40 | 0x02) << 24 | body.len() as u32).to_le_bytes().to_vec();
    p.extend_from_slice(&body);
    let f = decode_frame(&p, w, h).unwrap();
    assert_eq!(f.format, DxvFormat::Dxt5);
    assert_eq!(&f.texture[..tex.len()], &tex[..]);
    assert_eq!(&f.texture[tex.len()..], &tex[tex.len() - 16..tex.len() - 8]);
}

#[test]
fn corrupt_packets_fail_cleanly() {
    for format in FORMATS {
        let c = case(format, 64, 40, false, 9);
        // Truncations and bit flips must error or decode, never panic.
        for cut in [0, 4, 11, 12, 20, c.packet.len() / 2, c.packet.len() - 1] {
            let _ = decode_frame(&c.packet[..cut], 64, 40);
        }
        let mut p = c.packet.clone();
        for i in (12..p.len()).step_by(7) {
            p[i] ^= 0xA5;
        }
        let _ = decode_frame(&p, 64, 40);
        let mut p = c.packet.clone();
        let n = p.len();
        p[8..12].copy_from_slice(&((n - 12 + 1) as u32).to_le_bytes());
        assert!(decode_frame(&p, 64, 40).is_err(), "size mismatch must be rejected");
    }
}

/// Garbage opcode tables must fail cleanly, never panic.
#[test]
fn entropy_table_parser_survives_garbage() {
    let mut ops = [0u8; 64];
    for seed in 0..200u32 {
        let junk = texture(96, seed, 1);
        let mut gb = Reader::new(&junk);
        let mut bytes = vec![2u8];
        bytes.extend_from_slice(&junk);
        let mut gb2 = Reader::new(&bytes);
        let _ = decompress_opcodes(&mut gb2, &mut ops);
        let mut table = [0u32; 256];
        let _ = fill_ltable(&mut gb, &mut table);
    }
}

/// Runs FFmpeg's DXV decoder on a movie and returns raw frames in `pix_fmt`.
fn ffmpeg_decode(path: &Path, pix_fmt: &str) -> Vec<u8> {
    let out = Command::new(tool_path("ffmpeg"))
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-f", "rawvideo", "-pix_fmt", pix_fmt, "-"])
        .output()
        .unwrap();
    assert!(out.status.success(), "ffmpeg failed: {}", String::from_utf8_lossy(&out.stderr));
    out.stdout
}

/// FFmpeg's major version, from `ffmpeg -version` ("ffmpeg version 6.1.1-3ubuntu5", "n7.1").
fn ffmpeg_major() -> Option<u32> {
    let out = Command::new(tool_path("ffmpeg")).arg("-version").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let v = text.split_whitespace().nth(2)?.trim_start_matches('n');
    v.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

fn max_diff(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0)
}

/// Our packets in our QuickTime files decode in FFmpeg to the same pixels our CPU
/// fallback (and so the GPU) produces.
#[test]
fn matches_ffmpeg_decoder() {
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }
    let dir = std::env::temp_dir().join(format!("openvj-dxv-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let old_ffmpeg = ffmpeg_major().is_some_and(|v| v < 7);
    for format in FORMATS {
        // Heights are multiples of 16: FFmpeg before 7.0 padded the texture to 16 rows,
        // later versions (and Resolume) to 4.
        for (w, h) in [(64, 48), (60, 32)] {
            // Raw frames are a plain copy (covered by the round trip); FFmpeg 6 misreads them.
            for raw in [false] {
                let label = format!("{format:?} {w}x{h} raw={raw}");
                let frames: Vec<Case> = (0..3).map(|i| case_with(format, w, h, raw, i * 31 + w, false)).collect();
                let path = dir.join(format!("{format:?}-{w}-{raw}.mov"));
                let packets: Vec<Vec<u8>> = frames.iter().map(|c| c.packet.clone()).collect();
                write_test_mov(&path, b"DXD3", w as u16, h as u16, 30, &packets);
                assert!(is_dxv(&path));

                let decoded: Vec<DxvFrame> = packets.iter().map(|p| decode_frame(p, w, h).unwrap()).collect();
                let (w, h) = (w as usize, h as usize);
                match format {
                    DxvFormat::Dxt1 | DxvFormat::Dxt5 => {
                        let theirs = ffmpeg_decode(&path, "rgba");
                        for (i, f) in decoded.iter().enumerate() {
                            let ours = f.to_rgba();
                            let theirs = &theirs[i * w * h * 4..(i + 1) * w * h * 4];
                            // Compare premultiplied, so tiny alphas don't amplify rounding.
                            let pm = |px: &[u8]| -> Vec<u8> {
                                px.chunks(4)
                                    .flat_map(|p| {
                                        let a = p[3] as u32;
                                        [0, 1, 2].map(|c| (p[c] as u32 * a / 255) as u8).into_iter().chain([p[3]])
                                    })
                                    .collect()
                            };
                            // FFmpeg 6 returned DXV's premultiplied colours as if they were straight.
                            let (ours, theirs) = if format == DxvFormat::Dxt5 && old_ffmpeg {
                                (f.expand(false), theirs.to_vec())
                            } else {
                                (pm(&ours), pm(theirs))
                            };
                            let d = max_diff(&ours, &theirs);
                            assert!(d <= 4, "{label} frame {i}: differs from FFmpeg by {d}");
                        }
                    }
                    DxvFormat::YCoCg | DxvFormat::YCoCgAlpha => {
                        let alpha = format == DxvFormat::YCoCgAlpha;
                        let theirs = ffmpeg_decode(&path, if alpha { "yuva420p" } else { "yuv420p" });
                        let (hw, hh) = (w.div_ceil(2), h.div_ceil(2));
                        let frame_len = w * h * (1 + alpha as usize) + 2 * hw * hh;
                        assert_eq!(theirs.len(), frame_len * 3, "{label}");
                        // FFmpeg decodes whole chroma block rows only.
                        let chroma_rows = (f_ch(h) / 2 / 4 * 4).min(hh);
                        for (i, f) in decoded.iter().enumerate() {
                            let p = f.ycocg_planes();
                            let t = &theirs[i * frame_len..(i + 1) * frame_len];
                            let cw = f.coded_width as usize;
                            let (ty, rest) = t.split_at(w * h);
                            let (tu, rest) = rest.split_at(hw * hh);
                            let (tv, ta) = rest.split_at(hw * hh);
                            for y in 0..h {
                                assert_eq!(
                                    &p.y[y * cw..y * cw + w],
                                    &ty[y * w..(y + 1) * w],
                                    "{label} frame {i} Y row {y}"
                                );
                                if let Some(a) = &p.a {
                                    assert_eq!(
                                        &a[y * cw..y * cw + w],
                                        &ta[y * w..(y + 1) * w],
                                        "{label} frame {i} A row {y}"
                                    );
                                }
                            }
                            for y in 0..chroma_rows {
                                let row = |plane: &[u8]| plane[y * cw / 2..y * cw / 2 + hw].to_vec();
                                assert_eq!(row(&p.cg), tu[y * hw..(y + 1) * hw], "{label} frame {i} Cg row {y}");
                                assert_eq!(row(&p.co), tv[y * hw..(y + 1) * hw], "{label} frame {i} Co row {y}");
                            }
                        }
                    }
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

fn f_ch(h: usize) -> usize {
    h.next_multiple_of(4)
}

#[test]
fn decoder_follows_the_clock() {
    let dir = std::env::temp_dir().join(format!("openvj-dxv-play-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("play.mov");
    // Each frame's first texture byte is its index, so we can tell frames apart.
    let packets: Vec<Vec<u8>> = (0..10u8)
        .map(|i| {
            let (tex, _) = sizes(DxvFormat::Dxt1, 32, 16);
            let mut t = vec![0u8; tex];
            t[0] = i;
            packet(b"DXT1", true, &t)
        })
        .collect();
    write_test_mov(&path, b"DXD3", 32, 16, 10, &packets);
    let mut dec = DxvDecoder::open(&path).unwrap();
    assert_eq!(dec.format, DxvFormat::Dxt1);
    assert_eq!(dec.info.fps, 10.0);
    assert_eq!(dec.info.duration, Some(1.0));

    // The app polls every frame; the decoder converges on the clock's frame.
    let reaches = |dec: &mut DxvDecoder, t: f64, want: u8| {
        for _ in 0..500 {
            if dec.frame_at(t).is_some_and(|f| f.texture[0] == want) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        panic!("never showed frame {want} at t={t}");
    };
    reaches(&mut dec, 0.0, 0);
    assert!(dec.frame_at(0.05).is_none(), "same frame isn't returned twice");
    // Skipping ahead lands on the right frame, looping past the end.
    reaches(&mut dec, 0.55, 5);
    reaches(&mut dec, 1.25, 2);
    // A big jump back (speed change) recovers instead of stalling.
    reaches(&mut dec, 0.3, 3);
    drop(dec);
    let _ = std::fs::remove_dir_all(dir);
}

/// LSB-first bit writer.
#[derive(Default)]
struct Bits {
    out: Vec<u8>,
    n: usize,
}

impl Bits {
    fn push(&mut self, v: u32, bits: u32) {
        for i in 0..bits {
            if self.n.is_multiple_of(8) {
                self.out.push(0);
            }
            if (v >> i) & 1 != 0 {
                *self.out.last_mut().unwrap() |= 1 << (self.n % 8);
            }
            self.n += 1;
        }
    }
}

/// An entropy-coded opcode section (flag 2): a frequency table over `freqs` (symbol,
/// count; counts sum to 1024) followed by `payload` as the backwards-read bitstream.
/// Any payload decodes to *some* symbols, so FFmpeg's decoder is the judge of correctness.
fn entropy_ops(freqs: &[(usize, u32)], payload: &[u8]) -> Vec<u8> {
    let mut table = [0u32; 256];
    for &(s, f) in freqs {
        table[s] = f;
    }
    let last = freqs.iter().map(|f| f.0).max().unwrap();
    let mut bits = Bits::default();
    bits.push(2, 2);
    let (mut half, mut width, mut left) = (512u32, 10u32, 1024u32);
    for &v in &table[..=last] {
        bits.push(v, width);
        left -= v;
        if left < half {
            half >>= 1;
            width -= 1;
        }
    }
    bits.out.resize(bits.out.len() + 8, 0);
    // Find where the table ends the way a reader does, then append the bitstream there.
    let mut gb = Reader::new(&bits.out);
    let mut check = [0u32; 256];
    fill_ltable(&mut gb, &mut check).unwrap();
    assert_eq!(check, table);
    let mut out = bits.out[..gb.tell()].to_vec();
    let total = 4 + payload.len();
    out.extend_from_slice(&((total * 8) as u32).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

/// DXV HQ frames whose luma opcodes are entropy coded, as Resolume writes them.
#[test]
fn entropy_coded_opcodes_match_ffmpeg() {
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }
    let dir = std::env::temp_dir().join(format!("openvj-dxv-ans-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (w, h) = (64u32, 48u32);
    let (tex_size, ctex_size) = sizes(DxvFormat::YCoCg, w, h);
    let blocks = tex_size / 8;
    let tables: [&[(usize, u32)]; 4] =
        [&[(1, 512), (3, 512)], &[(1, 1000), (3, 24)], &[(1, 3), (3, 1021)], &[(1, 300), (2, 0), (3, 724)]];
    let mut packets = Vec::new();
    for (i, freqs) in tables.iter().enumerate() {
        // Ops 1 (repeat) and 3 (literal) only, so every decoded sequence is valid.
        let data = texture(8 * blocks, 100 + i as u32, 8);
        let payload = texture(blocks * 2 + 16, 200 + i as u32, 1);
        let ops = entropy_ops(freqs, &payload);
        let mut yo = Vec::new();
        yo.extend_from_slice(&((8 + data.len()) as u32).to_le_bytes());
        yo.extend_from_slice(&((blocks - 1) as u32).to_le_bytes());
        yo.extend_from_slice(&data);
        yo.extend(ops);
        let chroma = texture_chroma(ctex_size, i as u32);
        let mut chroma_fixed = chroma.clone();
        for b in chroma_fixed.chunks_mut(8) {
            if b[0] == b[1] {
                b[1] ^= 1;
            }
        }
        packets.push(packet(b"YCG6", false, &[yo, encode_cocg(&chroma_fixed)].concat()));
    }
    let path = dir.join("ans.mov");
    write_test_mov(&path, b"DXD3", w as u16, h as u16, 30, &packets);
    let theirs = ffmpeg_decode(&path, "yuv420p");
    let (w, h) = (w as usize, h as usize);
    let frame_len = w * h * 3 / 2;
    for (i, p) in packets.iter().enumerate() {
        let f = decode_frame(p, w as u32, h as u32).unwrap_or_else(|e| panic!("table {i}: {e}"));
        let ours = f.ycocg_planes();
        let t = &theirs[i * frame_len..];
        // Some blocks must be repeats and some literals, or the test proves little.
        let repeats = f.texture.chunks(8).zip(f.texture.chunks(8).skip(1)).filter(|(a, b)| a == b).count();
        if i == 0 {
            assert!(repeats > 10 && repeats < blocks - 10, "table {i}: {repeats} repeats");
        }
        assert!(ours.y == t[..w * h], "table {i}: luma differs from FFmpeg");
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// `cargo test -p openvj-media --release bench_ -- --ignored --nocapture`
/// Per-frame cost of our DXV path against FFmpeg decoding to RGBA (the old path).
#[test]
#[ignore]
fn bench_against_ffmpeg() {
    // 1088 rows: FFmpeg 6 needs heights padded to 16.
    let (w, h, n) = (1920u32, 1088u32, 60);
    let dir = std::env::temp_dir().join(format!("openvj-dxv-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for format in FORMATS {
        let packets: Vec<Vec<u8>> = (0..n).map(|i| case_with(format, w, h, false, i, false).packet).collect();
        let t = std::time::Instant::now();
        let mut uploaded = 0;
        for p in &packets {
            let f = decode_frame(p, w, h).unwrap();
            uploaded = f.texture.len() + f.chroma.len();
        }
        let ours = t.elapsed().as_secs_f64() * 1000.0 / n as f64;

        let path = dir.join("bench.mov");
        write_test_mov(&path, b"DXD3", w as u16, h as u16, 30, &packets);
        let t = std::time::Instant::now();
        let rgba = if ffmpeg_available() { ffmpeg_decode(&path, "rgba").len() } else { 0 };
        let theirs = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
        eprintln!(
            "{:<12} ours {ours:5.2} ms/frame, {:4.1} MB to GPU | FFmpeg {theirs:5.2} ms/frame, {:4.1} MB",
            format.name(),
            uploaded as f64 / 1e6,
            rgba as f64 / n as f64 / 1e6
        );
    }
    let _ = std::fs::remove_dir_all(dir);
}
