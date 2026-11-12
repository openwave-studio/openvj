//! openVJ audio analysis.
//!
//! [`Analyzer`] turns mono samples into four smoothed, auto-gained band levels (bass,
//! low mid, high mid, treble) that parameters can follow, plus Synesthesia-style hits,
//! presence and audio clocks per band, and a log-spaced spectrum with the waveform for
//! shaders. It is pure and runs on the UI
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
/// Number of log-spaced bins in [`Analyzer::spectrum`].
pub const SPECTRUM_BINS: usize = 256;
const SPECTRUM_LO_HZ: f32 = 20.0;
const SPECTRUM_HI_HZ: f32 = 16000.0;
/// How far a signal must jump above its recent average before it counts as a hit, and the
/// jump that reads as a full hit.
const HIT_THRESHOLD: f32 = 0.1;
const HIT_RANGE: f32 = 0.35;
/// How fast a hit falls back, and the averaging time it is measured against, in seconds.
const HIT_DECAY: f32 = 0.12;
const HIT_AVERAGE: f32 = 0.25;
/// Presence rises over about a second and falls over a few.
const PRESENCE_ATTACK: f32 = 1.0;
const PRESENCE_RELEASE: f32 = 3.0;
/// An audio clock runs this many seconds per second at full level (about real time at half).
const CLOCK_RATE: f32 = 2.0;
/// Audio clocks wrap here, like the shaders' TIME, to keep float precision.
const CLOCK_WRAP: f32 = 3600.0;
/// Slow smoothing of the spectrum's third channel, in seconds.
const SPECTRUM_SMOOTH: f32 = 0.3;

/// One frame of analysis.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Levels {
    /// Band levels, 0..1, normalised against each band's recent peak.
    pub bands: [f32; BAND_COUNT],
    /// Overall input level (RMS), 0..1, not auto-gained. For meters.
    pub level: f32,
    /// Hits, presence and clock of each band.
    pub band_features: [Features; BAND_COUNT],
    /// The same for the whole signal (the mean of the bands).
    pub overall: Features,
}

/// What a band (or the whole signal) is doing beyond its level, like Synesthesia's
/// `syn_*Level`, `syn_*Hits`, `syn_*Presence` and `syn_*Time`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Features {
    /// Smoothed level, 0..1.
    pub level: f32,
    /// 0..1 spike on a transient (a kick, a snare), falling back within a fraction of a second.
    pub hits: f32,
    /// 0..1, slow: high while the band is strong in the song.
    pub presence: f32,
    /// Seconds that run faster while the band is loud (wraps at an hour).
    pub clock: f32,
}

/// Follows one signal to derive its [`Features`].
#[derive(Debug, Clone, Copy, Default)]
struct Follower {
    average: f32,
    features: Features,
}

