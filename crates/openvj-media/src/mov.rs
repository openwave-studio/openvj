//! A minimal QuickTime / MP4 reader: finds the first video track and its sample table,
//! so compressed frames can be read straight from the file by index.
//!
//! This is enough for intra-only codecs such as DXV and HAP, where every frame decodes on
//! its own and random access is just "read `size` bytes at `offset`".

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};

/// The first video track of a QuickTime / MP4 file.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoTrack {
    /// Sample description fourcc, e.g. `*b"DXD3"`.
    pub fourcc: [u8; 4],
    pub width: u32,
    pub height: u32,
    /// Units per second for sample durations.
    pub timescale: u32,
    /// Sum of all sample durations, in `timescale` units.
    pub total_duration: u64,
    /// File offset and byte size of each frame, in decode order.
    pub samples: Vec<(u64, u32)>,
}

impl VideoTrack {
    pub fn fps(&self) -> f64 {
        let n = self.samples.len() as f64;
        if self.total_duration == 0 || self.timescale == 0 || n == 0.0 {
            return 30.0;
        }
        let fps = n * self.timescale as f64 / self.total_duration as f64;
        if (1.0..=240.0).contains(&fps) { fps } else { 30.0 }
    }

    pub fn duration(&self) -> f64 {
        self.samples.len() as f64 / self.fps()
    }

    /// Stream information for the clip grid and player.
    pub fn video_info(&self) -> crate::VideoInfo {
        crate::VideoInfo {
            width: self.width,
            height: self.height,
            fps: self.fps(),
            duration: Some(self.duration()),
            frames: Some(self.samples.len() as u64),
            decoder: None,
        }
    }
}

/// Reads the sample table of the first video track in a `.mov` / `.mp4` file.
pub fn read_video_track(path: &Path) -> Result<VideoTrack> {
    let mut file = File::open(path).with_context(|| format!("could not open {}", path.display()))?;
    let len = file.metadata()?.len();
    let moov = find_top_level(&mut file, len, b"moov")?.ok_or_else(|| anyhow!("no moov atom"))?;
    for trak in children(&moov).filter(|(t, _)| t == b"trak") {
        if let Some(track) = parse_trak(trak.1)? {
            return Ok(track);
        }
    }
    bail!("no video track in {}", path.display())
}

/// Scans the top-level atoms and returns the body of the first one named `name`.
fn find_top_level(file: &mut File, len: u64, name: &[u8; 4]) -> Result<Option<Vec<u8>>> {
    let mut pos = 0u64;
    while pos + 8 <= len {
        file.seek(SeekFrom::Start(pos))?;
        let mut head = [0u8; 16];
        file.read_exact(&mut head[..8])?;
        let mut size = u32::from_be_bytes(head[..4].try_into().unwrap()) as u64;
        let kind: [u8; 4] = head[4..8].try_into().unwrap();
        let mut header = 8u64;
        if size == 1 {
            file.read_exact(&mut head[8..16])?;
            size = u64::from_be_bytes(head[8..16].try_into().unwrap());
            header = 16;
        } else if size == 0 {
            size = len - pos;
        }
        if size < header || pos + size > len {
            bail!("corrupt atom {:?} at {pos}", String::from_utf8_lossy(&kind));
        }
        if &kind == name {
            let body_len = usize::try_from(size - header)?;
            if body_len > 512 << 20 {
                bail!("{} atom is too large", String::from_utf8_lossy(name));
            }
            let mut body = vec![0u8; body_len];
            file.read_exact(&mut body)?;
            return Ok(Some(body));
        }
        pos += size;
    }
    Ok(None)
}

/// Iterates `(type, body)` over the atoms packed in `data`. Stops at the first malformed one.
fn children(data: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
    let mut rest = data;
    std::iter::from_fn(move || {
        if rest.len() < 8 {
            return None;
        }
        let mut size = u32::from_be_bytes(rest[..4].try_into().unwrap()) as u64;
        let kind: [u8; 4] = rest[4..8].try_into().unwrap();
        let mut header = 8;
        if size == 1 {
            if rest.len() < 16 {
                return None;
            }
            size = u64::from_be_bytes(rest[8..16].try_into().unwrap());
            header = 16;
        } else if size == 0 {
            size = rest.len() as u64;
        }
        if size < header as u64 || size > rest.len() as u64 {
            return None;
        }
        let (atom, tail) = rest.split_at(size as usize);
        rest = tail;
        Some((kind, &atom[header..]))
    })
}

fn child<'a>(data: &'a [u8], name: &[u8; 4]) -> Option<&'a [u8]> {
    children(data).find(|(t, _)| t == name).map(|(_, b)| b)
}

/// Big-endian reads that fail instead of panicking on short input.
struct Be<'a>(&'a [u8]);

