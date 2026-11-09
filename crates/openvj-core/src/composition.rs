use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::playback::position;
use crate::{
    ClipTime, DEFAULT_CLIP_SECS, Effect, GeneratorKind, MidiMap, ModInputs, Modulation, PARAM_COUNT, PlayMode,
};

/// File extensions treated as still images.
pub const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp"];
/// File extensions treated as video (decoded by FFmpeg).
pub const VIDEO_EXTENSIONS: &[&str] = &["mov", "mp4", "m4v", "mkv", "webm", "avi", "mxf", "mpg", "mpeg"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClipId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    #[default]
    Normal,
    Add,
    Screen,
    Multiply,
    Overlay,
    Difference,
    Lighten,
    Darken,
    Subtract,
}

impl BlendMode {
    pub const ALL: [BlendMode; 9] = [
        BlendMode::Normal,
        BlendMode::Add,
        BlendMode::Screen,
        BlendMode::Multiply,
        BlendMode::Overlay,
        BlendMode::Difference,
        BlendMode::Lighten,
        BlendMode::Darken,
        BlendMode::Subtract,
    ];

    pub fn name(self) -> &'static str {
        match self {
            BlendMode::Normal => "Normal",
            BlendMode::Add => "Add",
            BlendMode::Screen => "Screen",
            BlendMode::Multiply => "Multiply",
            BlendMode::Overlay => "Overlay",
            BlendMode::Difference => "Difference",
            BlendMode::Lighten => "Lighten",
            BlendMode::Darken => "Darken",
            BlendMode::Subtract => "Subtract",
        }
    }

    /// Index the WGSL blend shader switches on. Must match `fs_blend`.
    pub fn shader_id(self) -> u32 {
        Self::ALL.iter().position(|m| *m == self).unwrap() as u32
    }

    pub fn from_index(i: usize) -> Self {
        Self::ALL.get(i).copied().unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClipSource {
    Generator {
        kind: GeneratorKind,
    },
    /// An ISF generator file (bundled or from an effects folder), with its parameter values
    /// saved by name. Its `kind` is the generator's id.
    Isf {
        generator: Effect,
    },
    Image {
        path: PathBuf,
    },
    Video {
        path: PathBuf,
    },
    /// A camera or capture device (webcam, capture card, OBS Virtual Camera), found by name.
    Camera {
        device: String,
        /// The OS's unique device id (AVFoundation `uniqueID` on macOS), when known. Used to
        /// tell apart two devices with the same name; the name wins if the id has changed.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        device_id: String,
    },
    /// A Syphon server on macOS, as Syphon names it: the app publishing it, plus the
    /// server's own name (often empty, or the same as the app).
    Syphon {
        app: String,
        #[serde(default)]
        server: String,
    },
}

impl ClipSource {
    /// Picks image or video by file extension.
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
            Some(ClipSource::Image { path: path.to_owned() })
        } else if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
            Some(ClipSource::Video { path: path.to_owned() })
        } else {
            None
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self {
            ClipSource::Generator { .. } | ClipSource::Isf { .. } => "gen",
            ClipSource::Image { .. } => "image",
            ClipSource::Video { .. } => "video",
            ClipSource::Camera { .. } => "camera",
            ClipSource::Syphon { .. } => "syphon",
        }
    }

    pub fn media_path(&self) -> Option<&Path> {
        match self {
            ClipSource::Image { path } | ClipSource::Video { path } => Some(path),
            _ => None,
        }
    }

    /// An ISF generator with default values. Unknown ids give a clip that shows as missing.
    pub fn isf(id: &str) -> Self {
        ClipSource::Isf { generator: Effect::new(id) }
    }

    /// True for generators, built-in or ISF: drawn on the GPU, with live thumbnails.
    pub fn is_generator(&self) -> bool {
        matches!(self, ClipSource::Generator { .. } | ClipSource::Isf { .. })
    }

    /// True for sources that stream from another device or app rather than a file.
    pub fn is_live(&self) -> bool {
        matches!(self, ClipSource::Camera { .. } | ClipSource::Syphon { .. })
    }

    /// Readable name of a live source, e.g. "FaceTime HD Camera" or "Synesthesia".
    pub fn live_name(&self) -> Option<String> {
        match self {
            ClipSource::Camera { device, .. } => Some(device.clone()),
            ClipSource::Syphon { app, server } => Some(syphon_display_name(app, server)),
            _ => None,
        }
    }

    fn default_name(&self) -> String {
        match self {
            ClipSource::Generator { kind } => kind.name().to_owned(),
            ClipSource::Isf { generator } => generator.name(),
            ClipSource::Image { path } | ClipSource::Video { path } => {
                path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "clip".into())
            }
            ClipSource::Camera { .. } | ClipSource::Syphon { .. } => self.live_name().unwrap_or_default(),
        }
    }
}

/// How a Syphon server is shown: "App - Server", or just the app when the server has no
/// name of its own or repeats the app's (Synesthesia publishes "Synesthesia - Synesthesia").
pub fn syphon_display_name(app: &str, server: &str) -> String {
    if server.is_empty() || server == app {
        app.to_owned()
    } else if app.is_empty() {
        server.to_owned()
    } else {
        format!("{app} - {server}")
    }
}

impl ClipSource {
    /// A Syphon source from an "App - Server" label, as Resolume saves it.
    pub fn syphon_from_label(label: &str) -> Self {
        let (app, server) = label.split_once(" - ").unwrap_or((label, ""));
        ClipSource::Syphon { app: app.trim().to_owned(), server: server.trim().to_owned() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub id: ClipId,
    pub name: String,
    pub source: ClipSource,
    /// Generator parameters, normalised 0..1. Unused for media clips.
    pub params: [f32; PARAM_COUNT],
    /// Playback speed multiplier (1.0 = normal).
    pub speed: f32,
    /// One modulator per generator parameter.
    #[serde(default)]
    pub mods: [Modulation; PARAM_COUNT],
    /// What happens at the end of the clip.
    #[serde(default)]
    pub play_mode: PlayMode,
    /// Beat sync: one pass through the clip lasts this many beats, whatever the media's own
    /// length, and `speed` is ignored. `None` plays at the media's own pace (times `speed`).
    #[serde(default)]
    pub sync_beats: Option<f32>,
    /// Length in seconds of a still image or generator clip, for the play modes that need an
    /// end. `None` uses [`DEFAULT_CLIP_SECS`]. Videos use their own length.
    #[serde(default)]
    pub duration: Option<f32>,
    /// Length of the video file in seconds, once known (from the file or an import).
    #[serde(default)]
    pub media_secs: Option<f64>,
}

impl Clip {
    pub fn param_names(&self) -> [&'static str; PARAM_COUNT] {
        match self.source {
            ClipSource::Generator { kind } => kind.params().map(|p| p.name),
            _ => [""; PARAM_COUNT],
        }
    }

    /// Generator parameters with their modulators applied.
    pub fn param_values(&self, inputs: &ModInputs) -> [f32; PARAM_COUNT] {
        std::array::from_fn(|i| self.mods[i].apply(self.params[i], inputs))
    }

    fn new(id: ClipId, source: ClipSource) -> Self {
        let params = match source {
            ClipSource::Generator { kind } => kind.default_params(),
            _ => [0.0; PARAM_COUNT],
        };
        Self {
            id,
            name: source.default_name(),
            source,
            params,
            speed: 1.0,
            mods: Default::default(),
            play_mode: PlayMode::Loop,
            sync_beats: None,
            duration: None,
            media_secs: None,
        }
    }

    /// The clip's length at normal speed, in seconds, or `None` for a live input or a video
    /// whose length isn't known yet.
    pub fn length(&self) -> Option<f64> {
        match self.source {
            ClipSource::Video { .. } => self.media_secs.filter(|s| *s > 0.0),
            // Live inputs have no length; they just run.
            ClipSource::Camera { .. } | ClipSource::Syphon { .. } => None,
            _ => Some(self.duration.unwrap_or(DEFAULT_CLIP_SECS).max(0.01) as f64),
        }
    }

    /// Where the clip is after playing for `secs` seconds, or `beats` beats.
    pub fn time_at(&self, secs: f64, beats: f64) -> ClipTime {
        let speed = self.speed as f64;
        let Some(len) = self.length() else {
            return ClipTime { secs: secs.max(0.0) * speed, ended: false };
        };
        let cycles = match self.sync_beats {
            Some(b) if b > 0.0 => beats / b as f64,
            _ => secs * speed / len,
        };
        let wrap = !self.source.is_generator();
        position(self.play_mode, cycles, beats, len, wrap, self.id.0)
    }
}

/// Which side of the crossfader a layer is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossfadeSide {
    /// Not affected by the crossfader.
    #[default]
    None,
    A,
    B,
}

impl CrossfadeSide {
    pub const ALL: [CrossfadeSide; 3] = [CrossfadeSide::A, CrossfadeSide::None, CrossfadeSide::B];

