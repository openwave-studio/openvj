//! Parameter modulation: audio bands and tempo-synced shapes driving normalised parameters.

use serde::{Deserialize, Serialize};

/// Number of audio bands a parameter can follow (bass, low mid, high mid, treble).
pub const BAND_COUNT: usize = 4;

/// Beat lengths a tempo-synced modulator can cycle over, in beats.
pub const MOD_RATES: [f32; 7] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];

/// Live inputs that modulators read, sampled once per frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ModInputs {
    /// Beat position from the tempo clock (or Ableton Link).
    pub beat: f64,
    /// Audio band levels, normalised 0..1.
    pub bands: [f32; BAND_COUNT],
    /// Hits, presence and clock of each band.
    pub band_signals: [AudioSignal; BAND_COUNT],
    /// The same for the whole signal.
    pub overall: AudioSignal,
}

/// What a band (or the whole signal) is doing, like Synesthesia's `syn_*Level`, `syn_*Hits`,
/// `syn_*Presence` and `syn_*Time`. Already scaled by the composition's Reactivity.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AudioSignal {
    /// 0..1.
    pub level: f32,
    /// 0..1 spike on transients.
    pub hits: f32,
    /// 0..1, slow: high while the band is strong in the song.
    pub presence: f32,
    /// Seconds that run faster while the band is loud.
    pub clock: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModSource {
    #[default]
    Off,
    Bass,
    LowMid,
    HighMid,
    Treble,
    Level,
    BassHits,
    LowMidHits,
    HighMidHits,
    TrebleHits,
    Hits,
    Sine,
    Saw,
    Square,
    Pulse,
}

impl ModSource {
    /// In menu order: audio sources first, then tempo shapes (see [`ModSource::FIRST_TEMPO`]).
    pub const ALL: [ModSource; 15] = [
        ModSource::Off,
        ModSource::Bass,
        ModSource::LowMid,
        ModSource::HighMid,
        ModSource::Treble,
        ModSource::Level,
        ModSource::BassHits,
        ModSource::LowMidHits,
        ModSource::HighMidHits,
        ModSource::TrebleHits,
        ModSource::Hits,
        ModSource::Sine,
        ModSource::Saw,
        ModSource::Square,
        ModSource::Pulse,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ModSource::Off => "Off",
            ModSource::Bass => "Bass",
            ModSource::LowMid => "Low mid",
            ModSource::HighMid => "High mid",
            ModSource::Treble => "Treble",
            ModSource::Level => "Level",
            ModSource::BassHits => "Bass hits",
            ModSource::LowMidHits => "Low mid hits",
            ModSource::HighMidHits => "High mid hits",
            ModSource::TrebleHits => "Treble hits",
            ModSource::Hits => "Hits",
            ModSource::Sine => "Sine",
            ModSource::Saw => "Saw",
            ModSource::Square => "Square",
            ModSource::Pulse => "Pulse",
        }
    }

    /// Index in [`ModSource::ALL`] of the first tempo shape; everything before it except Off
    /// follows the audio.
    pub const FIRST_TEMPO: usize = 11;

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap()
    }

    pub fn from_index(i: usize) -> Self {
        Self::ALL.get(i).copied().unwrap_or_default()
    }

    /// The audio band this source follows, if it is an audio source.
    pub fn band(self) -> Option<usize> {
        match self {
            ModSource::Bass => Some(0),
            ModSource::LowMid => Some(1),
            ModSource::HighMid => Some(2),
            ModSource::Treble => Some(3),
            _ => None,
        }
    }

    /// True for the shapes that cycle with the tempo.
    pub fn is_beat(self) -> bool {
        matches!(self, ModSource::Sine | ModSource::Saw | ModSource::Square | ModSource::Pulse)
    }
}

/// How a parameter follows a live input: `value = base + amount × signal`, clamped to 0..1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Modulation {
    pub source: ModSource,
    /// −1..1. Negative amounts push the value down as the signal rises.
    pub amount: f32,
    /// Cycle length in beats for tempo-synced shapes.
    pub beats: f32,
}

impl Default for Modulation {
    fn default() -> Self {
        Self { source: ModSource::Off, amount: 1.0, beats: 1.0 }
    }
}