impl Be<'_> {
    fn u32(&self, at: usize) -> Result<u32> {
        let b = self.0.get(at..at + 4).ok_or_else(|| anyhow!("truncated sample table"))?;
        Ok(u32::from_be_bytes(b.try_into().unwrap()))
    }
    fn u64(&self, at: usize) -> Result<u64> {
        let b = self.0.get(at..at + 8).ok_or_else(|| anyhow!("truncated sample table"))?;
        Ok(u64::from_be_bytes(b.try_into().unwrap()))
    }
    fn u16(&self, at: usize) -> Result<u16> {
        let b = self.0.get(at..at + 2).ok_or_else(|| anyhow!("truncated sample table"))?;
        Ok(u16::from_be_bytes(b.try_into().unwrap()))
    }
}

/// Parses a `trak` atom; `None` if it isn't a video track.
fn parse_trak(trak: &[u8]) -> Result<Option<VideoTrack>> {
    let Some(mdia) = child(trak, b"mdia") else { return Ok(None) };
    let Some(hdlr) = child(mdia, b"hdlr") else { return Ok(None) };
    // version/flags (4), pre_defined / component type (4), handler subtype (4)
    if hdlr.get(8..12) != Some(b"vide") {
        return Ok(None);
    }
    let mdhd = Be(child(mdia, b"mdhd").ok_or_else(|| anyhow!("no mdhd atom"))?);
    let timescale = if mdhd.0.first() == Some(&1) { mdhd.u32(20)? } else { mdhd.u32(12)? };
    let stbl = child(mdia, b"minf").and_then(|m| child(m, b"stbl")).ok_or_else(|| anyhow!("no stbl atom"))?;

    // Sample description: the first entry's fourcc and coded size.
    let stsd = Be(child(stbl, b"stsd").ok_or_else(|| anyhow!("no stsd atom"))?);
    let entry = stsd.0.get(8..).ok_or_else(|| anyhow!("empty stsd atom"))?;
    let fourcc: [u8; 4] = entry.get(4..8).ok_or_else(|| anyhow!("empty stsd atom"))?.try_into().unwrap();
    let entry = Be(entry);
    let (width, height) = (entry.u16(32)? as u32, entry.u16(34)? as u32);

    // Sample sizes.
    let stsz = Be(child(stbl, b"stsz").ok_or_else(|| anyhow!("no stsz atom"))?);
    let uniform = stsz.u32(4)?;
    let count = stsz.u32(8)? as usize;
    let sizes: Vec<u32> = if uniform != 0 {
        vec![uniform; count]
    } else {
        if stsz.0.len() < 12 + count * 4 {
            bail!("truncated stsz atom");
        }
        (0..count).map(|i| stsz.u32(12 + i * 4)).collect::<Result<_>>()?
    };

    // Chunk offsets.
    let chunks: Vec<u64> = if let Some(stco) = child(stbl, b"stco") {
        let stco = Be(stco);
        let n = stco.u32(4)? as usize;
        (0..n).map(|i| stco.u32(8 + i * 4).map(u64::from)).collect::<Result<_>>()?
    } else if let Some(co64) = child(stbl, b"co64") {
        let co64 = Be(co64);
        let n = co64.u32(4)? as usize;
        (0..n).map(|i| co64.u64(8 + i * 8)).collect::<Result<_>>()?
    } else {
        bail!("no chunk offsets");
    };

    // Samples per chunk: (first chunk, 1-based; samples in each chunk from there on).
    let stsc = Be(child(stbl, b"stsc").ok_or_else(|| anyhow!("no stsc atom"))?);
    let runs: Vec<(u32, u32)> = (0..stsc.u32(4)? as usize)
        .map(|i| Ok((stsc.u32(8 + i * 12)?, stsc.u32(12 + i * 12)?)))
        .collect::<Result<_>>()?;

    let mut samples = Vec::with_capacity(count);
    let mut next = sizes.iter();
    'chunks: for (ci, &chunk_offset) in chunks.iter().enumerate() {
        let chunk_no = ci as u32 + 1;
        let per_chunk = runs.iter().rev().find(|(first, _)| *first <= chunk_no).map_or(1, |r| r.1);
        let mut offset = chunk_offset;
        for _ in 0..per_chunk {
            let Some(&size) = next.next() else { break 'chunks };
            samples.push((offset, size));
            offset += size as u64;
        }
    }
    if samples.len() != count {
        bail!("sample table lists {count} frames but chunks hold {}", samples.len());
    }

    // Durations.
    let mut total_duration = 0u64;
    if let Some(stts) = child(stbl, b"stts") {
        let stts = Be(stts);
        for i in 0..stts.u32(4)? as usize {
            total_duration += stts.u32(8 + i * 8)? as u64 * stts.u32(12 + i * 8)? as u64;
        }
    }

    Ok(Some(VideoTrack { fourcc, width, height, timescale, total_duration, samples }))
}

