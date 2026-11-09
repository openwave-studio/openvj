//! Resolume DXV decoding to GPU-ready block-compressed textures.
//!
//! A DXV frame is a DXT / RGTC texture wrapped in a light LZ-style compression. Undoing
//! that wrapper is cheap; the expensive part, expanding blocks to pixels, is left to the
//! GPU, which samples BC formats natively. So a 1080p DXT1 frame goes to the GPU as 1 MB
//! instead of 8 MB of RGBA, and no FFmpeg process is involved.
//!
//! The bitstream handling is a port of FFmpeg's `libavcodec/dxv.c` (LGPL-2.1-or-later,
//! by Vittorio Giovara and Paul B Mahol), which openVJ's GPL-3.0-or-later licence allows.
//!
//! Variants:
//!
//! | DXV texture | Resolume name          | GPU layout                                        |
//! |-------------|------------------------|---------------------------------------------------|
//! | DXT1        | Normal quality         | BC1                                               |
//! | DXT5        | Normal quality + alpha | BC3, premultiplied alpha                          |
//! | YCG6        | High quality           | BC4 luma + half-size BC5 Co/Cg                    |
//! | YG10        | High quality + alpha   | BC5 luma/alpha + half-size BC5 Co/Cg              |
//!
//! DXV 2 files (an older header with LZF or raw payloads) decode as DXT1 / DXT5 too.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};

use crate::VideoInfo;
use crate::mov;
use crate::player::{FramePlayer, read_sample};

/// Sample description fourccs QuickTime uses for DXV.
const FOURCCS: [&[u8; 4]; 2] = [b"DXD3", b"DXDI"];

const fn be_tag(t: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*t)
}
const TAG_DXT1: u32 = be_tag(b"DXT1");
const TAG_DXT5: u32 = be_tag(b"DXT5");
const TAG_YCG6: u32 = be_tag(b"YCG6");
const TAG_YG10: u32 = be_tag(b"YG10");

/// How a decoded frame's textures are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DxvFormat {
    /// `texture` is BC1; alpha is always opaque.
    Dxt1,
    /// `texture` is BC3 with premultiplied alpha.
    Dxt5,
    /// `texture` is BC4 luma; `chroma` is BC5 (R = Co, G = Cg) at half size.
    YCoCg,
    /// `texture` is BC5 (R = luma, G = alpha); `chroma` is BC5 (R = Co, G = Cg) at half size.
    YCoCgAlpha,
}

impl DxvFormat {
    pub fn name(self) -> &'static str {
        match self {
            DxvFormat::Dxt1 => "DXV",
            DxvFormat::Dxt5 => "DXV alpha",
            DxvFormat::YCoCg => "DXV HQ",
            DxvFormat::YCoCgAlpha => "DXV HQ alpha",
        }
    }

    /// Bytes per 4×4 block of the main texture.
    fn block_bytes(self) -> usize {
        match self {
            DxvFormat::Dxt1 | DxvFormat::YCoCg => 8,
            DxvFormat::Dxt5 | DxvFormat::YCoCgAlpha => 16,
        }
    }
}

/// One decoded DXV frame, still block-compressed.
#[derive(Clone)]
pub struct DxvFrame {
    pub format: DxvFormat,
    /// Visible size.
    pub width: u32,
    pub height: u32,
    /// Size of `texture` in pixels; multiples of 4 (width of 16), at least the visible size.
    pub coded_width: u32,
    pub coded_height: u32,
    /// Blocks in row-major order.
    pub texture: Vec<u8>,
    /// BC5 Co/Cg blocks for the YCoCg formats, `coded_width / 2` × `chroma_height`; empty otherwise.
    pub chroma: Vec<u8>,
    pub chroma_height: u32,
}

impl std::fmt::Debug for DxvFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DxvFrame({:?} {}x{})", self.format, self.width, self.height)
    }
}

/// Byte reader mirroring FFmpeg's `bytestream2`: reads past the end yield zeros.
struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }
    fn left(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }
    fn tell(&self) -> usize {
        self.pos
    }
    fn seek(&mut self, pos: usize) {
        self.pos = pos.min(self.buf.len());
    }
    fn skip(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.buf.len());
    }
    fn u8(&mut self) -> u32 {
        if self.left() < 1 {
            self.pos = self.buf.len();
            return 0;
        }
        self.pos += 1;
        self.buf[self.pos - 1] as u32
    }
    fn peek_u8(&self) -> u32 {
        self.buf.get(self.pos).copied().unwrap_or(0) as u32
    }
    fn le16(&mut self) -> u32 {
        if self.left() < 2 {
            self.pos = self.buf.len();
            return 0;
        }
        self.pos += 2;
        u16::from_le_bytes([self.buf[self.pos - 2], self.buf[self.pos - 1]]) as u32
    }
    fn le32(&mut self) -> u32 {
        if self.left() < 4 {
            self.pos = self.buf.len();
            return 0;
        }
        self.pos += 4;
        u32::from_le_bytes(self.buf[self.pos - 4..self.pos].try_into().unwrap())
    }
}

#[inline]
fn rd32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
#[inline]
fn wr32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
#[inline]
fn rd16(b: &[u8], at: usize) -> u32 {
    u16::from_le_bytes([b[at], b[at + 1]]) as u32
}
#[inline]
fn wr16(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 2].copy_from_slice(&(v as u16).to_le_bytes());
}

fn invalid() -> anyhow::Error {
    anyhow!("invalid DXV data")
}

/// The 2-bit back-reference scheme shared by the DXT1 and DXT5 decompressors.
struct Ops {
    value: u32,
    state: u32,
    idx: usize,
}