impl Modulation {
    pub fn is_active(&self) -> bool {
        self.source != ModSource::Off
    }

    /// The raw 0..1 signal of the source.
    pub fn signal(&self, inputs: &ModInputs) -> f32 {
        if let Some(band) = self.source.band() {
            return inputs.bands[band].clamp(0.0, 1.0);
        }
        let hits = |b: usize| inputs.band_signals[b].hits.clamp(0.0, 1.0);
        match self.source {
            ModSource::Level => return inputs.overall.level.clamp(0.0, 1.0),
            ModSource::BassHits => return hits(0),
            ModSource::LowMidHits => return hits(1),
            ModSource::HighMidHits => return hits(2),
            ModSource::TrebleHits => return hits(3),
            ModSource::Hits => return inputs.overall.hits.clamp(0.0, 1.0),
            _ => {}
        }
        let phase = (inputs.beat / self.beats.max(0.0625) as f64).rem_euclid(1.0) as f32;
        match self.source {
            ModSource::Sine => 0.5 - 0.5 * (phase * std::f32::consts::TAU).cos(),
            ModSource::Saw => phase,
            ModSource::Square => (phase < 0.5) as u8 as f32,
            ModSource::Pulse => (1.0 - phase).powi(4),
            _ => 0.0,
        }
    }

    /// Applies the modulation to a normalised base value.
    pub fn apply(&self, base: f32, inputs: &ModInputs) -> f32 {
        if !self.is_active() {
            return base;
        }
        (base + self.amount * self.signal(inputs)).clamp(0.0, 1.0)
    }

    /// Like [`Modulation::apply`], but wraps around 0..1 instead of clamping (angles, hues).
    pub fn apply_wrapped(&self, base: f32, inputs: &ModInputs) -> f32 {
        if !self.is_active() {
            return base;
        }
        (base + self.amount * self.signal(inputs)).rem_euclid(1.0)
    }

    /// Index into [`MOD_RATES`] closest to this modulation's cycle length.
    pub fn rate_index(&self) -> usize {
        MOD_RATES
            .iter()
            .enumerate()
            .min_by(|a, b| (a.1 - self.beats).abs().total_cmp(&(b.1 - self.beats).abs()))
            .map(|(i, _)| i)
            .unwrap_or(2)
    }
}

/// A normalised 0..1 parameter with an optional modulator.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Param {
    pub value: f32,
    #[serde(default)]
    pub modulation: Modulation,
}

impl Param {
    pub fn new(value: f32) -> Self {
        Self { value, modulation: Modulation::default() }
    }

    pub fn eval(&self, inputs: &ModInputs) -> f32 {
        self.modulation.apply(self.value, inputs)
    }

    /// Evaluates a parameter that wraps around (angles, hues).
    pub fn eval_wrapped(&self, inputs: &ModInputs) -> f32 {
        self.modulation.apply_wrapped(self.value, inputs)
    }
}

/// Keeps the beat position continuous when the tempo changes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeatClock {
    bpm: f32,
    anchor_time: f64,
    anchor_beat: f64,
}

impl BeatClock {
    pub fn new(bpm: f32) -> Self {
        Self { bpm, anchor_time: 0.0, anchor_beat: 0.0 }
    }

    pub fn bpm(&self) -> f32 {
        self.bpm
    }

    pub fn beat_at(&self, now: f64) -> f64 {
        self.anchor_beat + crate::beats(now - self.anchor_time, self.bpm)
    }

    /// Changes tempo at `now` without jumping the beat position.
    pub fn set_bpm(&mut self, now: f64, bpm: f32) {
        if bpm != self.bpm {
            self.anchor_beat = self.beat_at(now);
            self.anchor_time = now;
            self.bpm = bpm;
        }
    }