/// Writes a minimal QuickTime file with one video track; for tests.
#[cfg(test)]
pub(crate) fn write_test_mov(path: &Path, fourcc: &[u8; 4], width: u16, height: u16, fps: u32, frames: &[Vec<u8>]) {
    fn atom(kind: &[u8; 4], parts: &[&[u8]]) -> Vec<u8> {
        let body: Vec<u8> = parts.concat();
        let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend(body);
        out
    }
    let be32 = |v: u32| v.to_be_bytes();
    let n = frames.len() as u32;
    let mdat_body: Vec<u8> = frames.concat();
    // ftyp + mdat header come first; frames follow the mdat header.
    let ftyp = atom(b"ftyp", &[b"qt  ", &be32(0x200), b"qt  "]);
    let first_offset = (ftyp.len() + 8) as u32;
    let mdat = atom(b"mdat", &[&mdat_body]);

    let matrix: [u32; 9] = [0x10000, 0, 0, 0, 0x10000, 0, 0, 0, 0x40000000];
    let matrix: Vec<u8> = matrix.iter().flat_map(|v| v.to_be_bytes()).collect();
    let mvhd = atom(
        b"mvhd",
        &[&[0; 4], &[0; 8], &be32(fps), &be32(n), &be32(0x10000), &[1, 0], &[0; 10], &matrix, &[0; 24], &be32(2)],
    );
    let tkhd = atom(
        b"tkhd",
        &[
            &[0, 0, 0, 3],
            &[0; 8],
            &be32(1),
            &[0; 4],
            &be32(n),
            &[0; 8],
            &[0; 4],
            &[0; 4],
            &matrix,
            &be32((width as u32) << 16),
            &be32((height as u32) << 16),
        ],
    );
    let mdhd = atom(b"mdhd", &[&[0; 4], &[0; 8], &be32(fps), &be32(n), &[0; 4]]);
    let hdlr = atom(b"hdlr", &[&[0; 4], b"mhlr", b"vide", &[0; 12], b"\0"]);
    let vmhd = atom(b"vmhd", &[&[0, 0, 0, 1], &[0; 8]]);
    let dref = atom(b"dref", &[&[0; 4], &be32(1), &atom(b"alis", &[&[0, 0, 0, 1]])]);
    let dinf = atom(b"dinf", &[&dref]);
    let mut name = [0u8; 32];
    name[0] = 3;
    name[1..4].copy_from_slice(b"DXV");
    let entry = atom(
        fourcc,
        &[
            &[0; 6],
            &[0, 1],
            &[0; 4],
            &[0; 4],
            &[0; 4],
            &[0; 4],
            &width.to_be_bytes(),
            &height.to_be_bytes(),
            &be32(72 << 16),
            &be32(72 << 16),
            &[0; 4],
            &[0, 1],
            &name,
            &[0, 24],
            &[0xff, 0xff],
        ],
    );
    let stsd = atom(b"stsd", &[&[0; 4], &be32(1), &entry]);
    let stts = atom(b"stts", &[&[0; 4], &be32(1), &be32(n), &be32(1)]);
    let stsc = atom(b"stsc", &[&[0; 4], &be32(1), &be32(1), &be32(1), &be32(1)]);
    let sizes: Vec<u8> = frames.iter().flat_map(|f| (f.len() as u32).to_be_bytes()).collect();
    let stsz = atom(b"stsz", &[&[0; 4], &be32(0), &be32(n), &sizes]);
    let mut offsets = Vec::new();
    let mut off = first_offset;
    for f in frames {
        offsets.extend_from_slice(&off.to_be_bytes());
        off += f.len() as u32;
    }
    let stco = atom(b"stco", &[&[0; 4], &be32(n), &offsets]);
    let stbl = atom(b"stbl", &[&stsd, &stts, &stsc, &stsz, &stco]);
    let minf = atom(b"minf", &[&vmhd, &hdlr, &dinf, &stbl]);
    let mdia = atom(b"mdia", &[&mdhd, &hdlr, &minf]);
    let trak = atom(b"trak", &[&tkhd, &mdia]);
    let moov = atom(b"moov", &[&mvhd, &trak]);
    std::fs::write(path, [ftyp, mdat, moov].concat()).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_back_a_written_track() {
        let dir = std::env::temp_dir().join(format!("openvj-mov-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.mov");
        let frames = vec![vec![1u8; 10], vec![2u8; 20], vec![3u8; 30]];
        write_test_mov(&path, b"DXD3", 64, 32, 25, &frames);
        let track = read_video_track(&path).unwrap();
        assert_eq!(&track.fourcc, b"DXD3");
        assert_eq!((track.width, track.height), (64, 32));
        assert_eq!(track.fps(), 25.0);
        assert_eq!(track.samples.len(), 3);
        let bytes = std::fs::read(&path).unwrap();
        for (i, (off, size)) in track.samples.iter().enumerate() {
            assert_eq!(&bytes[*off as usize..(*off + *size as u64) as usize], &frames[i][..]);
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_non_quicktime_files() {
        let dir = std::env::temp_dir().join(format!("openvj-mov-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.mov");
        std::fs::write(&path, b"definitely not a movie").unwrap();
        assert!(read_video_track(&path).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
