//! The Player's playlists: presets played one after another by themselves, as in
//! openDJ's Automix, saved as `.ovjplaylist` files apart from compositions.

use serde::{Deserialize, Serialize};

use crate::Clip;

/// File extension of a saved playlist.
pub const PLAYLIST_EXTENSION: &str = "ovjplaylist";

/// Beats in a bar (openVJ counts 4/4).
const BEATS_PER_BAR: f64 = 4.0;

/// The least a "Song" preset plays, in beats (16 bars), so a breakdown doesn't end it.
pub const SONG_MIN_BEATS: f64 = 16.0 * BEATS_PER_BAR;

/// The most a "Song" preset plays, in seconds, when no new song is heard.
pub const SONG_MAX_SECS: f64 = 300.0;

/// Previous presets remembered for Previous.
const HISTORY: usize = 64;

/// How long something lasts: a number of bars at the current tempo, seconds, or (for how
/// long a preset plays) until the song changes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Span {
    Bars(f32),
    Seconds(f32),
    /// Until openVJ hears a new song, between [`SONG_MIN_BEATS`] and [`SONG_MAX_SECS`].
    Song,
}

impl Span {
    /// The default time each preset plays.
    pub const DEFAULT_PLAY: Span = Span::Bars(32.0);
    /// The default transition between presets.
    pub const DEFAULT_TRANSITION: Span = Span::Bars(2.0);

    /// The choices in the LENGTH menu, under TIME and BARS headings, as in openDJ.
    pub const TRANSITIONS: [Span; 7] = [
        Span::Seconds(0.0),
        Span::Seconds(2.0),
        Span::Seconds(4.0),
        Span::Seconds(8.0),
        Span::Bars(1.0),
        Span::Bars(2.0),
        Span::Bars(4.0),
    ];

    pub fn label(self) -> String {
        match self {
            Span::Seconds(s) if s <= 0.0 => "0 seconds (cut)".into(),
            Span::Seconds(s) => format!("{} seconds", trim(s)),
            Span::Bars(1.0) => "1 bar".into(),
            Span::Bars(b) => format!("{} bars", trim(b)),
            Span::Song => "Until the song changes".into(),
        }
    }

    /// How far into this span `elapsed` is, 0 at the start and 1 at the end or later.
    fn progress(self, elapsed: Elapsed) -> f64 {
        let (done, len) = match self {
            Span::Bars(b) => (elapsed.beats, b as f64 * BEATS_PER_BAR),
            Span::Seconds(s) => (elapsed.secs, s as f64),
            Span::Song => (elapsed.secs, SONG_MAX_SECS),
        };
        if len <= 0.0 { 1.0 } else { (done / len).clamp(0.0, 1.0) }
    }
}

fn trim(v: f32) -> String {
    if v.fract() == 0.0 { format!("{}", v as i64) } else { format!("{v}") }
}

/// How one preset hands over to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionStyle {
    #[default]
    Crossfade,
    Cut,
    FadeThroughBlack,
}

impl TransitionStyle {
    pub const ALL: [TransitionStyle; 3] = [Self::Crossfade, Self::Cut, Self::FadeThroughBlack];

    pub fn name(self) -> &'static str {
        match self {
            Self::Crossfade => "Crossfade",
            Self::Cut => "Cut",
            Self::FadeThroughBlack => "Fade through black",
        }
    }

    /// Opacities of the outgoing and incoming preset at transition progress `t` (0..1).
    pub fn weights(self, t: f32) -> (f32, f32) {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Crossfade => (1.0 - t, t),
            Self::Cut => (0.0, 1.0),
            Self::FadeThroughBlack if t < 0.5 => (1.0 - 2.0 * t, 0.0),
            Self::FadeThroughBlack => (0.0, 2.0 * t - 1.0),
        }
    }
}

/// One look in a playlist: a clip (generator, ISF file or media) with its parameter
/// values, mappings, transform and effects, and how long it plays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    /// The look itself. Its id is only meaningful while it plays.
    pub clip: Clip,
    /// How long it plays before the next preset comes in.
    #[serde(default = "default_play")]
    pub play_for: Span,
}

fn default_play() -> Span {
    Span::DEFAULT_PLAY
}

impl Preset {
    pub fn new(clip: Clip) -> Self {
        Self { clip, play_for: Span::DEFAULT_PLAY }
    }

    pub fn name(&self) -> &str {
        &self.clip.name
    }
}

