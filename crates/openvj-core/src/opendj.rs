//! What openVJ knows about the music playing in openDJ, from the events openDJ
//! sends over OSC (`/beat`, `/phrase`, `/deck/N/track`, `/deck/N/cue`).
//!
//! Tempo and beat phase still come from Ableton Link or MIDI clock; this adds
//! the music's structure: where the master deck is in its phrase, the phrase's
//! name, and the tracks loaded on the decks.

/// One event from openDJ, already decoded from its OSC message.
#[derive(Debug, Clone, PartialEq)]
pub enum DjEvent {
    /// A beat of the master deck: beat in the bar (1–4), bar in the phrase (from 1), BPM.
    Beat { beat: u32, bar: u32, bpm: f32 },
    /// The master deck started a phrase: Intro, Build, Drop, Breakdown, Chorus, Verse,
    /// Outro, or "Phrase" for a track without phrase analysis; its length in bars.
    Phrase { name: String, bars: u32 },
    /// A track loaded on a deck (1-based).
    Track { deck: u32, title: String, artist: String, bpm: f32, key: String },
    /// A hot cue fired on a deck (1-based deck and cue number).
    Cue { deck: u32, cue: u32 },
}

/// A track loaded on one of openDJ's decks.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DjTrack {
    pub title: String,
    pub artist: String,
    pub bpm: f32,
    /// The key as openDJ writes it ("8A", "Am", "F#m"…).
    pub key: String,
}

/// openDJ's master deck as openVJ last heard it.
#[derive(Debug, Clone, Default)]
pub struct DjState {
    /// Beat in the bar (1–4) of the last `/beat`; 0 before the first.
    pub beat: u32,
    /// Bar in the phrase (from 1) of the last `/beat`; 0 before the first.
    pub bar: u32,
    pub bpm: f32,
    /// The current phrase's name, once a `/phrase` arrived.
    pub phrase: Option<String>,
    /// The current phrase's length in bars, once a `/phrase` arrived.
    pub phrase_bars: u32,
    /// Tracks by deck (index 0 = deck 1).
    pub decks: Vec<Option<DjTrack>>,
    /// The deck whose track matches the master deck's tempo best, if any.
    pub master_deck: Option<usize>,
    /// Counts every phrase start, so callers can tell a new phrase from the same one.
    pub phrase_count: u64,
    /// Counts every hot cue, with the last one fired.
    pub cue_count: u64,
    pub last_cue: Option<(u32, u32)>,
}

impl DjState {
    /// Takes one event into the state.
    pub fn apply(&mut self, event: DjEvent) {
        match event {
            DjEvent::Beat { beat, bar, bpm } => {
                // A bar number lower than the last one without a `/phrase` means a phrase
                // started that openDJ didn't name (its first phrase, or a jump).
                if bar < self.bar && bar <= 1 && self.phrase.is_some() {
                    self.phrase = None;
                    self.phrase_bars = 0;
                }
                self.beat = beat;
                self.bar = bar;
                self.bpm = bpm;
                self.master_deck = self.guess_master();
            }
            DjEvent::Phrase { name, bars } => {
                self.phrase = Some(name);
                self.phrase_bars = bars;
                self.bar = 1;
                self.beat = 1;
                self.phrase_count += 1;
            }
            DjEvent::Track { deck, title, artist, bpm, key } => {
                let i = deck.saturating_sub(1) as usize;
                if i >= 16 {
                    return;
                }
                if self.decks.len() <= i {
                    self.decks.resize(i + 1, None);
                }
                self.decks[i] = Some(DjTrack { title, artist, bpm, key });
                self.master_deck = self.guess_master();
            }
            DjEvent::Cue { deck, cue } => {
                self.last_cue = Some((deck, cue));
                self.cue_count += 1;
            }
        }
    }

    /// The master deck's track, when openVJ can tell which deck that is.
    pub fn master_track(&self) -> Option<&DjTrack> {
        self.decks.get(self.master_deck?)?.as_ref()
    }