impl Ops {
    /// FFmpeg's `CHECKPOINT(x)`: returns the next op and updates the back-reference distance.
    fn next(&mut self, gb: &mut Reader, x: usize, pos: usize) -> Result<u32> {
        if self.state == 0 {
            if gb.left() < 4 {
                return Err(invalid());
            }
            self.value = gb.le32();
            self.state = 16;
        }
        let op = self.value & 3;
        self.value >>= 2;
        self.state -= 1;
        match op {
            1 => self.idx = x,
            2 => {
                self.idx = (gb.u8() as usize + 2) * x;
                if self.idx > pos {
                    return Err(invalid());
                }
            }
            3 => {
                self.idx = (gb.le16() as usize + 0x102) * x;
                if self.idx > pos {
                    return Err(invalid());
                }
            }
            _ => {}
        }
        Ok(op)
    }
}

fn decompress_dxt1(gb: &mut Reader, tex: &mut [u8]) -> Result<()> {
    let words = tex.len() / 4;
    wr32(tex, 0, gb.le32());
    wr32(tex, 4, gb.le32());
    let mut pos = 2;
    let mut ops = Ops { value: 0, state: 0, idx: 0 };
    while pos + 2 <= words {
        let op = ops.next(gb, 2, pos)?;
        if op != 0 {
            for _ in 0..2 {
                let prev = rd32(tex, 4 * (pos - ops.idx));
                wr32(tex, 4 * pos, prev);
                pos += 1;
            }
        } else {
            for _ in 0..2 {
                let op = ops.next(gb, 2, pos)?;
                let prev = if op != 0 { rd32(tex, 4 * (pos - ops.idx)) } else { gb.le32() };
                wr32(tex, 4 * pos, prev);
                pos += 1;
            }
        }
    }
    Ok(())
}

/// Reads a run length continued in 16-bit steps while they're all ones.
fn long_count(gb: &mut Reader, mut n: u64) -> u64 {
    loop {
        let probe = gb.le16() as u64;
        n += probe;
        if probe != 0xFFFF || gb.left() == 0 {
            return n;
        }
    }
}

fn decompress_dxt5(gb: &mut Reader, tex: &mut [u8]) -> Result<()> {
    let words = tex.len() / 4;
    for i in 0..4 {
        wr32(tex, 4 * i, gb.le32());
    }
    let mut pos = 4usize;
    let mut run = 0u64;
    let mut ops = Ops { value: 0, state: 0, idx: 0 };
    let copy = |tex: &mut [u8], pos: &mut usize, back: usize| {
        let prev = rd32(tex, 4 * (*pos - back));
        wr32(tex, 4 * *pos, prev);
        *pos += 1;
    };

    while pos + 2 <= words {
        if run > 0 {
            run -= 1;
            copy(tex, &mut pos, 4);
            copy(tex, &mut pos, 4);
        } else {
            if gb.left() < 1 {
                return Err(invalid());
            }
            if ops.state == 0 {
                ops.value = gb.le32();
                ops.state = 16;
            }
            let op = ops.value & 3;
            ops.value >>= 2;
            ops.state -= 1;
            match op {
                0 => {
                    // Long copy of whole blocks.
                    let mut check = gb.u8() as u64 + 1;
                    if check == 256 {
                        check = long_count(gb, check);
                    }
                    while check > 0 && pos + 4 <= words {
                        for _ in 0..4 {
                            copy(tex, &mut pos, 4);
                        }
                        check -= 1;
                    }
                    continue;
                }
                1 => {
                    run = gb.u8() as u64;
                    if run == 255 {
                        run = long_count(gb, run);
                    }
                    copy(tex, &mut pos, 4);
                    copy(tex, &mut pos, 4);
                }
                2 => {
                    let idx = 8 + 4 * gb.le16() as usize;
                    if idx > pos || pos - idx + 2 > words {
                        return Err(invalid());
                    }
                    copy(tex, &mut pos, idx);
                    copy(tex, &mut pos, idx);
                }
                _ => {
                    for _ in 0..2 {
                        let v = gb.le32();
                        wr32(tex, 4 * pos, v);
                        pos += 1;
                    }
                }
            }
        }

        let op = ops.next(gb, 4, pos)?;
        if pos + 2 > words {
            return Err(invalid());
        }
        if op != 0 {
            if ops.idx > pos || pos - ops.idx + 2 > words {
                return Err(invalid());
            }
            copy(tex, &mut pos, ops.idx);
            copy(tex, &mut pos, ops.idx);
        } else {
            for _ in 0..2 {
                let op = ops.next(gb, 4, pos)?;
                if op != 0 && (ops.idx > pos || pos - ops.idx + 2 > words) {
                    return Err(invalid());
                }
                let v = if op != 0 { rd32(tex, 4 * (pos - ops.idx)) } else { gb.le32() };
                wr32(tex, 4 * pos, v);
                pos += 1;
            }
        }
    }
    Ok(())
}

/// LZF, used by DXV 2. Output beyond `tex` is dropped and missing output stays zero.
fn decompress_lzf(gb: &mut Reader, tex: &mut [u8]) -> Result<()> {
    let mut out: Vec<u8> = Vec::with_capacity(tex.len());
    while gb.left() > 2 {
        let s = gb.u8() as usize;
        if s < 32 {
            let n = s + 1;
            if gb.left() < n {
                return Err(invalid());
            }
            out.extend_from_slice(&gb.buf[gb.pos..gb.pos + n]);
            gb.skip(n);
        } else {
            let mut len = 2 + (s >> 5);
            let mut off = ((s & 0x1f) << 8) + 1;
            if len == 7 + 2 {
                len += gb.u8() as usize;
            }
            off += gb.u8() as usize;
            if off > out.len() {
                return Err(invalid());
            }
            let start = out.len() - off;
            for i in 0..len {
                out.push(out[start + i]);
            }
        }
        if out.len() > tex.len() + (1 << 16) {
            return Err(invalid());
        }
    }
    let n = out.len().min(tex.len());
    tex[..n].copy_from_slice(&out[..n]);
    Ok(())
}