/// An ordered list of presets and how to move through them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    pub version: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub presets: Vec<Preset>,
    /// LENGTH: how long the transition into the next preset takes.
    #[serde(default = "default_transition")]
    pub transition: Span,
    /// STYLE: how it looks.
    #[serde(default)]
    pub style: TransitionStyle,
    #[serde(default)]
    pub shuffle: bool,
    /// Start over after the last preset (otherwise the last one keeps playing).
    #[serde(default = "yes")]
    pub repeat: bool,
}

fn default_transition() -> Span {
    Span::DEFAULT_TRANSITION
}

fn yes() -> bool {
    true
}

impl Default for Playlist {
    fn default() -> Self {
        Self {
            version: Self::VERSION,
            name: String::new(),
            presets: Vec::new(),
            transition: Span::DEFAULT_TRANSITION,
            style: TransitionStyle::default(),
            shuffle: false,
            repeat: true,
        }
    }
}

impl Playlist {
    pub const VERSION: u32 = 1;

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let mut list: Self = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if list.version > Self::VERSION {
            return Err(format!("playlist version {} is newer than this openVJ supports", list.version));
        }
        list.version = Self::VERSION;
        Ok(list)
    }
}

/// Time since some moment, in both clocks: seconds and beats.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Elapsed {
    secs: f64,
    beats: f64,
}

/// A moment, in both clocks.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Moment {
    secs: f64,
    beats: f64,
}

impl Moment {
    fn since(self, now: Moment) -> Elapsed {
        Elapsed { secs: now.secs - self.secs, beats: now.beats - self.beats }
    }

    /// This moment moved on by `span`, at `bpm` to convert between the two clocks.
    fn after(self, span: Span, bpm: f32) -> Moment {
        let per_beat = 60.0 / bpm.max(1.0) as f64;
        match span {
            Span::Bars(b) => {
                let beats = b.max(0.0) as f64 * BEATS_PER_BAR;
                Moment { secs: self.secs + beats * per_beat, beats: self.beats + beats }
            }
            Span::Seconds(s) => {
                let secs = s.max(0.0) as f64;
                Moment { secs: self.secs + secs, beats: self.beats + secs / per_beat }
            }
            Span::Song => Moment { secs: self.secs + SONG_MAX_SECS, beats: self.beats + SONG_MAX_SECS / per_beat },
        }
    }
}

/// What the Player shows right now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutomixFrame {
    /// The preset coming in (or simply playing).
    pub current: usize,
    /// The preset going out, during a transition.
    pub previous: Option<usize>,
    /// Transition progress, 0..1; 1 when no transition is running.
    pub mix: f32,
}

impl AutomixFrame {
    /// Opacities of (previous, current) for the playlist's style.
    pub fn weights(&self, style: TransitionStyle) -> (f32, f32) {
        if self.previous.is_some() { style.weights(self.mix) } else { (0.0, 1.0) }
    }
}

/// Plays a playlist: knows which preset is on, when to move on, and how far a
/// transition has got. Times come from the app (seconds and beats).
#[derive(Debug, Clone)]
pub struct Automix {
    /// Play order: indices into the playlist's presets.
    order: Vec<usize>,
    /// Position in `order`.
    pos: usize,
    previous: Option<usize>,
    /// Presets played before the current one, oldest first, for Previous.
    history: Vec<usize>,
    /// When the current preset started (its transition in starts here too).
    started: Moment,
    /// Bumped on each reshuffle, so every pass gets a new order.
    pass: u64,
    seed: u64,
}

impl Automix {
    /// Starts at the first preset (or a random one when shuffling). The start snaps back to
    /// the last bar line, so bar-long presets change on bar lines. `None` for an empty playlist.
    pub fn start(list: &Playlist, now_secs: f64, beats: f64, bpm: f32, seed: u64) -> Option<Self> {
        if list.presets.is_empty() {
            return None;
        }
        let bar = (beats / BEATS_PER_BAR).floor() * BEATS_PER_BAR;
        let back = (beats - bar) * 60.0 / bpm.max(1.0) as f64;
        let mut a = Self {
            order: Vec::new(),
            pos: 0,
            previous: None,
            history: Vec::new(),
            started: Moment { secs: now_secs - back, beats: bar },
            pass: 0,
            seed,
        };
        a.order = a.make_order(list);
        Some(a)
    }

    fn make_order(&self, list: &Playlist) -> Vec<usize> {
        let mut order: Vec<usize> = (0..list.presets.len()).collect();
        if list.shuffle {
            // Fisher-Yates with a small stable generator, so tests are repeatable.
            let mut state = self.seed ^ self.pass.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            for i in (1..order.len()).rev() {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let j = ((state >> 33) % (i as u64 + 1)) as usize;
                order.swap(i, j);
            }
        }
        order
    }

