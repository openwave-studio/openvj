//! Plays intra-only movies (DXV, HAP) frame by frame, straight from the file.
//!
//! A worker thread reads and decodes frames by index and follows the clip clock: it skips
//! ahead when playback outruns it and restarts when the clock jumps back, so speed changes
//! never stall playback.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use anyhow::Result;

use crate::mov::VideoTrack;

/// Frames the worker may run ahead of the clock before a backwards jump resets it.
const LOOKAHEAD: i64 = 8;

/// Decodes one compressed sample into a frame, given the track's visible size.
pub(crate) type DecodeFn<F> = fn(&[u8], u32, u32) -> Result<F>;

/// Reads sample `(offset, size)` into `buf`.
pub(crate) fn read_sample(file: &mut File, (offset, size): (u64, u32), buf: &mut Vec<u8>) -> Result<()> {
    buf.resize(size as usize, 0);
    file.seek(SeekFrom::Start(offset))?;
    file.read_exact(buf)?;
    Ok(())
}

/// Hands out decoded frames of a looping movie as the clip clock moves.
pub(crate) struct FramePlayer<F> {
    fps: f64,
    want: Arc<AtomicI64>,
    frames: Receiver<(i64, F)>,
    pending: Option<(i64, F)>,
    /// Index of the last frame handed out (-1 before the first).
    index: i64,
}

impl<F: Send + 'static> FramePlayer<F> {
    /// Starts the worker. `first` is frame 0, already decoded by the caller to validate the file.
    pub fn spawn(name: &str, track: VideoTrack, mut file: File, first: F, decode: DecodeFn<F>) -> Result<Self> {
        let fps = track.fps();
        let want = Arc::new(AtomicI64::new(0));
        let (tx, frames) = mpsc::sync_channel::<(i64, F)>(2);
        let worker_want = want.clone();
        let label = name.to_owned();
        thread::Builder::new().name(format!("openvj-{}", name.to_lowercase())).spawn(move || {
            let VideoTrack { width, height, samples, .. } = track;
            let n = samples.len() as i64;
            if tx.send((0, first)).is_err() {
                return;
            }
            let mut buf = Vec::new();
            let mut next = 1i64;
            let mut reported = false;
            loop {
                let w = worker_want.load(Ordering::Relaxed);
                if w > next || w + LOOKAHEAD < next {
                    next = w;
                }
                let decoded = read_sample(&mut file, samples[next.rem_euclid(n) as usize], &mut buf)
                    .and_then(|()| decode(&buf, width, height));
                match decoded {
                    Ok(frame) => {
                        if tx.send((next, frame)).is_err() {
                            return;
                        }
                    }
                    Err(e) if !reported => {
                        eprintln!("openVJ: skipping undecodable {label} frame {}: {e:#}", next.rem_euclid(n));
                        reported = true;
                    }
                    Err(_) => {}
                }
                next += 1;
            }
        })?;
        Ok(Self { fps, want, frames, pending: None, index: -1 })
    }

    /// Returns the frame for clip time `t` (seconds) if it differs from the last one returned.
    pub fn frame_at(&mut self, t: f64) -> Option<F> {
        let target = (t.max(0.0) * self.fps).floor() as i64;
        self.want.store(target, Ordering::Relaxed);
        if target < self.index {
            // The clock jumped back (e.g. a speed change): accept earlier frames again.
            self.index = target - 1;
            self.pending = None;
        }
        let mut newest = None;
        loop {
            let item = match self.pending.take() {
                Some(p) => p,
                None => match self.frames.try_recv() {
                    Ok(p) => p,
                    Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
                },
            };
            let (i, frame) = item;
            if i > target {
                if i <= target + LOOKAHEAD {
                    self.pending = Some((i, frame));
                    break;
                }
                continue; // Decoded before a backwards jump.
            }
            if i > self.index {
                self.index = i;
                newest = Some(frame);
            }
        }
        newest
    }
}