    /// How far through the current phrase the master deck is, 0 at its first beat
    /// to just under 1 at its last; None until a phrase with a length arrived.
    pub fn phrase_position(&self) -> Option<f32> {
        if self.phrase.is_none() || self.phrase_bars == 0 || self.bar == 0 {
            return None;
        }
        let beats = (self.phrase_bars * 4) as f32;
        let done = ((self.bar - 1) * 4 + self.beat.clamp(1, 4) - 1) as f32;
        Some((done / beats).clamp(0.0, 1.0))
    }

    /// A short label for the toolbar: "DROP 5/16", "BAR 3", or "" before the first beat.
    pub fn label(&self) -> String {
        match (&self.phrase, self.bar) {
            (_, 0) => String::new(),
            (Some(name), bar) if self.phrase_bars > 0 => {
                format!("{} {}/{}", name.to_uppercase(), bar.min(self.phrase_bars), self.phrase_bars)
            }
            (Some(name), bar) => format!("{} {}", name.to_uppercase(), bar),
            (None, bar) => format!("BAR {bar}"),
        }
    }

    /// openDJ doesn't say which deck is the master, so pick the loaded deck whose BPM
    /// is closest to the beat's (allowing for the tempo fader, half and double time).
    /// With only one loaded deck, that one.
    fn guess_master(&self) -> Option<usize> {
        let loaded: Vec<(usize, &DjTrack)> =
            self.decks.iter().enumerate().filter_map(|(i, t)| t.as_ref().map(|t| (i, t))).collect();
        if loaded.len() == 1 {
            return Some(loaded[0].0);
        }
        if self.bpm <= 0.0 {
            return self.master_deck;
        }
        let distance = |t: &DjTrack| {
            if t.bpm <= 0.0 {
                return f32::MAX;
            }
            [0.5, 1.0, 2.0].iter().map(|m| (t.bpm * m / self.bpm).ln().abs()).fold(f32::MAX, f32::min)
        };
        loaded
            .iter()
            .map(|&(i, t)| (i, distance(t)))
            .filter(|&(_, d)| d < 0.1)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
            .or(self.master_deck)
    }
}

/// A key's place on the Camelot wheel: (1–12, minor). Reads Camelot ("8A", "12B"),
/// Open Key ("1m", "6d") and standard names ("Am", "F#m", "Bb", "C# minor", "Ebmaj").
pub fn camelot(key: &str) -> Option<(u8, bool)> {
    let k = key.trim();
    if k.is_empty() {
        return None;
    }
    let lower = k.to_ascii_lowercase();
    // Camelot: number then A (minor) or B (major).
    if let Some(n) = lower
        .strip_suffix('a')
        .or_else(|| lower.strip_suffix('b'))
        .and_then(|n| n.parse::<u8>().ok())
        .filter(|n| (1..=12).contains(n))
    {
        return Some((n, lower.ends_with('a')));
    }
    // Open Key: number then m (minor) or d (major); 1m = 6A (A minor), 1d = 6B (C major).
    if let Some(n) = lower
        .strip_suffix('m')
        .or_else(|| lower.strip_suffix('d'))
        .and_then(|n| n.parse::<u8>().ok())
        .filter(|n| (1..=12).contains(n))
    {
        return Some(((n + 4) % 12 + 1, lower.ends_with('m')));
    }
    // Standard names.
    let mut chars = k.chars();
    let letter = chars.next()?.to_ascii_uppercase();
    let base: i32 = match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest = chars.as_str();
    // A sharp or flat sign right after the letter ("Bb" is B flat: no mode word starts with b).
    let (shift, rest) = if let Some(r) = rest.strip_prefix(['#', '♯']) {
        (1, r)
    } else if let Some(r) = rest.strip_prefix(['b', '♭']) {
        (-1, r)
    } else {
        (0, rest)
    };
    let mode = rest.trim();
    let minor = match mode.to_ascii_lowercase().as_str() {
        _ if mode == "M" => false,
        "" | "maj" | "major" => false,
        "m" | "min" | "minor" => true,
        _ => return None,
    };
    let pitch = (base + shift).rem_euclid(12) as usize;
    // Camelot numbers by pitch class: majors (B) and minors (A).
    const MAJOR: [u8; 12] = [8, 3, 10, 5, 12, 7, 2, 9, 4, 11, 6, 1];
    const MINOR: [u8; 12] = [5, 12, 7, 2, 9, 4, 11, 6, 1, 8, 3, 10];
    Some((if minor { MINOR[pitch] } else { MAJOR[pitch] }, minor))
}