    pub fn label(self) -> &'static str {
        match self {
            CrossfadeSide::None => "–",
            CrossfadeSide::A => "A",
            CrossfadeSide::B => "B",
        }
    }
}

/// How the two sides fade as the crossfader moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossfadeCurve {
    /// A fades out as B fades in: half and half in the middle.
    #[default]
    Linear,
    /// Both sides stay at full strength until the fader passes the middle, like a DJ mixer.
    FullCentre,
}

impl CrossfadeCurve {
    pub const ALL: [CrossfadeCurve; 2] = [CrossfadeCurve::Linear, CrossfadeCurve::FullCentre];

    pub fn name(self) -> &'static str {
        match self {
            CrossfadeCurve::Linear => "Linear",
            CrossfadeCurve::FullCentre => "Full in the middle",
        }
    }

    pub fn from_index(i: usize) -> Self {
        Self::ALL.get(i).copied().unwrap_or_default()
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|c| *c == self).unwrap()
    }
}

/// The composition's A/B crossfader. Layers assigned to a side fade with it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Crossfader {
    /// 0 = all A, 1 = all B.
    pub position: f32,
    #[serde(default)]
    pub position_mod: Modulation,
    #[serde(default)]
    pub curve: CrossfadeCurve,
}

impl Default for Crossfader {
    fn default() -> Self {
        Self { position: 0.5, position_mod: Modulation::default(), curve: CrossfadeCurve::Linear }
    }
}

impl Crossfader {
    pub fn position_value(&self, inputs: &ModInputs) -> f32 {
        self.position_mod.apply(self.position, inputs)
    }

    /// Opacity multiplier for a layer on `side`.
    pub fn gain(&self, side: CrossfadeSide, inputs: &ModInputs) -> f32 {
        let p = self.position_value(inputs);
        let towards = match side {
            CrossfadeSide::None => return 1.0,
            CrossfadeSide::A => 1.0 - p,
            CrossfadeSide::B => p,
        };
        match self.curve {
            CrossfadeCurve::Linear => towards,
            CrossfadeCurve::FullCentre => (towards * 2.0).min(1.0),
        }
    }
}

/// Layers mixed together first, then blended into the composition as one, with their own
/// opacity, blend mode and effects. A group's layers sit next to each other in the stack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerGroup {
    pub name: String,
    pub opacity: f32,
    #[serde(default)]
    pub opacity_mod: Modulation,
    pub blend: BlendMode,
    pub bypass: bool,
    pub solo: bool,
    /// Applied to the group's mix before it is blended.
    #[serde(default)]
    pub effects: Vec<Effect>,
}

impl LayerGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            opacity: 1.0,
            opacity_mod: Modulation::default(),
            blend: BlendMode::Normal,
            bypass: false,
            solo: false,
            effects: Vec::new(),
        }
    }

    pub fn opacity_value(&self, inputs: &ModInputs) -> f32 {
        self.opacity_mod.apply(self.opacity, inputs)
    }
}

/// A clip that is currently running on a layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Playing {
    pub clip_id: ClipId,
    /// Time (seconds, composition clock) the clip was launched.
    pub started_at: f64,
    /// Beat (tempo clock) the clip was launched on, for beat-synced clips.
    pub started_beat: f64,
}

impl Playing {
    /// How far into `clip` this playback is at `now` / `beat`.
    pub fn time(&self, clip: &Clip, now: f64, beat: f64) -> ClipTime {
        clip.time_at(now - self.started_at, beat - self.started_beat)
    }
}

/// A crossfade in progress on a layer. `from == None` fades in from nothing,
/// and a layer with `active == None` fades out to nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fade {
    pub from: Option<Playing>,
    pub started_at: f64,
    pub duration: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub opacity: f32,
    pub blend: BlendMode,
    pub bypass: bool,
    pub solo: bool,
    /// Crossfade time when switching clips, in seconds. 0 = cut.
    pub transition_secs: f32,
    #[serde(default)]
    pub opacity_mod: Modulation,
    /// Applied to whatever the layer plays, top to bottom, before it is blended.
    #[serde(default)]
    pub effects: Vec<Effect>,
    /// Index into [`Composition::groups`].
    #[serde(default)]
    pub group: Option<usize>,
    #[serde(default)]
    pub crossfade: CrossfadeSide,
    /// One slot per composition column.
    pub clips: Vec<Option<Clip>>,
    #[serde(skip)]
    pub active: Option<Playing>,
    #[serde(skip)]
    pub fade: Option<Fade>,
    /// Copies of clips from other decks that are still playing or fading on this layer.
    /// Switching decks doesn't cut what's on screen, the same as in Resolume.
    #[serde(skip)]
    pub held: Vec<Clip>,
}