    /// Adopts an external beat position and tempo (e.g. from Ableton Link).
    pub fn sync(&mut self, now: f64, beat: f64, bpm: f32) {
        self.anchor_time = now;
        self.anchor_beat = beat;
        self.bpm = bpm;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(beat: f64, bands: [f32; BAND_COUNT]) -> ModInputs {
        ModInputs { beat, bands, ..Default::default() }
    }

    #[test]
    fn hits_and_level_sources() {
        let mut i = inputs(0.0, [0.0; 4]);
        i.band_signals[2].hits = 0.7;
        i.overall = AudioSignal { level: 0.4, hits: 0.9, ..Default::default() };
        let m = |source| Modulation { source, amount: 1.0, beats: 1.0 };
        assert!((m(ModSource::HighMidHits).signal(&i) - 0.7).abs() < 1e-6);
        assert_eq!(m(ModSource::BassHits).signal(&i), 0.0);
        assert!((m(ModSource::Hits).signal(&i) - 0.9).abs() < 1e-6);
        assert!((m(ModSource::Level).signal(&i) - 0.4).abs() < 1e-6);
        assert_eq!(ModSource::ALL[ModSource::FIRST_TEMPO], ModSource::Sine);
        assert!(ModSource::ALL[..ModSource::FIRST_TEMPO].iter().all(|s| !s.is_beat()));
        let json = serde_json::to_string(&m(ModSource::BassHits)).unwrap();
        assert!(json.contains("\"bass_hits\""), "{json}");
    }

    #[test]
    fn off_leaves_the_value_alone() {
        let m = Modulation::default();
        assert_eq!(m.apply(0.3, &inputs(0.7, [1.0; 4])), 0.3);
    }

    #[test]
    fn audio_band_adds_scaled_level_and_clamps() {
        let m = Modulation { source: ModSource::HighMid, amount: 0.5, beats: 1.0 };
        let i = inputs(0.0, [0.0, 0.0, 0.8, 0.0]);
        assert!((m.apply(0.2, &i) - 0.6).abs() < 1e-6);
        assert_eq!(m.apply(0.9, &i), 1.0);
        let neg = Modulation { amount: -1.0, ..m };
        assert_eq!(neg.apply(0.5, &i), 0.0);
    }

    #[test]
    fn beat_shapes_follow_the_phase() {
        let saw = Modulation { source: ModSource::Saw, amount: 1.0, beats: 2.0 };
        assert!((saw.signal(&inputs(1.0, [0.0; 4])) - 0.5).abs() < 1e-6);
        assert!((saw.signal(&inputs(3.0, [0.0; 4])) - 0.5).abs() < 1e-6);
        let sine = Modulation { source: ModSource::Sine, ..saw };
        assert!(sine.signal(&inputs(0.0, [0.0; 4])).abs() < 1e-6);
        assert!((sine.signal(&inputs(1.0, [0.0; 4])) - 1.0).abs() < 1e-6);
        let square = Modulation { source: ModSource::Square, beats: 1.0, ..saw };
        assert_eq!(square.signal(&inputs(0.25, [0.0; 4])), 1.0);
        assert_eq!(square.signal(&inputs(0.75, [0.0; 4])), 0.0);
        let pulse = Modulation { source: ModSource::Pulse, beats: 1.0, ..saw };
        assert_eq!(pulse.signal(&inputs(4.0, [0.0; 4])), 1.0);
        assert!(pulse.signal(&inputs(4.5, [0.0; 4])) < 0.1);
    }

    #[test]
    fn rate_index_snaps_to_nearest() {
        assert_eq!(Modulation { beats: 1.0, ..Default::default() }.rate_index(), 2);
        assert_eq!(Modulation { beats: 3.5, ..Default::default() }.rate_index(), 4);
    }

    #[test]
    fn beat_clock_is_continuous_across_tempo_changes() {
        let mut c = BeatClock::new(120.0);
        assert_eq!(c.beat_at(1.0), 2.0);
        c.set_bpm(1.0, 60.0);
        assert_eq!(c.beat_at(1.0), 2.0);
        assert_eq!(c.beat_at(2.0), 3.0);
        c.sync(5.0, 100.0, 120.0);
        assert_eq!(c.beat_at(5.5), 101.0);
    }

    #[test]
    fn modulation_defaults_when_missing_from_json() {
        let p: Param = serde_json::from_str(r#"{"value":0.25}"#).unwrap();
        assert_eq!(p, Param::new(0.25));
        let m: Modulation = serde_json::from_str(r#"{"source":"bass"}"#).unwrap();
        assert_eq!(m.amount, 1.0);
    }
}