impl Follower {
    /// `target` is this frame's level before release smoothing, `level` after it.
    fn update(&mut self, target: f32, level: f32, dt: f32) -> Features {
        let f = &mut self.features;
        let rise = target - self.average;
        let hit = ((rise - HIT_THRESHOLD) / HIT_RANGE).clamp(0.0, 1.0);
        f.hits = hit.max(f.hits * (-dt / HIT_DECAY).exp());
        self.average += (target - self.average) * (1.0 - (-dt / HIT_AVERAGE).exp());
        let tau = if level > f.presence { PRESENCE_ATTACK } else { PRESENCE_RELEASE };
        f.presence += (level - f.presence) * (1.0 - (-dt / tau).exp());
        f.clock = (f.clock + dt * level * CLOCK_RATE) % CLOCK_WRAP;
        f.level = level;
        *f
    }
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
    followers: [Follower; BAND_COUNT + 1],
    /// FFT bin range (inclusive) of each spectrum bin.
    spectrum_bins: Vec<(usize, usize)>,
    spectrum_peak_db: f32,
    spectrum: Vec<[f32; 4]>,
    /// Output multiplier applied after auto-gain (the composition's Reactivity).
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
        let sample_rate = sample_rate.max(1) as f32;
        let hz_per_bin = sample_rate / FFT_SIZE as f32;
        let ratio = SPECTRUM_HI_HZ / SPECTRUM_LO_HZ;
        let spectrum_bins = (0..SPECTRUM_BINS)
            .map(|i| {
                let edge = |i: usize| SPECTRUM_LO_HZ * ratio.powf(i as f32 / SPECTRUM_BINS as f32) / hz_per_bin;
                let lo = (edge(i).round() as usize).clamp(1, FFT_SIZE / 2);
                let hi = ((edge(i + 1).round() as usize).saturating_sub(1)).clamp(lo, FFT_SIZE / 2);
                (lo, hi)
            })
            .collect();
        Self {
            sample_rate,
            fft,
            window,
            history: vec![0.0; FFT_SIZE],
            scratch: vec![Complex32::default(); FFT_SIZE],
            peaks_db: [FLOOR_DB; BAND_COUNT],
            levels: Levels::default(),
            followers: Default::default(),
            spectrum_bins,
            spectrum_peak_db: FLOOR_DB,
            spectrum: vec![[0.0, 0.0, 0.0, 0.5]; SPECTRUM_BINS],
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

    /// The last analysed spectrum, [`SPECTRUM_BINS`] log-spaced bins from 20 Hz to 16 kHz,
    /// laid out like Synesthesia's `syn_Spectrum`: r = raw 0..1, g = with the band release
    /// (the one to use), b = slowly smoothed, a = waveform (0.5 is silence).
    pub fn spectrum(&self) -> &[[f32; 4]] {
        &self.spectrum
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
        let mut targets = [0.0; BAND_COUNT];
        for (b, band) in bands.iter_mut().enumerate() {
            let reference = self.peaks_db[b].max(loudest - SPREAD_DB);
            let target = ((db[b] - (reference - RANGE_DB)) / RANGE_DB).clamp(0.0, 1.0);
            let target = (target * self.gain).clamp(0.0, 1.0);
            targets[b] = target;
            let prev = self.levels.bands[b];
            *band = if target >= prev { target } else { target + (prev - target) * release };
        }
        let dt = dt.max(0.0);
        let mut band_features = [Features::default(); BAND_COUNT];
        for (b, f) in band_features.iter_mut().enumerate() {
            *f = self.followers[b].update(targets[b], bands[b], dt);
        }
        let mean = |v: &[f32; BAND_COUNT]| v.iter().sum::<f32>() / BAND_COUNT as f32;
        let overall = self.followers[BAND_COUNT].update(mean(&targets), mean(&bands), dt);
        let rms = (self.history.iter().map(|s| s * s).sum::<f32>() / FFT_SIZE as f32).sqrt();
        let level = (rms * std::f32::consts::SQRT_2).clamp(0.0, 1.0);
        self.levels = Levels { bands, level, band_features, overall };
        self.analyze_spectrum(norm, release, dt);
        self.levels
    }

    fn analyze_spectrum(&mut self, norm: f32, release: f32, dt: f32) {
        let mut loudest = FLOOR_DB;
        let mut db = [0.0f32; SPECTRUM_BINS];
        for (d, &(lo, hi)) in db.iter_mut().zip(&self.spectrum_bins) {
            let peak = self.scratch[lo..=hi].iter().map(|c| c.norm() * norm).fold(0.0, f32::max);
            *d = 20.0 * peak.max(1e-9).log10();
            loudest = loudest.max(*d);
        }
        self.spectrum_peak_db = loudest.max(self.spectrum_peak_db - PEAK_DECAY_DB * dt).max(FLOOR_DB);
        let floor = self.spectrum_peak_db - RANGE_DB;
        let smooth = 1.0 - (-dt / SPECTRUM_SMOOTH).exp();
        // The waveform: the newest 2 × SPECTRUM_BINS samples, every other one.
        let wave = &self.history[FFT_SIZE - 2 * SPECTRUM_BINS..];
        for (i, (s, d)) in self.spectrum.iter_mut().zip(db).enumerate() {
            let raw = (((d - floor) / RANGE_DB).clamp(0.0, 1.0) * self.gain).min(1.0);
            let held = if raw >= s[1] { raw } else { raw + (s[1] - raw) * release };
            let slow = s[2] + (raw - s[2]) * smooth;
            *s = [raw, held, slow, (0.5 + 0.5 * wave[2 * i]).clamp(0.0, 1.0)];
        }
    }

    /// Clears history and peaks, e.g. after switching devices.
    pub fn reset(&mut self) {
        self.history.fill(0.0);
        self.peaks_db = [FLOOR_DB; BAND_COUNT];
        self.levels = Levels::default();
        self.followers = Default::default();
        self.spectrum_peak_db = FLOOR_DB;
        self.spectrum.fill([0.0, 0.0, 0.0, 0.5]);
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
    fn a_kick_is_a_hit_and_a_steady_tone_is_not() {
        let mut a = Analyzer::new(RATE);
        for _ in 0..30 {
            a.push(&[0.0; 800]);
            a.analyze(1.0 / 60.0);
        }
        a.push(&sine(60.0, 0.8, FFT_SIZE));
        let kick = a.analyze(1.0 / 60.0);
        assert!(kick.band_features[0].hits > 0.9, "{kick:?}");
        assert!(kick.overall.hits > 0.1, "{kick:?}");
        let later = (0..120)
            .map(|_| {
                a.push(&sine(60.0, 0.8, FFT_SIZE));
                a.analyze(1.0 / 60.0)
            })
            .last()
            .unwrap();
        assert!(later.band_features[0].hits < 0.05, "steady tone: {later:?}");
        assert!(later.band_features[0].presence > 0.8, "presence builds up: {later:?}");
        assert!(later.band_features[3].presence < 0.2, "{later:?}");
    }

    #[test]
    fn clocks_run_faster_when_loud() {
        let mut a = Analyzer::new(RATE);
        for _ in 0..60 {
            a.push(&sine(60.0, 0.8, FFT_SIZE));
            a.analyze(1.0 / 60.0);
        }
        let l = a.levels();
        assert!(l.band_features[0].clock > 1.5, "bass clock ran ~2 s in 1 s: {l:?}");
        assert!(l.band_features[3].clock < 0.3, "treble barely moved: {l:?}");
    }

    #[test]
    fn spectrum_peaks_at_the_tone_and_carries_the_waveform() {
        let mut a = Analyzer::new(RATE);
        a.push(&sine(1000.0, 0.5, FFT_SIZE));
        a.analyze(1.0 / 60.0);
        let s = a.spectrum();
        assert_eq!(s.len(), SPECTRUM_BINS);
        let peak = s.iter().enumerate().max_by(|x, y| x.1[0].total_cmp(&y.1[0])).unwrap().0;
        let hz = SPECTRUM_LO_HZ * (SPECTRUM_HI_HZ / SPECTRUM_LO_HZ).powf((peak as f32 + 0.5) / SPECTRUM_BINS as f32);
        assert!((hz / 1000.0 - 1.0).abs() < 0.05, "peak at {hz} Hz");
        assert!(s[10][0] < 0.1 && s[250][0] < 0.1);
        let (lo, hi) = s.iter().fold((1.0f32, 0.0f32), |(lo, hi), v| (lo.min(v[3]), hi.max(v[3])));
        assert!(lo < 0.3 && hi > 0.7, "waveform swings around 0.5: {lo}..{hi}");
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