    /// The preset playing now, as an index into the playlist.
    pub fn current(&self) -> usize {
        self.order[self.pos]
    }

    /// Advances the clock to `now` and says what to show. Moves to the next preset when the
    /// current one has played its time, keeping bar-long presets on bar lines.
    pub fn tick(&mut self, list: &Playlist, now_secs: f64, beats: f64, bpm: f32) -> AutomixFrame {
        let now = Moment { secs: now_secs, beats };
        self.fit(list);
        // Catch up, one preset at a time (more than one only after a long stall).
        for _ in 0..list.presets.len().max(1) {
            let play_for = list.presets.get(self.current()).map_or(Span::DEFAULT_PLAY, |p| p.play_for);
            if play_for.progress(self.started.since(now)) < 1.0 || !self.has_next(list) {
                break;
            }
            let ends = self.started.after(play_for, bpm);
            self.advance(list, ends);
        }
        self.frame(list, now)
    }

    /// Skips to the next preset now, with the playlist's transition.
    pub fn next(&mut self, list: &Playlist, now_secs: f64, beats: f64) {
        self.fit(list);
        if self.has_next(list) {
            self.advance(list, Moment { secs: now_secs, beats });
        }
    }

    /// Brings in preset `index` now, with the playlist's transition; the playlist then goes
    /// on from there.
    pub fn jump(&mut self, list: &Playlist, index: usize, now_secs: f64, beats: f64) {
        self.fit(list);
        if index >= list.presets.len() || index == self.current() {
            return;
        }
        self.remember();
        if let Some(pos) = self.order.iter().position(|&i| i == index) {
            self.pos = pos;
        }
        self.started = Moment { secs: now_secs, beats };
    }

    /// Brings back the preset that played before the current one. False when there is none.
    pub fn back(&mut self, list: &Playlist, now_secs: f64, beats: f64) -> bool {
        self.fit(list);
        while let Some(prev) = self.history.pop() {
            if prev < list.presets.len() && prev != self.current() {
                self.previous = Some(self.current());
                if let Some(pos) = self.order.iter().position(|&i| i == prev) {
                    self.pos = pos;
                }
                self.started = Moment { secs: now_secs, beats };
                return true;
            }
        }
        false
    }

    /// The audio says a new song started: a preset set to play until the song changes moves
    /// on, once it has played its minimum.
    pub fn song_changed(&mut self, list: &Playlist, now_secs: f64, beats: f64) -> bool {
        self.fit(list);
        let now = Moment { secs: now_secs, beats };
        let is_song = list.presets.get(self.current()).is_some_and(|p| p.play_for == Span::Song);
        if is_song && self.started.since(now).beats >= SONG_MIN_BEATS && self.has_next(list) {
            self.advance(list, now);
            return true;
        }
        false
    }

    fn remember(&mut self) {
        self.previous = Some(self.current());
        self.history.push(self.current());
        if self.history.len() > HISTORY {
            self.history.remove(0);
        }
    }

    fn has_next(&self, list: &Playlist) -> bool {
        list.repeat || self.pos + 1 < self.order.len()
    }

    fn advance(&mut self, list: &Playlist, at: Moment) {
        self.remember();
        self.pos += 1;
        if self.pos >= self.order.len() {
            self.pass += 1;
            self.order = self.make_order(list);
            self.pos = 0;
            // Don't play the same preset twice in a row across a reshuffle.
            if self.order.len() > 1 && Some(self.order[0]) == self.previous {
                self.order.swap(0, 1);
            }
        }
        self.started = at;
    }

    fn frame(&mut self, list: &Playlist, now: Moment) -> AutomixFrame {
        let t = list.transition.progress(self.started.since(now));
        if t >= 1.0 {
            self.previous = None;
        }
        AutomixFrame { current: self.current(), previous: self.previous, mix: t as f32 }
    }