fn decompress_raw(gb: &mut Reader, tex: &mut [u8]) -> Result<()> {
    if gb.left() < tex.len() {
        return Err(invalid());
    }
    tex.copy_from_slice(&gb.buf[gb.pos..gb.pos + tex.len()]);
    gb.skip(tex.len());
    Ok(())
}

// ---------------------------------------------------------------- YCoCg (DXV HQ)

#[derive(Clone, Copy, Default)]
struct OpEntry {
    next: i32,
    val1: u8,
    val2: u8,
}

/// Reads the symbol frequency table of an entropy-coded opcode stream.
fn fill_ltable(gb: &mut Reader, table: &mut [u32; 256]) -> Result<usize> {
    let (mut half, mut bits, mut left) = (512u32, 1023u32, 1024u32);
    let (mut rshift, mut lshift) = (10u32, 30i32);
    let mut counter = 0usize;
    let mut mask = gb.le32() >> 2;
    while left != 0 {
        if counter >= 256 {
            return Err(invalid());
        }
        let value = bits & mask;
        left = left.wrapping_sub(value);
        mask >>= rshift;
        lshift -= rshift as i32;
        table[counter] = value;
        counter += 1;
        if lshift < 16 {
            if gb.left() == 0 {
                return Err(invalid());
            }
            let input = gb.le16();
            mask = mask.wrapping_add(input << lshift);
            lshift += 16;
        }
        if left < half {
            half >>= 1;
            bits >>= 1;
            rshift = rshift.saturating_sub(1);
        }
    }
    while counter > 0 && table[counter - 1] == 0 {
        counter -= 1;
    }
    if counter == 0 {
        return Err(invalid());
    }
    table[counter..].fill(0);
    if lshift >= 16 {
        gb.pos = gb.pos.saturating_sub(2);
    }
    Ok(counter)
}

fn fill_optable(table0: &[u32; 256], table1: &mut [OpEntry; 1024], nb: usize) -> Result<()> {
    let mut table2 = [0u32; 256];
    table2[0] = table0[0];
    for i in 0..nb.saturating_sub(1) {
        table2[i + 1] = table0[i + 1].wrapping_add(table2[i]);
    }
    let mut k = 0usize;
    if table2[0] == 0 {
        loop {
            k += 1;
            if k >= 256 {
                return Err(invalid());
            }
            if table2[k] != 0 {
                break;
            }
        }
    }
    let mut x = 0usize;
    for j in 2u32..2 + 1024 {
        table1[x].val1 = k as u8;
        while k < 256 && j > table2[k] {
            k += 1;
        }
        x = x.wrapping_sub(383) & 0x3FF;
    }
    table2[..nb].copy_from_slice(&table0[..nb]);
    for e in table1.iter_mut() {
        let v0 = e.val1 as usize;
        let v1 = table2[v0];
        table2[v0] = v1.wrapping_add(1);
        if v1 == 0 {
            return Err(invalid());
        }
        let x = 31 - v1.leading_zeros();
        if x > 10 {
            return Err(invalid());
        }
        e.val2 = (10 - x) as u8;
        e.next = ((v1 << e.val2) as i32 - 1024) as i16 as i32;
    }
    Ok(())
}

fn get_opcodes(gb: &mut Reader, table: &[u32; 256], dst: &mut [u8], nb: usize) -> Result<()> {
    let mut optable = [OpEntry::default(); 1024];
    let buf = gb.buf;
    let src = &buf[gb.pos..];
    fill_optable(table, &mut optable, nb)?;
    let size_in_bits = gb.le32() as u64;
    let bytes = (size_in_bits + 7) >> 3;
    let endoffset = bytes as i64 - 4;
    if endoffset <= 0 || (gb.left() as i64) < endoffset {
        return Err(invalid());
    }
    let endoffset = endoffset as usize;
    let mut offset = endoffset;
    let mut next = rd32(src, endoffset);
    let rshift = (((size_in_bits & 0xFF) as u32).wrapping_sub(1) & 7) + 15;
    let mut lshift = 32 - rshift;
    let mut idx = ((next >> rshift) & 0x3FF) as usize;
    for d in dst.iter_mut() {
        let e = optable[idx];
        *d = e.val1;
        let val = e.val2 as u32;
        let sum = val + lshift;
        let x = (next << lshift) >> 1 >> (31 - val);
        let back = (sum >> 3) as usize;
        lshift = sum & 7;
        let i = x as i64 + e.next as i64;
        if !(0..1024).contains(&i) || back > offset {
            return Err(invalid());
        }
        idx = i as usize;
        offset -= back;
        next = rd32(src, offset);
    }
    gb.skip(bytes as usize - 4);
    Ok(())
}

/// Fills `dst` with opcodes (stored, constant, or entropy-coded) and returns the bytes used.
fn decompress_opcodes(gb: &mut Reader, dst: &mut [u8]) -> Result<usize> {
    let start = gb.tell();
    let flag = gb.peek_u8();
    match flag & 3 {
        0 => {
            gb.skip(1);
            if gb.left() < dst.len() {
                return Err(invalid());
            }
            dst.copy_from_slice(&gb.buf[gb.pos..gb.pos + dst.len()]);
            gb.skip(dst.len());
        }
        1 => {
            gb.skip(1);
            let v = gb.u8() as u8;
            dst.fill(v);
        }
        _ => {
            let mut table = [0u32; 256];
            let nb = fill_ltable(gb, &mut table)?;
            get_opcodes(gb, &table, dst, nb)?;
        }
    }
    Ok(gb.tell() - start)
}

