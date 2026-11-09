//! openVJ audio analysis.
//!
//! [`Analyzer`] turns mono samples into four smoothed, auto-gained band levels (bass,
//! low mid, high mid, treble) that parameters can follow. It is pure and runs on the UI
//! thread once per frame. With the `input` feature, [`AudioInput`] captures an input
//! device with cpal and hands its samples over through a small shared buffer.

use std::sync::Arc;

use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};

#[cfg(feature = "input")]
mod input;
#[cfg(feature = "input")]
pub use input::{AudioInput, input_device_names};

pub const BAND_COUNT: usize = 4;
/// Default release time of the band levels, in seconds.
pub const DEFAULT_RELEASE: f32 = 0.12;
pub const BAND_NAMES: [&str; BAND_COUNT] = ["Bass", "Low mid", "High mid", "Treble"];
/// Frequency edges of the bands in Hz.
const BAND_EDGES: [f32; BAND_COUNT + 1] = [20.0, 150.0, 600.0, 3000.0, 16000.0];
const FFT_SIZE: usize = 2048;
/// Band levels are shown over this many dB below their recent peak.
const RANGE_DB: f32 = 30.0;
/// Peaks never drop below this, so silence reads as 0.
const FLOOR_DB: f32 = -60.0;
/// How far below the loudest band another band's reference level may sit.
const SPREAD_DB: f32 = 24.0;
/// How fast a band's peak falls back after a loud passage, in dB per second.
const PEAK_DECAY_DB: f32 = 4.0;

/// One frame of analysis.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Levels {
    /// Band levels, 0..1, normalised against each band's recent peak.
    pub bands: [f32; BAND_COUNT],
    /// Overall input level (RMS), 0..1, not auto-gained. For meters.
    pub level: f32,
}

pub struct Analyzer {
    sample_rate: f32,
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    /// The last `FFT_SIZE` samples, oldest first.
    history: Vec<f32>,
    scratch: Vec<Complex32>,
    peaks_db: [f32; BAND_COUNT],
    levels: Levels,
    /// Output multiplier applied after auto-gain.
    pub gain: f32,
    /// Release time of the band levels in seconds (attack is instant).
    pub release: f32,
}

