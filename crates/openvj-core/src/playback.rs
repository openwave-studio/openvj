//! Clip playback settings: how a clip's position moves through its media.
//!
//! These follow Resolume's clip transport: a play mode (loop, bounce, random, play once),
//! and either free-running time (Resolume's "Timeline", scaled by the clip speed) or beat
//! sync, where one pass through the clip lasts a fixed number of beats at the current tempo.

use serde::{Deserialize, Serialize};

/// How long a still image or generator clip lasts when nothing else says. This only matters
/// for the play modes that need an end (bounce, random and play once). Resolume uses 5 s
/// for generators.
pub const DEFAULT_CLIP_SECS: f32 = 5.0;

/// Beat lengths offered for beat sync, shortest first.
pub const SYNC_BEATS: [f32; 11] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0, 256.0];

/// What happens when a clip reaches its end. The order matches Resolume's play mode menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayMode {
    #[default]
    Loop,
    /// Plays forward, then backward, and so on.
    Bounce,
    /// Jumps to a random position on every beat and plays on from there.
    Random,
    /// Plays once, then clears the layer.
    OnceClear,
    /// Plays once, then holds the last frame.
    OnceHold,
}

impl PlayMode {
    pub const ALL: [PlayMode; 5] =
        [PlayMode::Loop, PlayMode::Bounce, PlayMode::Random, PlayMode::OnceClear, PlayMode::OnceHold];

    pub fn name(self) -> &'static str {
        match self {
            PlayMode::Loop => "Loop",
            PlayMode::Bounce => "Bounce",
            PlayMode::Random => "Random",
            PlayMode::OnceClear => "Play once & clear",
            PlayMode::OnceHold => "Play once & hold",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|m| *m == self).unwrap()
    }

    pub fn from_index(i: usize) -> Self {
        Self::ALL.get(i).copied().unwrap_or_default()
    }

    pub fn plays_once(self) -> bool {
        matches!(self, PlayMode::OnceClear | PlayMode::OnceHold)
    }
}

/// Where a clip is at a moment in time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipTime {
    /// Position in the clip's own time, in seconds. Media clips use it as the frame time;
    /// generators use it as their animation clock.
    pub secs: f64,
    /// The clip played once to its end (only in the play-once modes).
    pub ended: bool,
}

/// Maps elapsed time to a position.
///
/// * `cycles`: passes through the clip since launch (1.0 = one full length).
/// * `len`: the clip's length in seconds.
/// * `wrap`: whether looping wraps around (generators run on instead, since their
///   animation has no end and wrapping would make it jump).
/// * `seed`: varies the random jumps between clips.
pub(crate) fn position(mode: PlayMode, cycles: f64, beats: f64, len: f64, wrap: bool, seed: u64) -> ClipTime {
    let cycles = cycles.max(0.0);
    let (pos, ended) = match mode {
        PlayMode::Loop if wrap => (cycles.fract(), false),
        PlayMode::Loop => (cycles, false),
        PlayMode::Bounce => {
            let t = cycles.rem_euclid(2.0);
            (if t <= 1.0 { t } else { 2.0 - t }, false)
        }
        PlayMode::Random => {
            let jump = random_unit(seed, beats.max(0.0).floor() as u64);
            ((jump + cycles).fract(), false)
        }
        PlayMode::OnceClear | PlayMode::OnceHold => (cycles.min(1.0), cycles >= 1.0),
    };
    // Stay just inside the clip at its end so the last frame, not the first, is shown.
    let secs = if wrap || mode != PlayMode::Loop { (pos * len).min(len - 1e-6).max(0.0) } else { pos * len };
    ClipTime { secs, ended }
}

/// A stable pseudo-random number in 0..1 for (seed, n).
fn random_unit(seed: u64, n: u64) -> f64 {
    // SplitMix64.
    let mut z = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ n.wrapping_add(0xD1B5_4A32_D192_ED03);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(mode: PlayMode, cycles: f64) -> ClipTime {
        position(mode, cycles, cycles * 4.0, 10.0, true, 1)
    }

    #[test]
    fn loop_wraps() {
        assert!((at(PlayMode::Loop, 1.25).secs - 2.5).abs() < 1e-9);
        assert!(!at(PlayMode::Loop, 7.0).ended);
    }

    #[test]
    fn generators_run_on_in_loop_mode() {
        let t = position(PlayMode::Loop, 2.5, 0.0, 10.0, false, 1);
        assert_eq!(t.secs, 25.0);
    }

    #[test]
    fn bounce_goes_back_and_forth() {
        assert!((at(PlayMode::Bounce, 0.25).secs - 2.5).abs() < 1e-9);
        assert!((at(PlayMode::Bounce, 1.25).secs - 7.5).abs() < 1e-9);
        assert!((at(PlayMode::Bounce, 2.25).secs - 2.5).abs() < 1e-9);
    }

    #[test]
    fn play_once_ends_and_holds_the_last_frame() {
        let t = at(PlayMode::OnceHold, 0.5);
        assert!(!t.ended);
        let t = at(PlayMode::OnceClear, 3.0);
        assert!(t.ended);
        assert!(t.secs > 9.99 && t.secs < 10.0);
    }

    #[test]
    fn random_jumps_each_beat_and_plays_on_between() {
        let a = position(PlayMode::Random, 0.10, 1.2, 10.0, true, 7).secs;
        let b = position(PlayMode::Random, 0.11, 1.3, 10.0, true, 7).secs;
        let c = position(PlayMode::Random, 0.12, 2.1, 10.0, true, 7).secs;
        assert!(((b - a).rem_euclid(10.0) - 0.1).abs() < 1e-6, "plays forward within a beat");
        assert!(((c - b).rem_euclid(10.0) - 0.1).abs() > 1e-3, "jumps on the next beat");
        assert!((0.0..10.0).contains(&c));
    }

    #[test]
    fn play_mode_indices_round_trip() {
        for m in PlayMode::ALL {
            assert_eq!(PlayMode::from_index(m.index()), m);
        }
    }
}