/// A key as a value from 0 to 1 around the Camelot wheel (1A/1B at 0, 12A/12B at 11/12),
/// so neighbouring keys get neighbouring values: suited to driving a hue.
pub fn key_value(key: &str) -> Option<f32> {
    camelot(key).map(|(n, _)| (n - 1) as f32 / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(deck: u32, bpm: f32, key: &str) -> DjEvent {
        DjEvent::Track { deck, title: format!("T{deck}"), artist: "A".into(), bpm, key: key.into() }
    }

    #[test]
    fn phrase_and_bars_make_a_label_and_a_position() {
        let mut s = DjState::default();
        assert_eq!(s.label(), "");
        s.apply(DjEvent::Beat { beat: 1, bar: 3, bpm: 124.0 });
        assert_eq!(s.label(), "BAR 3");
        assert_eq!(s.phrase_position(), None);
        s.apply(DjEvent::Phrase { name: "Drop".into(), bars: 16 });
        assert_eq!(s.phrase_count, 1);
        s.apply(DjEvent::Beat { beat: 1, bar: 1, bpm: 124.0 });
        assert_eq!(s.label(), "DROP 1/16");
        assert_eq!(s.phrase_position(), Some(0.0));
        s.apply(DjEvent::Beat { beat: 3, bar: 9, bpm: 124.0 });
        assert_eq!(s.label(), "DROP 9/16");
        assert!((s.phrase_position().unwrap() - 34.0 / 64.0).abs() < 1e-6);
    }

    #[test]
    fn an_unnamed_restart_forgets_the_phrase() {
        let mut s = DjState::default();
        s.apply(DjEvent::Phrase { name: "Build".into(), bars: 8 });
        s.apply(DjEvent::Beat { beat: 4, bar: 8, bpm: 128.0 });
        s.apply(DjEvent::Beat { beat: 1, bar: 1, bpm: 128.0 });
        assert_eq!(s.phrase, None);
        assert_eq!(s.label(), "BAR 1");
    }

    #[test]
    fn master_deck_is_the_one_at_the_beat_tempo() {
        let mut s = DjState::default();
        s.apply(track(1, 124.0, "8A"));
        assert_eq!(s.master_track().unwrap().title, "T1");
        s.apply(track(2, 140.0, "5B"));
        s.apply(DjEvent::Beat { beat: 1, bar: 1, bpm: 141.0 });
        assert_eq!(s.master_track().unwrap().title, "T2");
        // Half time still counts.
        s.apply(DjEvent::Beat { beat: 2, bar: 1, bpm: 62.5 });
        assert_eq!(s.master_track().unwrap().title, "T1");
    }

    #[test]
    fn cues_count() {
        let mut s = DjState::default();
        s.apply(DjEvent::Cue { deck: 2, cue: 3 });
        assert_eq!((s.cue_count, s.last_cue), (1, Some((2, 3))));
    }

    #[test]
    fn keys_map_to_the_camelot_wheel() {
        assert_eq!(camelot("8A"), Some((8, true)));
        assert_eq!(camelot("12b"), Some((12, false)));
        assert_eq!(camelot("Am"), Some((8, true)));
        assert_eq!(camelot("C"), Some((8, false)));
        assert_eq!(camelot("F#m"), Some((11, true)));
        assert_eq!(camelot("Bb"), Some((6, false)));
        assert_eq!(camelot("Bbm"), Some((3, true)));
        assert_eq!(camelot("Eb minor"), Some((2, true)));
        assert_eq!(camelot("B"), Some((1, false)));
        assert_eq!(camelot("1m"), Some((6, true)));
        assert_eq!(camelot("1d"), Some((6, false)));
        assert_eq!(camelot(""), None);
        assert_eq!(camelot("H"), None);
        assert_eq!(key_value("1A"), Some(0.0));
        assert_eq!(key_value("7B"), Some(0.5));
    }
}