impl Analyzer {
    pub fn new(sample_rate: u32) -> Self {
        let fft = FftPlanner::new().plan_fft_forward(FFT_SIZE);
        let window = (0..FFT_SIZE)
            .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (FFT_SIZE - 1) as f32).cos())
            .collect();
        Self {
            sample_rate: sample_rate.max(1) as f32,
            fft,
            window,
            history: vec![0.0; FFT_SIZE],
            scratch: vec![Complex32::default(); FFT_SIZE],
            peaks_db: [FLOOR_DB; BAND_COUNT],
            levels: Levels::default(),
            gain: 1.0,
            release: DEFAULT_RELEASE,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate as u32
    }

    /// Appends mono samples (−1..1).
    pub fn push(&mut self, samples: &[f32]) {
        if samples.len() >= FFT_SIZE {
            self.history.copy_from_slice(&samples[samples.len() - FFT_SIZE..]);
        } else {
            self.history.drain(..samples.len());
            self.history.extend_from_slice(samples);
        }
    }

    pub fn levels(&self) -> Levels {
        self.levels
    }

    /// Analyses the most recent samples. `dt` is the time since the last call, in seconds.
    pub fn analyze(&mut self, dt: f32) -> Levels {
        for (i, (s, w)) in self.history.iter().zip(&self.window).enumerate() {
            self.scratch[i] = Complex32::new(s * w, 0.0);
        }
        self.fft.process(&mut self.scratch);

        // Hann window coherent gain is 0.5, so a full-scale sine peaks at FFT_SIZE / 4.
        let norm = 4.0 / FFT_SIZE as f32;
        let hz_per_bin = self.sample_rate / FFT_SIZE as f32;
        let nyquist_bin = FFT_SIZE / 2;
        let release = (-dt.max(0.0) / self.release.max(0.001)).exp();
        let mut db = [0.0; BAND_COUNT];
        for (b, db) in db.iter_mut().enumerate() {
            let lo = ((BAND_EDGES[b] / hz_per_bin).ceil() as usize).clamp(1, nyquist_bin);
            let hi = ((BAND_EDGES[b + 1] / hz_per_bin).floor() as usize).clamp(lo, nyquist_bin);
            // Strongest bin rather than the average, so a single kick or hi-hat reads clearly.
            let peak = self.scratch[lo..=hi].iter().map(|c| c.norm() * norm).fold(0.0, f32::max);
            *db = 20.0 * peak.max(1e-9).log10();
            self.peaks_db[b] = db.max(self.peaks_db[b] - PEAK_DECAY_DB * dt).max(FLOOR_DB);
        }
        // A band never auto-gains more than SPREAD_DB above the loudest one, so window
        // leakage next to a loud band doesn't read as a full-scale signal.
        let loudest = self.peaks_db.iter().copied().fold(FLOOR_DB, f32::max);
        let mut bands = [0.0; BAND_COUNT];
        for (b, band) in bands.iter_mut().enumerate() {
            let reference = self.peaks_db[b].max(loudest - SPREAD_DB);
            let target = ((db[b] - (reference - RANGE_DB)) / RANGE_DB).clamp(0.0, 1.0);
            let target = (target * self.gain).clamp(0.0, 1.0);
            let prev = self.levels.bands[b];
            *band = if target >= prev { target } else { target + (prev - target) * release };
        }
        let rms = (self.history.iter().map(|s| s * s).sum::<f32>() / FFT_SIZE as f32).sqrt();
        let level = (rms * std::f32::consts::SQRT_2).clamp(0.0, 1.0);
        self.levels = Levels { bands, level };
        self.levels
    }

    /// Clears history and peaks, e.g. after switching devices.
    pub fn reset(&mut self) {
        self.history.fill(0.0);
        self.peaks_db = [FLOOR_DB; BAND_COUNT];
        self.levels = Levels::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn sine(freq: f32, amp: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| amp * (std::f32::consts::TAU * freq * i as f32 / RATE as f32).sin()).collect()
    }

    fn strongest(levels: &Levels) -> usize {
        levels.bands.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).unwrap().0
    }

    #[test]
    fn silence_reads_zero() {
        let mut a = Analyzer::new(RATE);
        a.push(&[0.0; FFT_SIZE]);
        let l = a.analyze(1.0 / 60.0);
        assert_eq!(l.bands, [0.0; BAND_COUNT]);
        assert_eq!(l.level, 0.0);
    }

    #[test]
    fn tones_land_in_their_band() {
        for (freq, band) in [(60.0, 0), (300.0, 1), (1200.0, 2), (6000.0, 3)] {
            let mut a = Analyzer::new(RATE);
            a.push(&sine(freq, 0.5, FFT_SIZE));
            let l = a.analyze(1.0 / 60.0);
            assert_eq!(strongest(&l), band, "{freq} Hz: {l:?}");
            assert!(l.bands[band] > 0.9, "{freq} Hz: {l:?}");
            for (i, v) in l.bands.iter().enumerate().filter(|(i, _)| *i != band) {
                assert!(*v < 0.2, "{freq} Hz leaks into band {i}: {l:?}");
            }
            assert!((l.level - 0.5).abs() < 0.02, "level {l:?}");
        }
    }

    #[test]
    fn levels_release_smoothly_and_gain_scales() {
        let mut a = Analyzer::new(RATE);
        a.push(&sine(80.0, 0.8, FFT_SIZE));
        assert!(a.analyze(0.016).bands[0] > 0.9);
        a.push(&[0.0; FFT_SIZE]);
        let falling = a.analyze(0.016).bands[0];
        assert!(falling > 0.5 && falling < 1.0, "release keeps some level: {falling}");
        let later = (0..60).map(|_| a.analyze(0.016)).last().unwrap();
        assert!(later.bands[0] < 0.01);

        let mut a = Analyzer::new(RATE);
        a.gain = 0.5;
        a.push(&sine(80.0, 0.8, FFT_SIZE));
        assert!((a.analyze(0.016).bands[0] - 0.5).abs() < 0.05);
    }

    #[test]
    fn auto_gain_follows_quiet_material() {
        let mut a = Analyzer::new(RATE);
        a.push(&sine(80.0, 0.01, FFT_SIZE));
        assert!(a.analyze(0.016).bands[0] > 0.9, "quiet input still reaches full scale");
    }

    #[test]
    fn short_pushes_scroll_history() {
        let mut a = Analyzer::new(RATE);
        a.push(&[1.0; 10]);
        assert_eq!(a.history.len(), FFT_SIZE);
        assert_eq!(a.history[FFT_SIZE - 1], 1.0);
        assert_eq!(a.history[FFT_SIZE - 11], 0.0);
    }
}
