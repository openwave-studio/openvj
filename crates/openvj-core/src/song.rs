//! Hears when a new song starts, for playlist presets that play "until the song changes".
//!
//! Three signs, any one of them is enough:
//! - a gap: the music goes nearly silent for a moment, then comes back;
//! - a tempo jump: the tempo source (Link, MIDI clock, tap) settles on a new BPM;
//! - a new sound: the balance of the four audio bands, at a similar loudness, differs from
//!   the last half minute's for a good while (a DJ blend into the next track).
//!
//! A breakdown is quieter than the song around it, so a new sound only counts at a similar
//! loudness; the playlist's 16-bar minimum covers the rest. All of it runs on the four band
//! levels openVJ already has, a few multiplications per frame.

use crate::{BAND_COUNT, ModInputs};

/// Below this share of the song's usual loudness (and below [`SILENT`]) counts as a gap.
const GAP_SHARE: f32 = 0.12;
/// Absolute level that always counts as silent.
const SILENT: f32 = 0.02;
/// How long a gap must last, in seconds.
const GAP_SECS: f32 = 0.6;
/// Smallest tempo change that counts, in BPM.
const TEMPO_JUMP: f32 = 3.0;
/// How long a new tempo must hold, in seconds.
const TEMPO_HOLD: f32 = 4.0;
/// Time constants of the slow ("the song so far") and quick ("now") averages, in seconds.
const SLOW: f32 = 30.0;
const QUICK: f32 = 5.0;
/// How different the band balance must be (sum of absolute differences of band shares, 0..2).
const NEW_SOUND: f32 = 0.35;
/// The quick loudness must be at least this share of the slow one (not a breakdown).
const SIMILAR_LOUDNESS: f32 = 0.6;
/// How long the new sound must hold, in seconds.
const NEW_SOUND_SECS: f32 = 12.0;
/// No new sign for this long after one, in seconds.
const REST_SECS: f32 = 20.0;

/// Why a new song was heard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SongChange {
    Gap,
    Tempo,
    NewSound,
}

#[derive(Debug, Clone, Default)]
pub struct SongDetector {
    slow_shape: [f32; BAND_COUNT],
    quick_shape: [f32; BAND_COUNT],
    slow_level: f32,
    quick_level: f32,
    /// Seconds of sound heard so far (averages are meaningless before a few seconds).
    heard: f32,
    quiet_for: f32,
    tempo: f32,
    new_tempo_for: f32,
    new_sound_for: f32,
    rest: f32,
}