/// What a layer should draw this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerFrame<'a> {
    /// The clip being faded to (or simply playing), with its local time in seconds.
    pub incoming: Option<(&'a Clip, f64)>,
    /// The clip being faded away from, with its local time in seconds.
    pub outgoing: Option<(&'a Clip, f64)>,
    /// Crossfade position: 0 = all outgoing, 1 = all incoming.
    pub mix: f32,
}

impl Layer {
    pub fn new(name: impl Into<String>, columns: usize) -> Self {
        Self {
            name: name.into(),
            opacity: 1.0,
            blend: BlendMode::Normal,
            bypass: false,
            solo: false,
            transition_secs: 0.0,
            opacity_mod: Modulation::default(),
            effects: Vec::new(),
            group: None,
            crossfade: CrossfadeSide::None,
            clips: vec![None; columns],
            active: None,
            fade: None,
            held: Vec::new(),
        }
    }

    pub fn opacity_value(&self, inputs: &ModInputs) -> f32 {
        self.opacity_mod.apply(self.opacity, inputs)
    }

    pub fn clip_by_id(&self, id: ClipId) -> Option<&Clip> {
        self.clips.iter().flatten().chain(&self.held).find(|c| c.id == id)
    }

    /// Ids of the clips this layer is drawing (playing or fading out).
    fn live_ids(&self) -> impl Iterator<Item = ClipId> {
        self.active.map(|p| p.clip_id).into_iter().chain(self.fade.and_then(|f| f.from).map(|p| p.clip_id))
    }

    pub fn column_of(&self, id: ClipId) -> Option<usize> {
        self.clips.iter().position(|c| c.as_ref().is_some_and(|c| c.id == id))
    }

    /// Column of the clip currently playing (the incoming one during a fade).
    pub fn playing_column(&self) -> Option<usize> {
        self.active.and_then(|p| self.column_of(p.clip_id))
    }

    fn launch(&mut self, clip_id: ClipId, now: f64, beat: f64) {
        let next = Playing { clip_id, started_at: now, started_beat: beat };
        let retrigger = self.active.is_some_and(|p| p.clip_id == clip_id);
        if self.transition_secs > 0.0 && !retrigger {
            self.fade = Some(Fade { from: self.active, started_at: now, duration: self.transition_secs as f64 });
        } else {
            self.fade = None;
        }
        self.active = Some(next);
    }

    pub fn clear(&mut self, now: f64) {
        if self.active.is_none() {
            return;
        }
        if self.transition_secs > 0.0 {
            self.fade = Some(Fade { from: self.active, started_at: now, duration: self.transition_secs as f64 });
        } else {
            self.fade = None;
        }
        self.active = None;
    }

    /// Ends finished fades, clears the layer when a play-once-and-clear clip ends, and lets
    /// go of held clips that are no longer on screen.
    fn tick(&mut self, now: f64, beat: f64) {
        if self.fade.is_some_and(|f| now - f.started_at >= f.duration) {
            self.fade = None;
        }
        let ended = self.active.is_some_and(|p| {
            self.clip_by_id(p.clip_id).is_some_and(|c| c.play_mode == PlayMode::OnceClear && p.time(c, now, beat).ended)
        });
        if ended {
            self.clear(now);
        }
        if !self.held.is_empty() {
            let live: Vec<ClipId> = self.live_ids().collect();
            self.held.retain(|c| live.contains(&c.id));
        }
    }

    fn local_time(&self, p: Playing, now: f64, beat: f64) -> Option<(&Clip, f64)> {
        let clip = self.clip_by_id(p.clip_id)?;
        Some((clip, p.time(clip, now, beat).secs))
    }

    /// What to draw at `now` (seconds) and `beat` (tempo clock), or `None` if the layer is empty.
    pub fn frame(&self, now: f64, beat: f64) -> Option<LayerFrame<'_>> {
        let incoming = self.active.and_then(|p| self.local_time(p, now, beat));
        let (outgoing, mix) = match self.fade {
            Some(f) => {
                let t = ((now - f.started_at) / f.duration).clamp(0.0, 1.0) as f32;
                (f.from.and_then(|p| self.local_time(p, now, beat)), t)
            }
            None => (None, 1.0),
        };
        if incoming.is_none() && outgoing.is_none() {
            return None;
        }
        Some(LayerFrame { incoming, outgoing, mix })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
}

/// A page of clips. Every deck shares the composition's layers and effects but has its own
/// columns and clips, like Resolume's decks.
///
/// The open deck's columns and clips live in [`Composition::columns`] and [`Layer::clips`],
/// so its entry here holds only the name. Other decks keep theirs here until opened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Deck {
    pub name: String,
    #[serde(default)]
    pub columns: Vec<Column>,
    /// One row of slots per layer, bottom of the mix first.
    #[serde(default)]
    pub clips: Vec<Vec<Option<Clip>>>,
}