#[inline]
fn hash16(v: u32) -> usize {
    (0x9E37_79B1u32.wrapping_mul(v & 0xFFFF) >> 24) as usize
}
#[inline]
fn hash24(v: u32) -> usize {
    (0x9E37_79B1u32.wrapping_mul(v & 0xFF_FFFF) >> 24) as usize
}

/// Decoding state of one interleaved RGTC channel chain.
struct Chain<'o> {
    ops: &'o [u8],
    oi: usize,
    state: i64,
    tab0: [Option<usize>; 256],
    tab1: [Option<usize>; 256],
}

impl<'o> Chain<'o> {
    fn new(ops: &'o [u8]) -> Self {
        Self { ops, oi: 0, state: 0, tab0: [None; 256], tab1: [None; 256] }
    }

    fn note0(&mut self, tex: &[u8], at: usize) {
        self.tab0[hash16(rd16(tex, at))] = Some(at);
    }
    fn note1(&mut self, tex: &[u8], at: usize) {
        self.tab1[hash24(rd32(tex, at + 2))] = Some(at + 2);
    }
    fn t0(&self, gb: &mut Reader) -> Result<usize> {
        self.tab0[gb.u8() as usize].ok_or_else(invalid)
    }
    fn t1(&self, gb: &mut Reader) -> Result<usize> {
        self.tab1[gb.u8() as usize].ok_or_else(invalid)
    }