    /// Keeps the order valid after presets were added or removed while playing.
    fn fit(&mut self, list: &Playlist) {
        if self.order.len() != list.presets.len() || self.order.iter().any(|&i| i >= list.presets.len()) {
            let current = self.order.get(self.pos).copied();
            self.order = self.make_order(list);
            self.pos = current.and_then(|c| self.order.iter().position(|&i| i == c)).unwrap_or(0);
            self.previous = self.previous.filter(|&p| p < list.presets.len());
            self.history.retain(|&p| p < list.presets.len());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipSource, Composition};

    fn preset(name: &str, play_for: Span) -> Preset {
        let mut comp = Composition::new(64, 64, 1, 1);
        comp.set_clip(0, 0, ClipSource::isf(name)).unwrap();
        let mut clip = comp.clip(0, 0).unwrap().clone();
        clip.name = name.into();
        Preset { clip, play_for }
    }

    fn list(n: usize, play_for: Span) -> Playlist {
        Playlist { presets: (0..n).map(|i| preset(&format!("p{i}"), play_for)).collect(), ..Playlist::default() }
    }

    /// At 120 BPM one beat is half a second.
    const BPM: f32 = 120.0;
    fn at(beats: f64) -> (f64, f64) {
        (beats * 0.5, beats)
    }

    #[test]
    fn plays_each_preset_for_its_bars_then_crossfades() {
        let mut l = list(3, Span::Bars(16.0));
        l.transition = Span::Bars(2.0);
        let mut a = Automix::start(&l, 0.0, 0.0, BPM, 1).unwrap();
        let (s, b) = at(63.0);
        assert_eq!(a.tick(&l, s, b, BPM), AutomixFrame { current: 0, previous: None, mix: 1.0 });
        // 16 bars = 64 beats; halfway through a 2-bar (8-beat) crossfade at beat 68.
        let (s, b) = at(68.0);
        let f = a.tick(&l, s, b, BPM);
        assert_eq!((f.current, f.previous), (1, Some(0)));
        assert!((f.mix - 0.5).abs() < 1e-6);
        assert_eq!(f.weights(l.style), (0.5, 0.5));
        let (s, b) = at(72.0);
        assert_eq!(a.tick(&l, s, b, BPM), AutomixFrame { current: 1, previous: None, mix: 1.0 });
    }

    #[test]
    fn start_snaps_back_to_the_bar_line() {
        let l = list(2, Span::Bars(1.0));
        // Started at beat 6 (mid bar 2): the first preset counts from beat 4, so it ends at 8.
        let (s, b) = at(6.0);
        let mut a = Automix::start(&l, s, b, BPM, 1).unwrap();
        let (s, b) = at(7.9);
        assert_eq!(a.tick(&l, s, b, BPM).current, 0);
        let (s, b) = at(8.0);
        assert_eq!(a.tick(&l, s, b, BPM).current, 1);
    }

    #[test]
    fn seconds_and_cut() {
        let mut l = list(2, Span::Seconds(10.0));
        l.transition = Span::Seconds(0.0);
        let mut a = Automix::start(&l, 0.0, 0.0, BPM, 1).unwrap();
        assert_eq!(a.tick(&l, 9.9, 19.8, BPM).current, 0);
        assert_eq!(a.tick(&l, 10.0, 20.0, BPM), AutomixFrame { current: 1, previous: None, mix: 1.0 });
    }

    #[test]
    fn repeat_wraps_and_no_repeat_holds_the_last() {
        let mut l = list(2, Span::Bars(1.0));
        l.transition = Span::Seconds(0.0);
        let mut a = Automix::start(&l, 0.0, 0.0, BPM, 1).unwrap();
        let (s, b) = at(8.0);
        assert_eq!(a.tick(&l, s, b, BPM).current, 0, "wrapped after two presets");
        l.repeat = false;
        let mut a = Automix::start(&l, 0.0, 0.0, BPM, 1).unwrap();
        let (s, b) = at(40.0);
        assert_eq!(a.tick(&l, s, b, BPM).current, 1, "holds the last preset");
    }

    #[test]
    fn shuffle_plays_every_preset_once_per_pass() {
        let mut l = list(5, Span::Bars(1.0));
        l.shuffle = true;
        l.transition = Span::Seconds(0.0);
        let mut a = Automix::start(&l, 0.0, 0.0, BPM, 42).unwrap();
        let mut seen = vec![a.current()];
        for bar in 1..5 {
            let (s, b) = at(bar as f64 * 4.0);
            seen.push(a.tick(&l, s, b, BPM).current);
        }
        seen.sort();
        assert_eq!(seen, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn next_skips_now_with_the_transition() {
        let l = list(3, Span::Bars(32.0));
        let mut a = Automix::start(&l, 0.0, 0.0, BPM, 1).unwrap();
        let (s, b) = at(10.0);
        a.next(&l, s, b);
        let (s, b) = at(14.0);
        let f = a.tick(&l, s, b, BPM);
        assert_eq!((f.current, f.previous), (1, Some(0)));
        assert!((f.mix - 0.5).abs() < 1e-6);
    }

    #[test]
    fn jump_and_back() {
        let l = list(4, Span::Bars(32.0));
        let mut a = Automix::start(&l, 0.0, 0.0, BPM, 1).unwrap();
        let (s, b) = at(4.0);
        a.jump(&l, 2, s, b);
        let (s, b) = at(8.0);
        let f = a.tick(&l, s, b, BPM);
        assert_eq!((f.current, f.previous), (2, Some(0)), "crossfading from 0 into 2");
        let (s, b) = at(20.0);
        assert!(a.back(&l, s, b));
        assert_eq!(a.current(), 0);
        // After 0 comes 1 again in list order, and there's nothing more to go back to.
        let (s, b) = at(24.0);
        assert!(!a.back(&l, s, b));
        let (s, b) = at(40.0);
        a.next(&l, s, b);
        assert_eq!(a.current(), 1);
    }

    #[test]
    fn back_follows_what_played_with_shuffle() {
        let mut l = list(6, Span::Bars(1.0));
        l.shuffle = true;
        l.transition = Span::Seconds(0.0);
        let mut a = Automix::start(&l, 0.0, 0.0, BPM, 7).unwrap();
        let mut played = vec![a.current()];
        for bar in 1..4 {
            let (s, b) = at(bar as f64 * 4.0);
            played.push(a.tick(&l, s, b, BPM).current);
        }
        let (s, b) = at(14.0);
        assert!(a.back(&l, s, b));
        assert_eq!(a.current(), played[2]);
    }

    #[test]
    fn song_presets_wait_for_the_song_to_change() {
        let mut l = list(2, Span::Song);
        l.transition = Span::Seconds(0.0);
        let mut a = Automix::start(&l, 0.0, 0.0, BPM, 1).unwrap();
        // A change heard before 16 bars (a breakdown) is ignored.
        let (s, b) = at(32.0);
        assert!(!a.song_changed(&l, s, b));
        let (s, b) = at(200.0);
        assert_eq!(a.tick(&l, s, b, BPM).current, 0, "no timer while waiting for the song");
        assert!(a.song_changed(&l, s, b));
        assert_eq!(a.current(), 1);
        // Five minutes without a change: it moves on anyway.
        let f = a.tick(&l, 100.0 + SONG_MAX_SECS, 200.0 + SONG_MAX_SECS * 2.0, BPM);
        assert_eq!(f.current, 0);
        // A bars preset ignores song changes.
        let l = list(2, Span::Bars(32.0));
        let mut a = Automix::start(&l, 0.0, 0.0, BPM, 1).unwrap();
        let (s, b) = at(100.0);
        assert!(!a.song_changed(&l, s, b));
    }

    #[test]
    fn fade_through_black_goes_dark_in_the_middle() {
        let s = TransitionStyle::FadeThroughBlack;
        assert_eq!(s.weights(0.0), (1.0, 0.0));
        assert_eq!(s.weights(0.5), (0.0, 0.0));
        assert_eq!(s.weights(1.0), (0.0, 1.0));
        assert_eq!(TransitionStyle::Cut.weights(0.1), (0.0, 1.0));
    }

    #[test]
    fn survives_removing_presets_while_playing() {
        let mut l = list(3, Span::Bars(1.0));
        let mut a = Automix::start(&l, 0.0, 0.0, BPM, 1).unwrap();
        let (s, b) = at(4.0);
        a.tick(&l, s, b, BPM);
        l.presets.truncate(1);
        let (s, b) = at(5.0);
        assert_eq!(a.tick(&l, s, b, BPM).current, 0);
    }

    #[test]
    fn saves_and_loads() {
        let mut l = list(2, Span::Seconds(12.0));
        l.name = "Friday".into();
        l.style = TransitionStyle::FadeThroughBlack;
        l.shuffle = true;
        let back = Playlist::from_json(&l.to_json().unwrap()).unwrap();
        assert_eq!(back, l);
        let mut v: serde_json::Value = serde_json::from_str(&l.to_json().unwrap()).unwrap();
        v["version"] = 99.into();
        assert!(Playlist::from_json(&v.to_string()).is_err());
    }

    #[test]
    fn minimal_file_gets_defaults() {
        let l = Playlist::from_json(r#"{"version":1}"#).unwrap();
        assert_eq!(l.transition, Span::DEFAULT_TRANSITION);
        assert!(l.repeat && !l.shuffle && l.presets.is_empty());
    }

    #[test]
    fn labels() {
        assert_eq!(Span::Seconds(0.0).label(), "0 seconds (cut)");
        assert_eq!(Span::Bars(1.0).label(), "1 bar");
        assert_eq!(Span::Bars(32.0).label(), "32 bars");
        assert_eq!(Span::Seconds(2.5).label(), "2.5 seconds");
        assert_eq!(Span::Song.label(), "Until the song changes");
        assert_eq!(serde_json::to_string(&Span::Song).unwrap(), "\"song\"");
    }
}