impl Deck {
    fn named(name: impl Into<String>) -> Self {
        Self { name: name.into(), columns: Vec::new(), clips: Vec::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Composition {
    /// Bumped whenever the file format changes incompatibly.
    pub version: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub master: f32,
    #[serde(default)]
    pub master_mod: Modulation,
    pub bpm: f32,
    pub columns: Vec<Column>,
    /// Index 0 is the bottom of the mix.
    pub layers: Vec<Layer>,
    /// Applied to the whole mix before the master fader.
    #[serde(default)]
    pub effects: Vec<Effect>,
    /// MIDI controller mappings, saved with the composition.
    #[serde(default, skip_serializing_if = "MidiMap::is_empty")]
    pub midi: MidiMap,
    /// Every deck, in tab order. See [`Deck`] for where the open deck's clips are kept.
    #[serde(default)]
    pub decks: Vec<Deck>,
    /// Index into `decks` of the deck shown in the grid.
    #[serde(default)]
    pub current_deck: usize,
    /// Layer groups, referenced by [`Layer::group`].
    #[serde(default)]
    pub groups: Vec<LayerGroup>,
    #[serde(default)]
    pub crossfader: Crossfader,
    next_id: u64,
    /// Clock reading from the last [`Composition::tick`]: (seconds, beat).
    #[serde(skip)]
    clock: (f64, f64),
}

impl Default for Composition {
    fn default() -> Self {
        Self::new(1280, 720, 4, 8)
    }
}

impl Composition {
    pub const VERSION: u32 = 1;

    pub fn new(width: u32, height: u32, layers: usize, columns: usize) -> Self {
        let mut comp = Self {
            version: Self::VERSION,
            name: "Untitled".into(),
            width,
            height,
            master: 1.0,
            master_mod: Modulation::default(),
            bpm: 120.0,
            columns: Vec::new(),
            layers: Vec::new(),
            effects: Vec::new(),
            midi: MidiMap::default(),
            decks: vec![Deck::named("Deck 1")],
            current_deck: 0,
            groups: Vec::new(),
            crossfader: Crossfader::default(),
            next_id: 1,
            clock: (0.0, 0.0),
        };
        for _ in 0..columns {
            comp.add_column();
        }
        for _ in 0..layers {
            comp.add_layer();
        }
        comp
    }

    /// A composition pre-filled with generators so there is something to play on first launch.
    pub fn demo() -> Self {
        use GeneratorKind::*;
        let mut comp = Self::new(1280, 720, 4, 8);
        comp.name = "Demo".into();
        // (layer, column, generator); layer 0 is the bottom of the mix.
        let clips = [
            (0, 0, Plasma),
            (0, 1, Gradient),
            (0, 2, Noise),
            (0, 3, Tunnel),
            (0, 4, Checker),
            (1, 0, Rings),
            (1, 1, Kaleido),
            (1, 2, Bars),
            (1, 4, Tunnel),
            (2, 1, Bars),
            (2, 3, Rings),
            (2, 4, Strobe),
        ];
        for (layer, col, kind) in clips {
            comp.set_clip(layer, col, ClipSource::Generator { kind });
        }
        comp.layers[1].blend = BlendMode::Multiply;
        comp.layers[1].opacity = 0.6;
        comp.layers[2].blend = BlendMode::Add;
        comp.layers[2].opacity = 0.4;
        comp.layers[3].name = "Media".into();

        // Show off modulation and effects: the plasma warps with the bass, the base layer
        // cycles its hue over four bars, and the top generator layer splits on the bass.
        use crate::{Effect, ModSource};
        let bass = Modulation { source: ModSource::Bass, amount: 0.6, beats: 1.0 };
        comp.clip_mut(0, 0).expect("demo clip").mods[3] = bass;
        let mut color = Effect::new("color");
        color.params.get_mut("hue").expect("hue").modulation =
            Modulation { source: ModSource::Saw, amount: 1.0, beats: 16.0 };
        comp.layers[0].effects.push(color);
        let mut split = Effect::new("rgb_split");
        let amount = split.params.get_mut("amount").expect("amount");
        amount.value = 0.1;
        amount.modulation = bass;
        comp.layers[2].effects.push(split);
        comp
    }

    fn next_clip_id(&mut self) -> ClipId {
        let id = ClipId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Adds a layer on top of the stack and returns its index.
    pub fn add_layer(&mut self) -> usize {
        let name = format!("Layer {}", self.layers.len() + 1);
        self.layers.push(Layer::new(name, self.columns.len()));
        for deck in self.other_decks_mut() {
            deck.clips.push(vec![None; deck.columns.len()]);
        }
        self.layers.len() - 1
    }

    pub fn remove_layer(&mut self, layer: usize) {
        if layer < self.layers.len() && self.layers.len() > 1 {
            self.layers.remove(layer);
            for deck in self.other_decks_mut() {
                if layer < deck.clips.len() {
                    deck.clips.remove(layer);
                }
            }
            self.prune_groups();
        }
    }

    /// Puts `layer` in a group. Joining a group moves the layer next to the group's other
    /// layers (just above them), keeping groups together in the stack. Returns the layer's
    /// new index.
    pub fn set_layer_group(&mut self, layer: usize, group: Option<usize>) -> usize {
        if layer >= self.layers.len() || group.is_some_and(|g| g >= self.groups.len()) {
            return layer;
        }
        let mut layer = layer;
        self.layers[layer].group = None;
        if let Some(g) = group {
            let members: Vec<usize> = (0..self.layers.len()).filter(|&i| self.layers[i].group == Some(g)).collect();
            if let (Some(&lo), Some(&hi)) = (members.first(), members.last())
                && !(lo.saturating_sub(1)..=hi + 1).contains(&layer)
            {
                let at = if layer < lo { hi } else { hi + 1 };
                self.move_layer(layer, at);
                layer = at;
            }
            self.layers[layer].group = Some(g);
        }
        self.prune_groups();
        layer
    }

    /// Makes a new group holding `layers` (indices) and returns the group's index.
    /// The layers should be next to each other.
    pub fn group_layers(&mut self, layers: &[usize]) -> Option<usize> {
        let layers: Vec<usize> = layers.iter().copied().filter(|&l| l < self.layers.len()).collect();
        if layers.is_empty() {
            return None;
        }
        let g = self.groups.len();
        self.groups.push(LayerGroup::new(format!("Group {}", g + 1)));
        for l in layers {
            self.layers[l].group = Some(g);
        }
        self.prune_groups();
        self.layers.iter().filter_map(|l| l.group).max()
    }

    /// Moves a layer to another place in the stack (in every deck), shifting the ones between.
    pub fn move_layer(&mut self, from: usize, to: usize) {
        if from >= self.layers.len() || to >= self.layers.len() || from == to {
            return;
        }
        let l = self.layers.remove(from);
        self.layers.insert(to, l);
        for deck in self.other_decks_mut() {
            if from < deck.clips.len() && to < deck.clips.len() {
                let row = deck.clips.remove(from);
                deck.clips.insert(to, row);
            }
        }
    }

    /// Indices of the layers in a group, bottom first.
    pub fn group_members(&self, group: usize) -> Vec<usize> {
        (0..self.layers.len()).filter(|&i| self.layers[i].group == Some(group)).collect()
    }

    /// Drops groups without layers and renumbers the rest, keeping their order.
    fn prune_groups(&mut self) {
        let used: Vec<bool> = (0..self.groups.len()).map(|g| self.layers.iter().any(|l| l.group == Some(g))).collect();
        if used.iter().all(|u| *u) {
            return;
        }
        let mut remap = vec![None; self.groups.len()];
        let mut kept = Vec::new();
        for (g, group) in std::mem::take(&mut self.groups).into_iter().enumerate() {
            if used[g] {
                remap[g] = Some(kept.len());
                kept.push(group);
            }
        }
        self.groups = kept;
        for l in &mut self.layers {
            l.group = l.group.and_then(|g| remap.get(g).copied().flatten());
        }
    }

    // ------------------------------------------------------------ decks

    fn other_decks_mut(&mut self) -> impl Iterator<Item = &mut Deck> {
        let current = self.current_deck;
        self.decks.iter_mut().enumerate().filter(move |(i, _)| *i != current).map(|(_, d)| d)
    }

    pub fn deck_name(&self) -> &str {
        self.decks.get(self.current_deck).map_or("", |d| d.name.as_str())
    }

    /// Shows another deck in the grid. Clips that are playing keep playing until something
    /// else is launched on their layer.
    pub fn switch_deck(&mut self, index: usize) {
        if index == self.current_deck || index >= self.decks.len() {
            return;
        }
        // Hold on to whatever is on screen from the deck being closed.
        for layer in &mut self.layers {
            let live: Vec<ClipId> = layer.live_ids().collect();
            let keep: Vec<Clip> = layer.clips.iter().flatten().filter(|c| live.contains(&c.id)).cloned().collect();
            layer.held.extend(keep);
        }
        let closing = self.current_deck;
        self.decks[closing].columns = std::mem::take(&mut self.columns);
        self.decks[closing].clips = self.layers.iter_mut().map(|l| std::mem::take(&mut l.clips)).collect();

        let opening = &mut self.decks[index];
        self.columns = std::mem::take(&mut opening.columns);
        let mut rows = std::mem::take(&mut opening.clips);
        self.current_deck = index;
        if self.columns.is_empty() {
            self.columns.push(Column { name: "1".into() });
        }
        rows.resize_with(self.layers.len(), Vec::new);
        let cols = self.columns.len();
        for (layer, mut row) in self.layers.iter_mut().zip(rows) {
            row.resize(cols, None);
            layer.clips = row;
            // A clip that's back in the grid no longer needs its held copy.
            let back: Vec<ClipId> = layer.clips.iter().flatten().map(|c| c.id).collect();
            layer.held.retain(|c| !back.contains(&c.id));
        }
    }

    /// Adds an empty deck after the others, with as many columns as the open one, and opens it.
    pub fn add_deck(&mut self) -> usize {
        let cols = self.columns.len().max(1);
        let mut deck = Deck::named(format!("Deck {}", self.decks.len() + 1));
        deck.columns = (0..cols).map(|i| Column { name: (i + 1).to_string() }).collect();
        deck.clips = vec![vec![None; cols]; self.layers.len()];
        self.decks.push(deck);
        let index = self.decks.len() - 1;
        self.switch_deck(index);
        index
    }

    pub fn rename_deck(&mut self, index: usize, name: &str) {
        if let Some(deck) = self.decks.get_mut(index)
            && !name.trim().is_empty()
        {
            deck.name = name.trim().to_owned();
        }
    }

    /// Removes a deck and its clips. The last deck can't be removed.
    pub fn remove_deck(&mut self, index: usize) {
        if index >= self.decks.len() || self.decks.len() <= 1 {
            return;
        }
        if index == self.current_deck {
            self.switch_deck(if index == 0 { 1 } else { index - 1 });
        }
        self.decks.remove(index);
        if self.current_deck > index {
            self.current_deck -= 1;
        }
    }

    pub fn add_column(&mut self) -> usize {
        self.columns.push(Column { name: format!("{}", self.columns.len() + 1) });
        for layer in &mut self.layers {
            layer.clips.push(None);
        }
        self.columns.len() - 1
    }

    pub fn remove_column(&mut self, col: usize) {
        if col >= self.columns.len() || self.columns.len() <= 1 {
            return;
        }
        self.columns.remove(col);
        for layer in &mut self.layers {
            if let Some(clip) = layer.clips.remove(col)
                && layer.active.is_some_and(|p| p.clip_id == clip.id)
            {
                layer.active = None;
                layer.fade = None;
            }
        }
    }

    pub fn clip(&self, layer: usize, col: usize) -> Option<&Clip> {
        self.layers.get(layer)?.clips.get(col)?.as_ref()
    }

    pub fn clip_mut(&mut self, layer: usize, col: usize) -> Option<&mut Clip> {
        self.layers.get_mut(layer)?.clips.get_mut(col)?.as_mut()
    }

    pub fn find_clip(&self, id: ClipId) -> Option<(usize, usize)> {
        self.layers.iter().enumerate().find_map(|(l, layer)| layer.column_of(id).map(|c| (l, c)))
    }

    /// Puts a new clip in a slot (replacing what was there) and returns its id.
    pub fn set_clip(&mut self, layer: usize, col: usize, source: ClipSource) -> Option<ClipId> {
        if layer >= self.layers.len() || col >= self.columns.len() {
            return None;
        }
        self.remove_clip(layer, col);
        let id = self.next_clip_id();
        self.layers[layer].clips[col] = Some(Clip::new(id, source));
        Some(id)
    }

    pub fn remove_clip(&mut self, layer: usize, col: usize) -> Option<Clip> {
        let l = self.layers.get_mut(layer)?;
        let clip = l.clips.get_mut(col)?.take()?;
        if l.active.is_some_and(|p| p.clip_id == clip.id) {
            l.active = None;
        }
        if l.fade.is_some_and(|f| f.from.is_some_and(|p| p.clip_id == clip.id)) {
            l.fade = None;
        }
        Some(clip)
    }

    /// Moves a clip between slots, swapping with whatever is at the destination.
    pub fn move_clip(&mut self, from: (usize, usize), to: (usize, usize)) {
        if from == to || self.clip(from.0, from.1).is_none() || to.0 >= self.layers.len() || to.1 >= self.columns.len()
        {
            return;
        }
        let a = self.layers[from.0].clips[from.1].take();
        let b = self.layers[to.0].clips[to.1].take();
        // Clips keep their ids but playback state belongs to the layer they leave.
        for (layer, clip) in [(from.0, &a), (to.0, &b)] {
            if let Some(clip) = clip
                && from.0 != to.0
                && self.layers[layer].active.is_some_and(|p| p.clip_id == clip.id)
            {
                self.layers[layer].active = None;
                self.layers[layer].fade = None;
            }
        }
        self.layers[to.0].clips[to.1] = a;
        self.layers[from.0].clips[from.1] = b;
    }

    /// The tempo clock's beat at `now`, extrapolated from the last [`Composition::tick`].
    pub fn beat_at(&self, now: f64) -> f64 {
        let (secs, beat) = self.clock;
        beat + crate::beats(now - secs, self.bpm)
    }

    /// Launches the clip in a slot. Empty slots do nothing.
    pub fn trigger(&mut self, layer: usize, col: usize, now: f64) {
        let beat = self.beat_at(now);
        if let Some(id) = self.clip(layer, col).map(|c| c.id) {
            self.layers[layer].launch(id, now, beat);
        }
    }

    /// Launches every clip in a column. Layers with an empty slot in that column are cleared.
    pub fn trigger_column(&mut self, col: usize, now: f64) {
        if col >= self.columns.len() {
            return;
        }
        let beat = self.beat_at(now);
        for layer in &mut self.layers {
            match layer.clips[col].as_ref().map(|c| c.id) {
                Some(id) => layer.launch(id, now, beat),
                None => layer.clear(now),
            }
        }
    }

    pub fn clear_layer(&mut self, layer: usize, now: f64) {
        if let Some(l) = self.layers.get_mut(layer) {
            l.clear(now);
        }
    }

    pub fn clear_all(&mut self, now: f64) {
        for l in &mut self.layers {
            l.clear(now);
        }
    }

    /// Advances playback state (finishes fades and play-once clips). Call once per frame
    /// with the composition clock in seconds and the tempo clock in beats.
    pub fn tick(&mut self, now: f64, beat: f64) {
        self.clock = (now, beat);
        for l in &mut self.layers {
            l.tick(now, beat);
        }
    }

    pub fn group_of(&self, layer: &Layer) -> Option<&LayerGroup> {
        layer.group.and_then(|g| self.groups.get(g))
    }

    /// Layers that contribute to the mix, bottom to top, honouring bypass and solo on layers
    /// and their groups. Soloing a group solos all of its layers.
    pub fn visible_layers(&self) -> impl Iterator<Item = (usize, &Layer)> {
        let soloed = |l: &Layer| l.solo || self.group_of(l).is_some_and(|g| g.solo);
        let any_solo = self.layers.iter().any(soloed);
        self.layers
            .iter()
            .enumerate()
            .filter(move |(_, l)| !l.bypass && !self.group_of(l).is_some_and(|g| g.bypass) && (!any_solo || soloed(l)))
    }

    /// A layer's opacity after modulation and the crossfader.
    pub fn layer_opacity(&self, layer: &Layer, inputs: &ModInputs) -> f32 {
        layer.opacity_value(inputs) * self.crossfader.gain(layer.crossfade, inputs)
    }

    /// Ids of clips that are currently audible/visible (playing or fading).
    pub fn live_clip_ids(&self) -> Vec<ClipId> {
        self.layers.iter().flat_map(Layer::live_ids).collect()
    }

    pub fn master_value(&self, inputs: &ModInputs) -> f32 {
        self.master_mod.apply(self.master, inputs)
    }

    /// Clips in the open deck, plus clips from other decks still playing. These are the ones
    /// that can be drawn this frame.
    pub fn all_clips(&self) -> impl Iterator<Item = &Clip> {
        self.layers.iter().flat_map(|l| l.clips.iter().flatten().chain(&l.held))
    }

    /// Every clip in every deck, once each. Use it for media loading and file paths.
    pub fn every_clip(&self) -> impl Iterator<Item = &Clip> {
        let open = self.layers.iter().flat_map(|l| l.clips.iter().flatten());
        let others = self.decks.iter().flat_map(|d| d.clips.iter().flatten().flatten());
        open.chain(others)
    }

    pub fn every_clip_mut(&mut self) -> impl Iterator<Item = &mut Clip> {
        let open = self.layers.iter_mut().flat_map(|l| l.clips.iter_mut().flatten().chain(l.held.iter_mut()));
        let others = self.decks.iter_mut().flat_map(|d| d.clips.iter_mut().flatten().flatten());
        open.chain(others)
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let mut comp: Self = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if comp.version > Self::VERSION {
            return Err(format!("composition version {} is newer than this openVJ supports", comp.version));
        }
        comp.version = Self::VERSION;
        if comp.columns.is_empty() {
            comp.add_column();
        }
        if comp.layers.is_empty() {
            comp.add_layer();
        }
        let cols = comp.columns.len();
        let groups = comp.groups.len();
        for l in &mut comp.layers {
            l.clips.resize(cols, None);
            l.group = l.group.filter(|g| *g < groups);
        }
        comp.prune_groups();
        // Files from before decks have none: their grid becomes the first deck.
        if comp.decks.is_empty() {
            comp.decks.push(Deck::named("Deck 1"));
        }
        comp.current_deck = comp.current_deck.min(comp.decks.len() - 1);
        let (layers, current) = (comp.layers.len(), comp.current_deck);
        for (i, deck) in comp.decks.iter_mut().enumerate() {
            if i == current {
                deck.columns.clear();
                deck.clips.clear();
                continue;
            }
            if deck.columns.is_empty() {
                deck.columns.push(Column { name: "1".into() });
            }
            deck.clips.resize_with(layers, Vec::new);
            for row in &mut deck.clips {
                row.resize(deck.columns.len(), None);
            }
        }
        let max_id = comp.every_clip().map(|c| c.id.0).max().unwrap_or(0);
        comp.next_id = comp.next_id.max(max_id + 1);
        Ok(comp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isf_generator_clips_save_by_name() {
        let mut c = Composition::new(64, 36, 1, 1);
        let mut source = ClipSource::isf("some_folder_generator");
        if let ClipSource::Isf { generator } = &mut source {
            generator.params.insert("zoom".into(), crate::Param::new(0.75));
        }
        c.set_clip(0, 0, source.clone());
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains(r#""type":"isf""#) && json.contains("some_folder_generator"), "{json}");
        let back: Composition = serde_json::from_str(&json).unwrap();
        let clip = back.clip(0, 0).unwrap();
        assert_eq!(clip.source, source);
        assert!(clip.source.is_generator());
        // Not installed here: the name falls back to the id.
        assert_eq!(clip.name, "some_folder_generator");
    }

    fn generator(kind: GeneratorKind) -> ClipSource {
        ClipSource::Generator { kind }
    }

    #[test]
    fn trigger_plays_and_empty_slot_is_ignored() {
        let mut c = Composition::new(640, 360, 2, 3);
        let id = c.set_clip(0, 1, generator(GeneratorKind::Plasma)).unwrap();
        c.trigger(0, 1, 1.0);
        assert_eq!(c.layers[0].active.map(|p| p.clip_id), Some(id));
        assert_eq!(c.layers[0].playing_column(), Some(1));
        c.trigger(0, 2, 2.0);
        assert_eq!(c.layers[0].playing_column(), Some(1));
    }

    #[test]
    fn column_trigger_clears_layers_with_empty_slots() {
        let mut c = Composition::new(640, 360, 2, 2);
        c.set_clip(0, 0, generator(GeneratorKind::Plasma));
        c.set_clip(1, 0, generator(GeneratorKind::Rings));
        c.set_clip(0, 1, generator(GeneratorKind::Bars));
        c.trigger_column(0, 0.0);
        assert!(c.layers.iter().all(|l| l.active.is_some()));
        c.trigger_column(1, 1.0);
        assert_eq!(c.layers[0].playing_column(), Some(1));
        assert!(c.layers[1].active.is_none());
    }

    #[test]
    fn crossfade_reports_mix_and_finishes() {
        let mut c = Composition::new(640, 360, 1, 2);
        let a = c.set_clip(0, 0, generator(GeneratorKind::Plasma)).unwrap();
        let b = c.set_clip(0, 1, generator(GeneratorKind::Rings)).unwrap();
        c.layers[0].transition_secs = 2.0;
        c.trigger(0, 0, 0.0);
        c.tick(5.0, 0.0);
        c.trigger(0, 1, 10.0);
        let f = c.layers[0].frame(11.0, 0.0).unwrap();
        assert_eq!(f.incoming.unwrap().0.id, b);
        assert_eq!(f.outgoing.unwrap().0.id, a);
        assert!((f.mix - 0.5).abs() < 1e-6);
        assert_eq!(f.incoming.unwrap().1, 1.0);
        c.tick(12.5, 0.0);
        let f = c.layers[0].frame(12.5, 0.0).unwrap();
        assert!(f.outgoing.is_none());
        assert_eq!(f.mix, 1.0);
    }

    #[test]
    fn clear_with_transition_fades_out() {
        let mut c = Composition::new(640, 360, 1, 1);
        c.set_clip(0, 0, generator(GeneratorKind::Plasma));
        c.layers[0].transition_secs = 1.0;
        c.trigger(0, 0, 0.0);
        c.tick(2.0, 0.0);
        c.clear_layer(0, 2.0);
        let f = c.layers[0].frame(2.25, 0.0).unwrap();
        assert!(f.incoming.is_none());
        assert!(f.outgoing.is_some());
        assert!((f.mix - 0.25).abs() < 1e-6);
        c.tick(3.5, 0.0);
        assert!(c.layers[0].frame(3.5, 0.0).is_none());
    }

    #[test]
    fn speed_scales_clip_time() {
        let mut c = Composition::new(640, 360, 1, 1);
        c.set_clip(0, 0, generator(GeneratorKind::Plasma));
        c.clip_mut(0, 0).unwrap().speed = 2.0;
        c.trigger(0, 0, 1.0);
        assert_eq!(c.layers[0].frame(2.0, 0.0).unwrap().incoming.unwrap().1, 2.0);
    }

    #[test]
    fn removing_a_playing_clip_stops_the_layer() {
        let mut c = Composition::new(640, 360, 1, 1);
        c.set_clip(0, 0, generator(GeneratorKind::Plasma));
        c.trigger(0, 0, 0.0);
        c.remove_clip(0, 0);
        assert!(c.layers[0].frame(1.0, 0.0).is_none());
    }

    #[test]
    fn solo_and_bypass_filter_visible_layers() {
        let mut c = Composition::new(640, 360, 3, 1);
        c.layers[0].bypass = true;
        assert_eq!(c.visible_layers().map(|(i, _)| i).collect::<Vec<_>>(), vec![1, 2]);
        c.layers[2].solo = true;
        assert_eq!(c.visible_layers().map(|(i, _)| i).collect::<Vec<_>>(), vec![2]);
    }

    #[test]
    fn columns_and_layers_stay_rectangular() {
        let mut c = Composition::new(640, 360, 2, 2);
        c.add_column();
        assert!(c.layers.iter().all(|l| l.clips.len() == 3));
        let id = c.set_clip(1, 2, generator(GeneratorKind::Noise)).unwrap();
        c.trigger(1, 2, 0.0);
        c.remove_column(2);
        assert!(c.layers.iter().all(|l| l.clips.len() == 2));
        assert!(c.find_clip(id).is_none());
        assert!(c.layers[1].active.is_none());
    }

    #[test]
    fn move_clip_swaps_slots() {
        let mut c = Composition::new(640, 360, 2, 2);
        let a = c.set_clip(0, 0, generator(GeneratorKind::Plasma)).unwrap();
        let b = c.set_clip(1, 1, generator(GeneratorKind::Rings)).unwrap();
        c.move_clip((0, 0), (1, 1));
        assert_eq!(c.find_clip(a), Some((1, 1)));
        assert_eq!(c.find_clip(b), Some((0, 0)));
    }

    #[test]
    fn json_round_trip_keeps_ids_unique() {
        let mut c = Composition::demo();
        c.trigger(0, 0, 0.0);
        let json = c.to_json().unwrap();
        let mut back = Composition::from_json(&json).unwrap();
        assert_eq!(back.layers.len(), c.layers.len());
        assert!(back.layers[0].active.is_none(), "playback state is not saved");
        let new_id = back.set_clip(3, 7, generator(GeneratorKind::Plasma)).unwrap();
        assert!(c.all_clips().all(|clip| clip.id != new_id));
    }

    #[test]
    fn effects_and_modulation_survive_json() {
        use crate::ModSource;
        let mut c = Composition::new(640, 360, 1, 1);
        c.set_clip(0, 0, generator(GeneratorKind::Plasma));
        c.clip_mut(0, 0).unwrap().mods[1].source = ModSource::Bass;
        c.layers[0].effects.push(Effect::new("blur"));
        c.layers[0].opacity_mod.source = ModSource::Pulse;
        c.effects.push(Effect::new("invert"));
        let back = Composition::from_json(&c.to_json().unwrap()).unwrap();
        assert_eq!(back.clip(0, 0).unwrap().mods[1].source, ModSource::Bass);
        assert_eq!(back.layers[0].effects, c.layers[0].effects);
        assert_eq!(back.layers[0].opacity_mod.source, ModSource::Pulse);
        assert_eq!(back.effects, c.effects);
    }

    #[test]
    fn version_1_files_without_modulation_still_load() {
        let mut c = Composition::new(640, 360, 1, 1);
        c.set_clip(0, 0, generator(GeneratorKind::Plasma));
        let mut v: serde_json::Value = serde_json::from_str(&c.to_json().unwrap()).unwrap();
        v.as_object_mut().unwrap().remove("effects");
        v.as_object_mut().unwrap().remove("master_mod");
        let layer = v["layers"][0].as_object_mut().unwrap();
        layer.remove("effects");
        layer.remove("opacity_mod");
        v["layers"][0]["clips"][0].as_object_mut().unwrap().remove("mods");
        let back = Composition::from_json(&v.to_string()).unwrap();
        assert!(back.layers[0].effects.is_empty());
        assert!(!back.clip(0, 0).unwrap().mods[0].is_active());
    }

    #[test]
    fn modulated_values() {
        use crate::ModSource;
        let mut c = Composition::new(640, 360, 1, 1);
        c.set_clip(0, 0, generator(GeneratorKind::Plasma));
        let clip = c.clip_mut(0, 0).unwrap();
        clip.params = [0.0; PARAM_COUNT];
        clip.mods[2] = Modulation { source: ModSource::Treble, amount: 0.5, beats: 1.0 };
        let inputs = ModInputs { beat: 0.0, bands: [0.0, 0.0, 0.0, 1.0] };
        assert_eq!(c.clip(0, 0).unwrap().param_values(&inputs), [0.0, 0.0, 0.5, 0.0]);
        c.layers[0].opacity = 0.2;
        c.layers[0].opacity_mod = Modulation { source: ModSource::Treble, amount: -1.0, beats: 1.0 };
        assert_eq!(c.layers[0].opacity_value(&inputs), 0.0);
        assert_eq!(c.master_value(&inputs), 1.0);
    }

    #[test]
    fn decks_keep_their_own_columns_and_clips() {
        let mut c = Composition::new(640, 360, 2, 3);
        let a = c.set_clip(0, 0, generator(GeneratorKind::Plasma)).unwrap();
        let second = c.add_deck();
        assert_eq!((second, c.current_deck), (1, 1));
        assert_eq!(c.columns.len(), 3, "a new deck copies the open deck's column count");
        assert!(c.clip(0, 0).is_none());
        let b = c.set_clip(1, 2, generator(GeneratorKind::Rings)).unwrap();
        c.add_column();

        c.switch_deck(0);
        assert_eq!(c.columns.len(), 3);
        assert_eq!(c.clip(0, 0).map(|x| x.id), Some(a));
        assert!(c.find_clip(b).is_none());
        c.switch_deck(1);
        assert_eq!(c.columns.len(), 4);
        assert_eq!(c.find_clip(b), Some((1, 2)));
        let ids: Vec<ClipId> = c.every_clip().map(|x| x.id).collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&a) && ids.contains(&b));
    }

    #[test]
    fn playing_clips_survive_a_deck_switch_until_replaced() {
        let mut c = Composition::new(640, 360, 1, 2);
        let a = c.set_clip(0, 0, generator(GeneratorKind::Plasma)).unwrap();
        c.trigger(0, 0, 0.0);
        c.add_deck();
        c.tick(1.0, 0.0);
        let frame = c.layers[0].frame(1.0, 0.0).expect("still playing after the switch");
        assert_eq!(frame.incoming.unwrap().0.id, a);
        assert!(c.all_clips().any(|x| x.id == a), "held clips are drawn");
        assert_eq!(c.layers[0].playing_column(), None, "but not shown in the new deck's grid");

        let b = c.set_clip(0, 1, generator(GeneratorKind::Rings)).unwrap();
        c.trigger(0, 1, 2.0);
        c.tick(2.0, 0.0);
        assert!(c.layers[0].held.is_empty());
        assert_eq!(c.layers[0].frame(2.0, 0.0).unwrap().incoming.unwrap().0.id, b);

        // Going back to a deck whose clip is still playing doesn't keep a duplicate.
        c.switch_deck(0);
        c.trigger(0, 0, 3.0);
        c.switch_deck(1);
        c.switch_deck(0);
        assert!(c.layers[0].held.is_empty());
        assert_eq!(c.all_clips().filter(|x| x.id == a).count(), 1);
    }

    #[test]
    fn layers_added_or_removed_apply_to_every_deck() {
        let mut c = Composition::new(640, 360, 2, 2);
        c.add_deck();
        c.set_clip(1, 0, generator(GeneratorKind::Bars));
        c.switch_deck(0);
        c.add_layer();
        c.remove_layer(0);
        c.switch_deck(1);
        assert_eq!(c.layers.len(), 2);
        assert!(c.layers.iter().all(|l| l.clips.len() == 2));
        assert!(c.clip(0, 0).is_some(), "layer 2's clip moved down with its layer");
    }

    #[test]
    fn removing_decks() {
        let mut c = Composition::new(640, 360, 1, 1);
        c.remove_deck(0);
        assert_eq!(c.decks.len(), 1, "the last deck stays");
        c.add_deck();
        c.add_deck();
        c.rename_deck(2, " Encore ");
        c.remove_deck(2);
        assert_eq!(c.decks.len(), 2);
        assert_eq!(c.current_deck, 1);
        c.switch_deck(0);
        c.remove_deck(1);
        assert_eq!((c.decks.len(), c.current_deck), (1, 0));
    }

    #[test]
    fn decks_survive_json_and_old_files_get_one() {
        let mut c = Composition::new(640, 360, 1, 2);
        c.set_clip(0, 0, generator(GeneratorKind::Plasma));
        c.add_deck();
        c.rename_deck(1, "Encore");
        let b = c.set_clip(0, 1, generator(GeneratorKind::Rings)).unwrap();
        c.switch_deck(0);
        let mut back = Composition::from_json(&c.to_json().unwrap()).unwrap();
        assert_eq!(back.decks.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), ["Deck 1", "Encore"]);
        assert_eq!(back.current_deck, 0);
        back.switch_deck(1);
        assert_eq!(back.find_clip(b), Some((0, 1)));
        let fresh = back.set_clip(0, 0, generator(GeneratorKind::Noise)).unwrap();
        assert!(c.every_clip().all(|x| x.id != fresh), "ids stay unique across decks");

        let mut v: serde_json::Value = serde_json::from_str(&c.to_json().unwrap()).unwrap();
        v.as_object_mut().unwrap().remove("decks");
        v.as_object_mut().unwrap().remove("current_deck");
        let old = Composition::from_json(&v.to_string()).unwrap();
        assert_eq!(old.decks.len(), 1);
        assert!(old.clip(0, 0).is_some());
    }

    fn video() -> ClipSource {
        ClipSource::Video { path: "loop.mov".into() }
    }

    #[test]
    fn video_loops_over_its_length_at_speed() {
        let mut c = Composition::new(640, 360, 1, 1);
        c.set_clip(0, 0, video());
        let clip = c.clip_mut(0, 0).unwrap();
        clip.media_secs = Some(4.0);
        clip.speed = 2.0;
        c.trigger(0, 0, 0.0);
        let t = c.layers[0].frame(3.0, 0.0).unwrap().incoming.unwrap().1;
        assert!((t - 2.0).abs() < 1e-9, "6 s of media wraps to 2 s, got {t}");
    }

    #[test]
    fn beat_sync_stretches_a_clip_to_the_beat() {
        let mut c = Composition::new(640, 360, 1, 1);
        c.set_clip(0, 0, video());
        let clip = c.clip_mut(0, 0).unwrap();
        clip.media_secs = Some(4.0);
        clip.sync_beats = Some(8.0);
        clip.speed = 3.0; // ignored while synced
        c.tick(10.0, 100.0);
        c.trigger(0, 0, 10.0);
        // Two beats in: a quarter of the way through the 4 s clip.
        let t = c.layers[0].frame(11.0, 102.0).unwrap().incoming.unwrap().1;
        assert!((t - 1.0).abs() < 1e-9, "got {t}");
        // The launch beat is extrapolated from the last tick at the composition's tempo.
        c.tick(20.0, 120.0);
        c.trigger(0, 0, 20.5);
        assert_eq!(c.layers[0].active.unwrap().started_beat, 121.0);
    }

    #[test]
    fn play_once_and_clear_empties_the_layer() {
        let mut c = Composition::new(640, 360, 1, 1);
        c.set_clip(0, 0, generator(GeneratorKind::Plasma));
        let clip = c.clip_mut(0, 0).unwrap();
        clip.play_mode = PlayMode::OnceClear;
        clip.duration = Some(2.0);
        c.trigger(0, 0, 0.0);
        c.tick(1.0, 2.0);
        assert!(c.layers[0].active.is_some());
        c.tick(2.5, 5.0);
        assert!(c.layers[0].active.is_none());

        c.clip_mut(0, 0).unwrap().play_mode = PlayMode::OnceHold;
        c.trigger(0, 0, 3.0);
        c.tick(10.0, 20.0);
        let t = c.layers[0].frame(10.0, 20.0).unwrap().incoming.unwrap().1;
        assert!(t > 1.99 && t < 2.0, "holds the last frame, got {t}");
    }

    #[test]
    fn groups_bypass_and_solo_their_layers() {
        let mut c = Composition::new(640, 360, 4, 1);
        let g = c.group_layers(&[1, 2]).unwrap();
        assert_eq!(c.group_members(g), vec![1, 2]);
        c.groups[g].bypass = true;
        assert_eq!(c.visible_layers().map(|(i, _)| i).collect::<Vec<_>>(), vec![0, 3]);
        c.groups[g].bypass = false;
        c.groups[g].solo = true;
        assert_eq!(c.visible_layers().map(|(i, _)| i).collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn joining_a_group_keeps_its_layers_together() {
        let mut c = Composition::new(640, 360, 5, 1);
        c.layers[4].name = "top".into();
        let g = c.group_layers(&[0, 1]).unwrap();
        let at = c.set_layer_group(4, Some(g));
        assert_eq!(at, 2);
        assert_eq!(c.layers[2].name, "top");
        assert_eq!(c.group_members(g), vec![0, 1, 2]);
        // Leaving the last layer of a group removes the group.
        for l in [0, 0, 0] {
            c.set_layer_group(c.group_members(0).first().copied().unwrap_or(l), None);
        }
        assert!(c.groups.is_empty());
        assert!(c.layers.iter().all(|l| l.group.is_none()));
    }

    #[test]
    fn moving_a_layer_moves_its_clips_in_other_decks() {
        let mut c = Composition::new(640, 360, 3, 1);
        c.add_deck();
        let id = c.set_clip(0, 0, generator(GeneratorKind::Bars)).unwrap();
        c.switch_deck(0);
        c.move_layer(0, 2);
        c.switch_deck(1);
        assert_eq!(c.find_clip(id), Some((2, 0)));
    }

    #[test]
    fn crossfader_fades_its_sides() {
        let mut c = Composition::new(640, 360, 3, 1);
        c.layers[0].crossfade = CrossfadeSide::A;
        c.layers[1].crossfade = CrossfadeSide::B;
        let inputs = ModInputs::default();
        c.crossfader.position = 0.25;
        let gains: Vec<f32> = c.layers.iter().map(|l| c.layer_opacity(l, &inputs)).collect();
        assert_eq!(gains, vec![0.75, 0.25, 1.0]);
        c.crossfader.curve = CrossfadeCurve::FullCentre;
        let gains: Vec<f32> = c.layers.iter().map(|l| c.layer_opacity(l, &inputs)).collect();
        assert_eq!(gains, vec![1.0, 0.5, 1.0]);
    }

    #[test]
    fn playback_groups_and_crossfader_survive_json() {
        let mut c = Composition::new(640, 360, 3, 1);
        c.set_clip(0, 0, generator(GeneratorKind::Plasma));
        let clip = c.clip_mut(0, 0).unwrap();
        clip.play_mode = PlayMode::Bounce;
        clip.sync_beats = Some(16.0);
        clip.duration = Some(12.0);
        let g = c.group_layers(&[1, 2]).unwrap();
        c.groups[g].name = "Visuals".into();
        c.groups[g].effects.push(Effect::new("blur"));
        c.layers[0].crossfade = CrossfadeSide::B;
        c.crossfader.position = 0.8;
        let back = Composition::from_json(&c.to_json().unwrap()).unwrap();
        assert_eq!(back.clip(0, 0), c.clip(0, 0));
        assert_eq!(back.groups, c.groups);
        assert_eq!(back.layers[2].group, Some(g));
        assert_eq!(back.layers[0].crossfade, CrossfadeSide::B);
        assert_eq!(back.crossfader, c.crossfader);
    }

    #[test]
    fn media_detection_by_extension() {
        assert!(matches!(ClipSource::from_path(Path::new("a/b.PNG")), Some(ClipSource::Image { .. })));
        assert!(matches!(ClipSource::from_path(Path::new("loop.mov")), Some(ClipSource::Video { .. })));
        assert!(ClipSource::from_path(Path::new("notes.txt")).is_none());
    }

    #[test]
    fn live_sources_are_named_after_their_device_or_app() {
        let mut c = Composition::new(640, 360, 1, 3);
        let cam = ClipSource::Camera { device: "FaceTime HD Camera".into(), device_id: String::new() };
        c.set_clip(0, 0, cam.clone());
        c.set_clip(0, 1, ClipSource::Syphon { app: "Synesthesia".into(), server: "Synesthesia".into() });
        c.set_clip(0, 2, ClipSource::Syphon { app: "TouchDesigner".into(), server: "TDSyphonSpoutOut".into() });
        assert_eq!(c.clip(0, 0).unwrap().name, "FaceTime HD Camera");
        assert_eq!(c.clip(0, 1).unwrap().name, "Synesthesia");
        assert_eq!(c.clip(0, 2).unwrap().name, "TouchDesigner - TDSyphonSpoutOut");
        assert!(cam.is_live() && cam.media_path().is_none());

        let back = Composition::from_json(&c.to_json().unwrap()).unwrap();
        assert_eq!(back.layers, c.layers);
        assert!(!c.to_json().unwrap().contains("device_id"), "an unknown id isn't saved");

        let isf = ClipSource::syphon_from_label("ISFEditor - ISF Test App");
        assert_eq!(isf, ClipSource::Syphon { app: "ISFEditor".into(), server: "ISF Test App".into() });
        assert_eq!(isf.live_name().unwrap(), "ISFEditor - ISF Test App");
        assert_eq!(ClipSource::syphon_from_label("Synesthesia").live_name().unwrap(), "Synesthesia");
    }
}