    /// FFmpeg's `dxv_decompress_cgo`: writes one 8-byte RGTC block at `*dst`.
    /// `offset` is the distance to skip to reach the previous block of this chain.
    fn block(
        &mut self,
        gb: &mut Reader,
        tex: &mut [u8],
        tex_size: usize,
        dst: &mut usize,
        offset: usize,
    ) -> Result<()> {
        let d = *dst;
        let back = 8 + offset;
        let back_ref = |gb: &mut Reader| -> Result<usize> {
            let v = back * (gb.le16() as usize + 1);
            if v > d { Err(invalid()) } else { Ok(d - v) }
        };
        if self.state <= 0 {
            let opcode = *self.ops.get(self.oi).ok_or_else(invalid)?;
            self.oi += 1;
            if opcode == 0 {
                let mut v = gb.u8() as i64;
                if v == 255 {
                    loop {
                        if gb.left() == 0 {
                            return Err(invalid());
                        }
                        let o = gb.le16() as i64;
                        v += o;
                        if o != 0xFFFF {
                            break;
                        }
                    }
                }
                let (a, b) = (rd32(tex, d - back), rd32(tex, d - back + 4));
                wr32(tex, d, a);
                wr32(tex, d + 4, b);
                // FFmpeg jumps into the run-copy branch, which writes the same block again.
                self.state = v + 3;
            } else {
                match opcode {
                    1 => {
                        let (a, b) = (rd32(tex, d - back), rd32(tex, d - back + 4));
                        wr32(tex, d, a);
                        wr32(tex, d + 4, b);
                    }
                    2 => {
                        let p = back_ref(gb)?;
                        let (a, b) = (rd32(tex, p), rd32(tex, p + 4));
                        wr32(tex, d, a);
                        wr32(tex, d + 4, b);
                        self.note0(tex, d);
                        self.note1(tex, d);
                    }
                    3 => {
                        let (a, b) = (gb.le32(), gb.le32());
                        wr32(tex, d, a);
                        wr32(tex, d + 4, b);
                        self.note0(tex, d);
                        self.note1(tex, d);
                    }
                    4 => {
                        let t3 = self.t1(gb)?;
                        let v = gb.le16();
                        wr16(tex, d, v);
                        let v = rd16(tex, t3);
                        wr16(tex, d + 2, v);
                        tex[d + 4] = tex[t3 + 2];
                        let v = gb.le16();
                        wr16(tex, d + 5, v);
                        tex[d + 7] = gb.u8() as u8;
                        self.note0(tex, d);
                    }
                    5 => {
                        let t3 = self.t1(gb)?;
                        let v = gb.le16();
                        wr16(tex, d, v);
                        let v = gb.le16();
                        wr16(tex, d + 2, v);
                        tex[d + 4] = gb.u8() as u8;
                        let v = rd16(tex, t3);
                        wr16(tex, d + 5, v);
                        tex[d + 7] = tex[t3 + 2];
                        self.note0(tex, d);
                        self.note1(tex, d);
                    }
                    6 => {
                        let t0 = self.t1(gb)?;
                        let t1 = self.t1(gb)?;
                        let v = gb.le16();
                        wr16(tex, d, v);
                        let v = rd16(tex, t0);
                        wr16(tex, d + 2, v);
                        tex[d + 4] = tex[t0 + 2];
                        let v = rd16(tex, t1);
                        wr16(tex, d + 5, v);
                        tex[d + 7] = tex[t1 + 2];
                        self.note0(tex, d);
                    }
                    7 => {
                        let p = back_ref(gb)?;
                        let v = gb.le16();
                        wr16(tex, d, v);
                        let v = rd16(tex, p + 2);
                        wr16(tex, d + 2, v);
                        let v = rd32(tex, p + 4);
                        wr32(tex, d + 4, v);
                        self.note0(tex, d);
                        self.note1(tex, d);
                    }
                    8 => {
                        let t1 = self.t0(gb)?;
                        let v = rd16(tex, t1);
                        wr16(tex, d, v);
                        let v = gb.le16();
                        wr16(tex, d + 2, v);
                        let v = gb.le32();
                        wr32(tex, d + 4, v);
                        self.note1(tex, d);
                    }
                    9 => {
                        let t1 = self.t0(gb)?;
                        let t3 = self.t1(gb)?;
                        let v = rd16(tex, t1);
                        wr16(tex, d, v);
                        let v = rd16(tex, t3);
                        wr16(tex, d + 2, v);
                        tex[d + 4] = tex[t3 + 2];
                        let v = gb.le16();
                        wr16(tex, d + 5, v);
                        tex[d + 7] = gb.u8() as u8;
                        self.note1(tex, d);
                    }
                    10 => {
                        let t1 = self.t0(gb)?;
                        let t3 = self.t1(gb)?;
                        let v = rd16(tex, t1);
                        wr16(tex, d, v);
                        let v = gb.le16();
                        wr16(tex, d + 2, v);
                        tex[d + 4] = gb.u8() as u8;
                        let v = rd16(tex, t3);
                        wr16(tex, d + 5, v);
                        tex[d + 7] = tex[t3 + 2];
                        self.note1(tex, d);
                    }
                    11 => {
                        let t0 = self.t0(gb)?;
                        let t3 = self.t1(gb)?;
                        let t1 = self.t1(gb)?;
                        let v = rd16(tex, t0);
                        wr16(tex, d, v);
                        let v = rd16(tex, t3);
                        wr16(tex, d + 2, v);
                        tex[d + 4] = tex[t3 + 2];
                        let v = rd16(tex, t1);
                        wr16(tex, d + 5, v);
                        tex[d + 7] = tex[t1 + 2];
                    }
                    12 => {
                        let t1 = self.t0(gb)?;
                        let p = back_ref(gb)?;
                        let v = rd16(tex, t1);
                        wr16(tex, d, v);
                        let v = rd16(tex, p + 2);
                        wr16(tex, d + 2, v);
                        let v = rd32(tex, p + 4);
                        wr32(tex, d + 4, v);
                        self.note1(tex, d);
                    }
                    13 => {
                        let v = rd16(tex, d - back);
                        wr16(tex, d, v);
                        let v = gb.le16();
                        wr16(tex, d + 2, v);
                        let v = gb.le32();
                        wr32(tex, d + 4, v);
                        self.note1(tex, d);
                    }
                    14 => {
                        let t3 = self.t1(gb)?;
                        let v = rd16(tex, d - back);
                        wr16(tex, d, v);
                        let v = rd16(tex, t3);
                        wr16(tex, d + 2, v);
                        tex[d + 4] = tex[t3 + 2];
                        let v = gb.le16();
                        wr16(tex, d + 5, v);
                        tex[d + 7] = gb.u8() as u8;
                        self.note1(tex, d);
                    }
                    15 => {
                        let t3 = self.t1(gb)?;
                        let v = rd16(tex, d - back);
                        wr16(tex, d, v);
                        let v = gb.le16();
                        wr16(tex, d + 2, v);
                        tex[d + 4] = gb.u8() as u8;
                        let v = rd16(tex, t3);
                        wr16(tex, d + 5, v);
                        tex[d + 7] = tex[t3 + 2];
                        self.note1(tex, d);
                    }
                    16 => {
                        let t3 = self.t1(gb)?;
                        let t1 = self.t1(gb)?;
                        let v = rd16(tex, d - back);
                        wr16(tex, d, v);
                        let v = rd16(tex, t3);
                        wr16(tex, d + 2, v);
                        tex[d + 4] = tex[t3 + 2];
                        let v = rd16(tex, t1);
                        wr16(tex, d + 5, v);
                        tex[d + 7] = tex[t1 + 2];
                    }
                    17 => {
                        let p = back_ref(gb)?;
                        let v = rd16(tex, d - back);
                        wr16(tex, d, v);
                        let v = rd16(tex, p + 2);
                        wr16(tex, d + 2, v);
                        let v = rd32(tex, p + 4);
                        wr32(tex, d + 4, v);
                        self.note1(tex, d);
                    }
                    _ => {}
                }
            }
        } else {
            let (a, b) = (rd32(tex, d - back), rd32(tex, d - back + 4));
            wr32(tex, d, a);
            wr32(tex, d + 4, b);
            self.state -= 1;
        }
        if d + 8 > tex_size {
            return Err(invalid());
        }
        *dst = d + 8;
        Ok(())
    }
}

/// Two interleaved RGTC channels (BC5): the first 8 bytes of each 16-byte block, then the second.
fn decompress_cocg(gb: &mut Reader, tex: &mut [u8], tex_size: usize, ops0: &mut [u8], ops1: &mut [u8]) -> Result<()> {
    let op_offset = gb.le32() as usize;
    let size0 = gb.le32() as usize;
    let size1 = gb.le32() as usize;
    let data_start = gb.tell();
    if op_offset < 12 || op_offset - 12 > gb.left() {
        return Err(invalid());
    }
    gb.skip(op_offset - 12);
    if size0 > ops0.len() {
        return Err(invalid());
    }
    let skip0 = decompress_opcodes(gb, &mut ops0[..size0])?;
    if size1 > ops1.len() {
        return Err(invalid());
    }
    let skip1 = decompress_opcodes(gb, &mut ops1[..size1])?;
    gb.seek(data_start);

    for i in 0..4 {
        wr32(tex, 4 * i, gb.le32());
    }
    let mut a = Chain::new(&ops0[..size0]);
    let mut b = Chain::new(&ops1[..size1]);
    a.note0(tex, 0);
    a.note1(tex, 0);
    b.note0(tex, 8);
    b.note1(tex, 8);
    let mut dst = 16;
    while dst + 10 < tex_size {
        a.block(gb, tex, tex_size, &mut dst, 8)?;
        b.block(gb, tex, tex_size, &mut dst, 8)?;
    }
    gb.seek(data_start - 12 + op_offset + skip0 + skip1);
    Ok(())
}