impl SongDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds one frame of audio (`inputs`, `dt` seconds) and the current tempo. Returns the
    /// reason when a new song just started.
    pub fn update(&mut self, inputs: &ModInputs, bpm: f32, dt: f32) -> Option<SongChange> {
        let dt = dt.clamp(0.0, 0.5);
        self.rest = (self.rest - dt).max(0.0);
        let change = self.gap(inputs.overall.level, dt).or(self.tempo(bpm, dt)).or(self.sound(inputs, dt));
        match change {
            Some(c) if self.rest <= 0.0 => {
                self.restart();
                Some(c)
            }
            _ => None,
        }
    }

    fn restart(&mut self) {
        // The new song becomes "the song so far".
        self.slow_shape = self.quick_shape;
        self.slow_level = self.quick_level;
        self.new_sound_for = 0.0;
        self.new_tempo_for = 0.0;
        self.rest = REST_SECS;
    }

    fn gap(&mut self, level: f32, dt: f32) -> Option<SongChange> {
        let quiet = level < SILENT || (self.heard > 5.0 && level < self.slow_level * GAP_SHARE);
        if quiet {
            self.quiet_for += dt;
            return None;
        }
        let was_gap = self.quiet_for >= GAP_SECS && self.heard > 5.0;
        self.quiet_for = 0.0;
        was_gap.then_some(SongChange::Gap)
    }

    fn tempo(&mut self, bpm: f32, dt: f32) -> Option<SongChange> {
        if self.tempo <= 0.0 {
            self.tempo = bpm;
            return None;
        }
        if (bpm - self.tempo).abs() < TEMPO_JUMP {
            self.new_tempo_for = 0.0;
            return None;
        }
        self.new_tempo_for += dt;
        if self.new_tempo_for >= TEMPO_HOLD {
            self.tempo = bpm;
            return Some(SongChange::Tempo);
        }
        None
    }

    fn sound(&mut self, inputs: &ModInputs, dt: f32) -> Option<SongChange> {
        let level = inputs.overall.level;
        if level < SILENT {
            return None; // Silence says nothing about the sound.
        }
        let sum: f32 = inputs.bands.iter().sum::<f32>().max(1e-4);
        let shape = inputs.bands.map(|b| b / sum);
        let ease = |avg: &mut f32, v: f32, tau: f32| *avg += (v - *avg) * (dt / tau).min(1.0);
        if self.heard == 0.0 {
            self.slow_shape = shape;
            self.quick_shape = shape;
            self.slow_level = level;
            self.quick_level = level;
        }
        self.heard += dt;
        for (avg, v) in self.quick_shape.iter_mut().zip(shape) {
            ease(avg, v, QUICK);
        }
        ease(&mut self.quick_level, level, QUICK);
        let differs: f32 = self.slow_shape.iter().zip(&self.quick_shape).map(|(a, b)| (a - b).abs()).sum();
        let loud_enough = self.quick_level >= self.slow_level * SIMILAR_LOUDNESS;
        if self.heard > SLOW / 2.0 && differs >= NEW_SOUND && loud_enough {
            self.new_sound_for += dt;
        } else {
            self.new_sound_for = 0.0;
            // Only learn "the song so far" while the sound is the same song.
            for (avg, v) in self.slow_shape.iter_mut().zip(shape) {
                ease(avg, v, SLOW);
            }
            ease(&mut self.slow_level, level, SLOW);
        }
        (self.new_sound_for >= NEW_SOUND_SECS).then_some(SongChange::NewSound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AudioSignal;

    const DT: f32 = 1.0 / 60.0;

    fn sound(bands: [f32; BAND_COUNT]) -> ModInputs {
        let level = bands.iter().sum::<f32>() / BAND_COUNT as f32;
        ModInputs { bands, overall: AudioSignal { level, ..Default::default() }, ..Default::default() }
    }

    /// Runs `secs` of the same sound and returns every change heard.
    fn run(d: &mut SongDetector, input: &ModInputs, bpm: f32, secs: f32) -> Vec<SongChange> {
        (0..(secs / DT) as usize).filter_map(|_| d.update(input, bpm, DT)).collect()
    }

    const HOUSE: [f32; BAND_COUNT] = [0.7, 0.4, 0.3, 0.2];
    const BREAKDOWN: [f32; BAND_COUNT] = [0.05, 0.15, 0.2, 0.15];
    const TECHNO: [f32; BAND_COUNT] = [0.3, 0.2, 0.6, 0.6];

    #[test]
    fn a_gap_between_songs() {
        let mut d = SongDetector::new();
        assert!(run(&mut d, &sound(HOUSE), 124.0, 60.0).is_empty());
        assert!(run(&mut d, &sound([0.0; 4]), 124.0, 1.0).is_empty());
        assert_eq!(run(&mut d, &sound(HOUSE), 124.0, 5.0), vec![SongChange::Gap]);
    }

    #[test]
    fn a_short_dip_is_not_a_gap() {
        let mut d = SongDetector::new();
        run(&mut d, &sound(HOUSE), 124.0, 60.0);
        run(&mut d, &sound([0.0; 4]), 124.0, 0.2);
        assert!(run(&mut d, &sound(HOUSE), 124.0, 5.0).is_empty());
    }

    #[test]
    fn a_tempo_jump() {
        let mut d = SongDetector::new();
        run(&mut d, &sound(HOUSE), 124.0, 30.0);
        // Nudging the tempo a little isn't a new song.
        assert!(run(&mut d, &sound(HOUSE), 125.0, 10.0).is_empty());
        assert_eq!(run(&mut d, &sound(HOUSE), 132.0, 10.0), vec![SongChange::Tempo]);
    }

    #[test]
    fn a_blend_into_a_different_track() {
        let mut d = SongDetector::new();
        run(&mut d, &sound(HOUSE), 128.0, 90.0);
        assert_eq!(run(&mut d, &sound(TECHNO), 128.0, 40.0), vec![SongChange::NewSound]);
    }

    #[test]
    fn a_breakdown_is_not_a_new_song() {
        let mut d = SongDetector::new();
        run(&mut d, &sound(HOUSE), 128.0, 90.0);
        // 32 bars of breakdown at 128 BPM is a minute.
        assert!(run(&mut d, &sound(BREAKDOWN), 128.0, 60.0).is_empty());
        assert!(run(&mut d, &sound(HOUSE), 128.0, 30.0).is_empty());
    }
}