/// One RGTC channel (BC4).
fn decompress_yo(gb: &mut Reader, tex: &mut [u8], tex_size: usize, ops: &mut [u8]) -> Result<()> {
    let op_offset = gb.le32() as usize;
    let size = gb.le32() as usize;
    let data_start = gb.tell();
    if op_offset < 8 || op_offset - 8 > gb.left() {
        return Err(invalid());
    }
    gb.skip(op_offset - 8);
    if size > ops.len() {
        return Err(invalid());
    }
    let skip = decompress_opcodes(gb, &mut ops[..size])?;
    gb.seek(data_start);

    wr32(tex, 0, gb.le32());
    wr32(tex, 4, gb.le32());
    let mut chain = Chain::new(&ops[..size]);
    chain.note0(tex, 0);
    chain.note1(tex, 0);
    let mut dst = 8;
    while dst < tex_size {
        chain.block(gb, tex, tex_size, &mut dst, 0)?;
    }
    gb.seek(data_start + op_offset + skip - 8);
    Ok(())
}

// ---------------------------------------------------------------- frames

/// Room for the decompressors' overshoot (FFmpeg pads its buffers the same way).
const PAD: usize = 64;

/// Decodes one DXV packet of a `width`×`height` video into block-compressed textures.
pub fn decode_frame(packet: &[u8], width: u32, height: u32) -> Result<DxvFrame> {
    if width == 0 || height == 0 || width > 16384 || height > 16384 {
        bail!("invalid DXV frame size {width}x{height}");
    }
    let cw = width.next_multiple_of(16) as usize;
    let ch = height.next_multiple_of(4) as usize;
    if ch / 2 / 4 < 1 {
        bail!("DXV frame too small");
    }
    let mut gb = Reader::new(packet);
    if gb.left() < 12 {
        bail!("DXV packet too short");
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Method {
        Dxt1,
        Dxt5,
        Ycg6,
        Yg10,
        Lzf,
        Raw,
    }
    let tag = gb.le32();
    let (format, mut method, new_header) = match tag {
        TAG_DXT1 => (DxvFormat::Dxt1, Method::Dxt1, true),
        TAG_DXT5 => (DxvFormat::Dxt5, Method::Dxt5, true),
        TAG_YCG6 => (DxvFormat::YCoCg, Method::Ycg6, true),
        TAG_YG10 => (DxvFormat::YCoCgAlpha, Method::Yg10, true),
        _ => {
            // DXV 2: the tag is a 24-bit size plus a type byte.
            let old = tag >> 24;
            let major = (old & 0x0F) as i32 - 1;
            let method = if old & 0x80 != 0 { Method::Raw } else { Method::Lzf };
            let format = if old & 0x40 != 0 {
                DxvFormat::Dxt5
            } else if old & 0x20 != 0 || major == 1 {
                DxvFormat::Dxt1
            } else {
                bail!("unsupported DXV header {tag:#010x}");
            };
            (format, method, false)
        }
    };
    let size = if new_header {
        let _major = gb.u8();
        let _minor = gb.u8();
        // The encoder stores the texture as-is when compression doesn't pay off.
        if gb.u8() != 0 {
            method = Method::Raw;
        }
        gb.skip(1);
        gb.le32() as usize
    } else {
        (tag & 0x00FF_FFFF) as usize
    };
    if size != gb.left() {
        bail!("incomplete DXV frame (header says {size} bytes, {} left)", gb.left());
    }

    // Sizes exactly as FFmpeg computes them; they decide where decompression stops.
    let blocks = (cw / 4) * ch / 4;
    let tex_size = blocks * format.block_bytes();
    let ctex_size =
        if matches!(format, DxvFormat::YCoCg | DxvFormat::YCoCgAlpha) { cw / 2 / 4 * ch / 2 / 4 * 16 } else { 0 };
    let mut tex = vec![0u8; tex_size + PAD];
    let mut ctex = vec![0u8; ctex_size + PAD];

    let t = &mut tex[..tex_size];
    match method {
        Method::Dxt1 => decompress_dxt1(&mut gb, t)?,
        Method::Dxt5 => decompress_dxt5(&mut gb, t)?,
        Method::Lzf => decompress_lzf(&mut gb, t)?,
        Method::Raw => {
            decompress_raw(&mut gb, t)?;
            // FFmpeg reads only the first plane of raw YCoCg frames; take the chroma when present.
            if ctex_size > 0 && gb.left() >= ctex_size {
                decompress_raw(&mut gb, &mut ctex[..ctex_size])?;
            }
        }
        Method::Ycg6 | Method::Yg10 => {
            let mut ops = [cw * ch / 16, cw * ch / 32, cw * ch / 32, cw * ch / 16].map(|n| vec![0u8; n]);
            let [o0, o1, o2, o3] = &mut ops;
            if method == Method::Ycg6 {
                decompress_yo(&mut gb, &mut tex, tex_size, o0)?;
            } else {
                decompress_cocg(&mut gb, &mut tex, tex_size, o0, o3)?;
            }
            decompress_cocg(&mut gb, &mut ctex, ctex_size, o1, o2)?;
        }
    }
    tex.truncate(tex_size);

    // The chroma plane can end in a partial block row; round it up so it's a valid texture.
    let (chroma, chroma_height) = if ctex_size > 0 {
        let row = cw / 8 * 16;
        let rows = ctex_size.div_ceil(row);
        ctex.resize(rows * row, 0);
        (ctex, rows as u32 * 4)
    } else {
        (Vec::new(), 0)
    };
    Ok(DxvFrame {
        format,
        width,
        height,
        coded_width: cw as u32,
        coded_height: ch as u32,
        texture: tex,
        chroma,
        chroma_height,
    })
}

// ---------------------------------------------------------------- CPU fallback

/// A BC1 colour block. `four_color` forces the 4-colour mode, as BC2/BC3 do.
pub(crate) fn bc_color_block(block: &[u8], out: &mut [[u8; 4]; 16], four_color: bool) {
    let c0 = rd16(block, 0);
    let c1 = rd16(block, 2);
    let expand = |c: u32| {
        let r = (c >> 11) & 31;
        let g = (c >> 5) & 63;
        let b = c & 31;
        [(r * 527 + 23) >> 6, (g * 259 + 33) >> 6, (b * 527 + 23) >> 6]
    };
    let (a, b) = (expand(c0), expand(c1));
    let mut pal = [[0u8; 4]; 4];
    pal[0] = [a[0] as u8, a[1] as u8, a[2] as u8, 255];
    pal[1] = [b[0] as u8, b[1] as u8, b[2] as u8, 255];
    if four_color || c0 > c1 {
        for i in 0..3 {
            pal[2][i] = ((2 * a[i] + b[i]) / 3) as u8;
            pal[3][i] = ((a[i] + 2 * b[i]) / 3) as u8;
        }
        pal[2][3] = 255;
        pal[3][3] = 255;
    } else {
        for i in 0..3 {
            pal[2][i] = ((a[i] + b[i]) / 2) as u8;
        }
        pal[2][3] = 255;
        pal[3] = [0, 0, 0, 255];
    }
    let bits = rd32(block, 4);
    for (i, px) in out.iter_mut().enumerate() {
        *px = pal[((bits >> (2 * i)) & 3) as usize];
    }
}

pub(crate) fn bc_alpha_block(block: &[u8], out: &mut [u8; 16]) {
    let (a0, a1) = (block[0] as u32, block[1] as u32);
    let mut pal = [a0, a1, 0, 0, 0, 0, 0, 255];
    if a0 > a1 {
        for i in 1..7 {
            pal[i + 1] = ((7 - i as u32) * a0 + i as u32 * a1) / 7;
        }
    } else {
        for i in 1..5 {
            pal[i + 1] = ((5 - i as u32) * a0 + i as u32 * a1) / 5;
        }
    }
    let bits = block[2..8].iter().rev().fold(0u64, |acc, &b| (acc << 8) | b as u64);
    for (i, v) in out.iter_mut().enumerate() {
        *v = pal[((bits >> (3 * i)) & 7) as usize] as u8;
    }
}

/// Calls `f(block, x, y)` for each block of a `w`×`h` plane, in row-major order.
pub(crate) fn expand_blocks(
    data: &[u8],
    w: usize,
    h: usize,
    block_bytes: usize,
    mut f: impl FnMut(&[u8], usize, usize),
) {
    let bw = w / 4;
    for by in 0..h / 4 {
        for bx in 0..bw {
            let at = (by * bw + bx) * block_bytes;
            f(&data[at..at + block_bytes], bx * 4, by * 4);
        }
    }
}

impl DxvFrame {
    /// Expands the frame to straight-alpha RGBA8 at its visible size, on the CPU.
    /// For GPUs without BC texture support, and for tests.
    pub fn to_rgba(&self) -> Vec<u8> {
        self.expand(true)
    }

    /// Like [`DxvFrame::to_rgba`], optionally keeping DXT5's premultiplied colours.
    pub(crate) fn expand(&self, unpremultiply: bool) -> Vec<u8> {
        let (cw, ch) = (self.coded_width as usize, self.coded_height as usize);
        let mut full = vec![0u8; cw * ch * 4];
        let put = |full: &mut [u8], x: usize, y: usize, i: usize, px: [u8; 4]| {
            let at = ((y + i / 4) * cw + x + i % 4) * 4;
            full[at..at + 4].copy_from_slice(&px);
        };
        match self.format {
            DxvFormat::Dxt1 => expand_blocks(&self.texture, cw, ch, 8, |b, x, y| {
                let mut px = [[0u8; 4]; 16];
                bc_color_block(b, &mut px, false);
                for (i, p) in px.into_iter().enumerate() {
                    put(&mut full, x, y, i, [p[0], p[1], p[2], 255]);
                }
            }),
            DxvFormat::Dxt5 => expand_blocks(&self.texture, cw, ch, 16, |b, x, y| {
                let mut alpha = [0u8; 16];
                bc_alpha_block(&b[..8], &mut alpha);
                let mut px = [[0u8; 4]; 16];
                bc_color_block(&b[8..], &mut px, true);
                for (i, p) in px.into_iter().enumerate() {
                    let a = alpha[i];
                    let un = |c: u8| {
                        if !unpremultiply {
                            c
                        } else if a == 0 {
                            0
                        } else {
                            ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8
                        }
                    };
                    put(&mut full, x, y, i, [un(p[0]), un(p[1]), un(p[2]), a]);
                }
            }),
            DxvFormat::YCoCg | DxvFormat::YCoCgAlpha => {
                let planes = self.ycocg_planes();
                for y in 0..ch {
                    for x in 0..cw {
                        let luma = planes.y[y * cw + x] as f32 / 255.0;
                        let ci = (y / 2).min(planes.chroma_h - 1) * (cw / 2) + x / 2;
                        let co = planes.co[ci] as f32 / 255.0 - 0.5;
                        let cg = planes.cg[ci] as f32 / 255.0 - 0.5;
                        let rgb = [luma + co - cg, luma + cg, luma - co - cg];
                        let a = planes.a.as_ref().map_or(255, |a| a[y * cw + x]);
                        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                        let at = (y * cw + x) * 4;
                        full[at..at + 4].copy_from_slice(&[q(rgb[0]), q(rgb[1]), q(rgb[2]), a]);
                    }
                }
            }
        }
        crop(&full, cw, self.width as usize, self.height as usize, 4)
    }

    /// Expands the YCoCg formats to 8-bit planes, coded size (luma) and half size (chroma).
    pub(crate) fn ycocg_planes(&self) -> YCoCgPlanes {
        let (cw, ch) = (self.coded_width as usize, self.coded_height as usize);
        let mut y = vec![0u8; cw * ch];
        let mut a = (self.format == DxvFormat::YCoCgAlpha).then(|| vec![0u8; cw * ch]);
        let bytes = self.format.block_bytes();
        expand_blocks(&self.texture, cw, ch, bytes, |b, x0, y0| {
            let mut v = [0u8; 16];
            bc_alpha_block(&b[..8], &mut v);
            for (i, v) in v.into_iter().enumerate() {
                y[(y0 + i / 4) * cw + x0 + i % 4] = v;
            }
            if let Some(a) = a.as_mut() {
                let mut v = [0u8; 16];
                bc_alpha_block(&b[8..], &mut v);
                for (i, v) in v.into_iter().enumerate() {
                    a[(y0 + i / 4) * cw + x0 + i % 4] = v;
                }
            }
        });
        let (hw, hh) = (cw / 2, self.chroma_height as usize);
        let mut co = vec![0u8; hw * hh];
        let mut cg = vec![0u8; hw * hh];
        expand_blocks(&self.chroma, hw, hh, 16, |b, x0, y0| {
            let mut v = [0u8; 16];
            bc_alpha_block(&b[..8], &mut v);
            for (i, v) in v.into_iter().enumerate() {
                co[(y0 + i / 4) * hw + x0 + i % 4] = v;
            }
            bc_alpha_block(&b[8..], &mut v);
            for (i, v) in v.into_iter().enumerate() {
                cg[(y0 + i / 4) * hw + x0 + i % 4] = v;
            }
        });
        YCoCgPlanes { y, a, co, cg, chroma_h: hh.max(1) }
    }
}

pub(crate) struct YCoCgPlanes {
    pub y: Vec<u8>,
    pub a: Option<Vec<u8>>,
    pub co: Vec<u8>,
    pub cg: Vec<u8>,
    pub chroma_h: usize,
}

pub(crate) fn crop(src: &[u8], stride_px: usize, w: usize, h: usize, bpp: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(w * h * bpp);
    for row in src.chunks(stride_px * bpp).take(h) {
        out.extend_from_slice(&row[..w * bpp]);
    }
    out
}

// ---------------------------------------------------------------- player

/// True if `path` is a QuickTime movie whose first video track is DXV.
pub fn is_dxv(path: &Path) -> bool {
    mov::read_video_track(path).is_ok_and(|t| FOURCCS.contains(&&t.fourcc))
}

/// The DXV variant of a movie, from its first frame's header; `None` if it isn't DXV.
pub fn probe(path: &Path) -> Option<DxvFormat> {
    let track = mov::read_video_track(path).ok().filter(|t| FOURCCS.contains(&&t.fourcc))?;
    let mut file = File::open(path).ok()?;
    let mut head = [0u8; 4];
    file.seek(SeekFrom::Start(track.samples.first()?.0)).ok()?;
    file.read_exact(&mut head).ok()?;
    match u32::from_le_bytes(head) {
        TAG_DXT1 => Some(DxvFormat::Dxt1),
        TAG_DXT5 => Some(DxvFormat::Dxt5),
        TAG_YCG6 => Some(DxvFormat::YCoCg),
        TAG_YG10 => Some(DxvFormat::YCoCgAlpha),
        tag if tag >> 24 & 0x40 != 0 => Some(DxvFormat::Dxt5),
        _ => Some(DxvFormat::Dxt1),
    }
}

/// Plays a DXV movie as block-compressed frames, looping; see [`crate::player`].
pub struct DxvDecoder {
    pub info: VideoInfo,
    pub format: DxvFormat,
    player: FramePlayer<DxvFrame>,
}

impl DxvDecoder {
    /// Opens a DXV movie. Fails if it isn't DXV or its first frame doesn't decode, so
    /// callers can fall back to FFmpeg.
    pub fn open(path: &Path) -> Result<Self> {
        let track = mov::read_video_track(path)?;
        if !FOURCCS.contains(&&track.fourcc) {
            bail!("not a DXV movie ({})", String::from_utf8_lossy(&track.fourcc));
        }
        if track.samples.is_empty() {
            bail!("DXV movie has no frames");
        }
        let mut file = File::open(path).with_context(|| format!("could not open {}", path.display()))?;
        let mut buf = Vec::new();
        read_sample(&mut file, track.samples[0], &mut buf)?;
        let first = decode_frame(&buf, track.width, track.height)?;
        let info = track.video_info();
        let format = first.format;
        let player = FramePlayer::spawn("DXV", track, file, first, decode_frame)?;
        Ok(Self { info, format, player })
    }

    /// Returns the frame for clip time `t` (seconds) if it differs from the last one returned.
    pub fn frame_at(&mut self, t: f64) -> Option<DxvFrame> {
        self.player.frame_at(t)
    }
}

#[cfg(test)]
pub(crate) mod tests;
