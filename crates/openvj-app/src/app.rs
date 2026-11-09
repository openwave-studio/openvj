//! Application state and the actions behind every UI callback.
//!
//! Everything runs on the UI thread: Slint callbacks mutate [`App`], and a 60 Hz timer
//! calls [`App::frame`] to advance playback, feed video frames and render on the GPU.
//! Decoding and media loading happen on background threads.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Instant;

use openvj_audio::{Analyzer, AudioInput};
use openvj_core::{
    BAND_COUNT, BeatClock, BlendMode, ClipId, ClipSource, Composition, CrossfadeCurve, CrossfadeSide,
    DEFAULT_CLIP_SECS, Effect, GeneratorKind, MOD_RATES, MidiEffect, MidiMessage, ModInputs, ModSource, Modulation,
    Param, PlayMode, Playing, SYNC_BEATS, TapTempo, apply_midi, library, move_effect,
};
use openvj_media::dxv::{self, DxvDecoder, DxvFormat, DxvFrame};
use openvj_media::hap::{self, HapDecoder, HapFormat, HapFrame};
use openvj_media::live::{self, CameraDevice, LiveSource, LiveStatus, LiveStream, SyphonServer};
use openvj_media::{LoadedMedia, VideoDecoder, VideoInfo};
use openvj_render::{BcFrame, BcLayout, Engine, THUMB_SIZE};
use slint::{ComponentHandle, Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};

use crate::browser::{self, Entry};
use crate::link::Link;
use crate::midi::{self, MidiPorts};
use crate::syphon_out::{self, SyphonOutput};
use crate::{
    AudioLevels, BrowserItem, CellData, ColumnData, DeckData, EffectData, LayerData, MainWindow, MappingWindow,
    ModNames, OutputWindow, ParamData,
};

// The output mapping window's side of the app.
#[path = "mapping_ui.rs"]
mod mapping_ui;

const _: () = assert!(BAND_COUNT == openvj_audio::BAND_COUNT, "core and audio band counts differ");

/// Where a parameter lives; must match `Scope` in state.slint. Parameters are addressed
/// as (scope, owner, index): the owner is an effect index for the fx scopes and a layer
/// index (or -1 for the selected one) for the layer scope.
mod scope {
    pub const COMP: i32 = 0;
    pub const COMP_FX: i32 = 1;
    pub const LAYER: i32 = 2;
    pub const LAYER_FX: i32 = 3;
    pub const CLIP: i32 = 4;
    pub const GROUP: i32 = 5;
    pub const GROUP_FX: i32 = 6;
}

/// Parameter index of an effect's dry/wet mix (its channels come first).
const MIX_INDEX: usize = 1000;
/// Clip scope rows of an ISF generator's channels start here.
const ISF_PARAM_BASE: usize = 2000;
/// Index of the speed row in the clip scope (after the generator parameters).
const SPEED_INDEX: usize = openvj_core::PARAM_COUNT;
/// Clip scope rows after speed: beats per pass when beat-synced, and still/generator length.
const BEATS_INDEX: usize = SPEED_INDEX + 1;
const DURATION_INDEX: usize = SPEED_INDEX + 2;
/// Index of the crossfader row in the composition scope (after master, gain and release).
const CROSSFADER_INDEX: usize = 3;
const MAX_DURATION: f32 = 60.0;
const MAX_SPEED: f32 = 4.0;
const MAX_TRANSITION: f32 = 4.0;
const MAX_GAIN: f32 = 2.0;
const MAX_RELEASE: f32 = 1.0;
const RATE_LABELS: [&str; MOD_RATES.len()] = ["¼", "½", "1", "2", "4", "8", "16"];
/// Seconds a live input stays open after its last clip stops, so relaunching is instant.
const LIVE_LINGER: f64 = 3.0;
/// Seconds before reconnecting an input that failed (unplugged camera, app not running).
const LIVE_RETRY: f64 = 3.0;
/// Seconds between checks for Syphon servers starting and stopping.
const SYPHON_POLL: f64 = 2.0;

/// Load state of an image or video clip.
enum Media {
    Loading,
    Ready { video: Option<VideoInfo>, detail: String },
    Failed(String),
}

/// What the user picked in the browser and will place with the next slot click.
#[derive(Clone, PartialEq)]
enum Armed {
    Generator(usize),
    /// An ISF generator, by id.
    Isf(String),
    File(PathBuf),
    Input(ClipSource),
}

/// An open camera or Syphon stream, shared by every clip that uses it.
struct LiveInput {
    stream: LiveStream,
    /// When the last clip using it stopped playing and was deselected.
    idle_since: Option<f64>,
}

fn live_source(source: &ClipSource) -> Option<LiveSource> {
    match source {
        ClipSource::Camera { device, device_id } => {
            Some(LiveSource::Camera { name: device.clone(), id: device_id.clone() })
        }
        ClipSource::Syphon { app, server } => Some(LiveSource::Syphon { app: app.clone(), server: server.clone() }),
        _ => None,
    }
}

type LoadResult = (ClipId, Result<LoadedMedia, String>);

/// A running video decoder.
enum Player {
    /// An FFmpeg process piping RGBA frames.
    Ffmpeg(VideoDecoder),
    /// DXV frames read straight from the file and uploaded block-compressed.
    Dxv(DxvDecoder),
    /// HAP frames read straight from the file and uploaded block-compressed.
    Hap(HapDecoder),
}

fn bc_frame(f: &DxvFrame) -> BcFrame<'_> {
    BcFrame {
        layout: match f.format {
            DxvFormat::Dxt1 => BcLayout::Bc1Opaque,
            DxvFormat::Dxt5 => BcLayout::Bc3Premultiplied,
            DxvFormat::YCoCg => BcLayout::YCoCg,
            DxvFormat::YCoCgAlpha => BcLayout::YCoCgAlpha,
        },
        width: f.width,
        height: f.height,
        coded_width: f.coded_width,
        coded_height: f.coded_height,
        data: &f.texture,
        chroma: &f.chroma,
        chroma_height: f.chroma_height,
    }
}

fn hap_bc_frame(f: &HapFrame) -> BcFrame<'_> {
    BcFrame {
        layout: match f.format {
            HapFormat::Hap => BcLayout::Bc1Opaque,
            HapFormat::HapAlpha => BcLayout::Bc3,
            HapFormat::HapQ => BcLayout::ScaledYCoCg,
            HapFormat::HapQAlpha => BcLayout::ScaledYCoCgAlpha,
            HapFormat::HapR => BcLayout::Bc7,
        },
        width: f.width,
        height: f.height,
        coded_width: f.coded_width,
        coded_height: f.coded_height,
        data: &f.texture,
        chroma: &f.alpha,
        chroma_height: if f.alpha.is_empty() { 0 } else { f.coded_height },
    }
}

pub struct App {
    pub comp: Composition,
    engine: Engine,
    syphon_out: SyphonOutput,
    clock: Instant,
    tap: TapTempo,
    beat_clock: BeatClock,
    link: Link,
    /// Beat and audio levels for this frame.
    inputs: ModInputs,
    last_frame: f64,

    analyzer: Analyzer,
    audio: Option<AudioInput>,
    /// "Default input" followed by the device names, once enumerated.
    audio_devices: Vec<String>,
    audio_device: usize,
    audio_status: String,

    midi: Option<MidiPorts>,
    /// Port names when the ports were last opened, to notice controllers being plugged in.
    midi_port_list: Vec<String>,
    midi_scan_at: f64,
    midi_status: String,
    /// The last message received, for checking a controller is talking to openVJ.
    midi_last: String,
    midi_dirty: bool,
    /// Where things in the last imported `.avc` ended up, for resolving MIDI presets.
    /// Cleared when another composition is opened.
    resolume_refs: Option<openvj_resolume::ResolumeRefs>,

    /// Selected slot (layer, column). Drives the preview monitor and clip properties.
    selected: Option<(usize, usize)>,
    selected_layer: usize,
    armed: Option<Armed>,
    last_column: Option<usize>,
    file: Option<PathBuf>,
    status: String,
    escape_at: Option<f64>,
    pub output_open: bool,
    /// Properties tab to switch to on the next sync.
    tab_request: Option<i32>,

    browser_dir: PathBuf,
    browser_entries: Vec<Entry>,

    thumbs: HashMap<ClipId, Image>,
    media: HashMap<ClipId, Media>,
    /// Running decoders, keyed by clip, with the launch time they were opened for.
    decoders: HashMap<ClipId, (f64, Player)>,
    live: HashMap<LiveSource, LiveInput>,
    /// Inputs that failed, and when to try them again.
    live_retry: HashMap<LiveSource, f64>,
    cameras: Vec<CameraDevice>,
    syphon_servers: Vec<SyphonServer>,
    syphon_polled: f64,
    load_tx: Sender<LoadResult>,
    load_rx: Receiver<LoadResult>,

    // UI models, kept so rows can be updated in place (replacing them would reset
    // widgets mid-drag).
    layers_model: Rc<VecModel<LayerData>>,
    cell_models: Vec<Rc<VecModel<CellData>>>,
    columns_model: Rc<VecModel<ColumnData>>,
    decks_model: Rc<VecModel<DeckData>>,
    generators_model: Rc<VecModel<BrowserItem>>,
    inputs_model: Rc<VecModel<BrowserItem>>,
    files_model: Rc<VecModel<BrowserItem>>,
    bands_model: Rc<VecModel<f32>>,
    comp_params: Rc<VecModel<ParamData>>,
    audio_params: Rc<VecModel<ParamData>>,
    layer_params: Rc<VecModel<ParamData>>,
    clip_params: Rc<VecModel<ParamData>>,
    group_params: Rc<VecModel<ParamData>>,
    comp_fx: EffectModels,
    layer_fx: EffectModels,
    group_fx: EffectModels,

    grid_dirty: bool,
    props_dirty: bool,
    /// [`EffectLibrary::generation`] the Effects tab shows.
    effects_generation: Option<u64>,
    /// Ids of the ISF generators in the Sources tab, after the built-ins, in row order.
    isf_generators: Vec<String>,
    browser_dirty: bool,
    images_dirty: bool,
    /// The output window's picture must be fetched again after the next mapped frame.
    mapping_dirty: bool,
    /// The output mapping (screens, slices, masks) the output window shows.
    mapping: crate::mapping::Mapping,
    mapping_ui: slint::Weak<MappingWindow>,
    pub mapping_window_open: bool,
    /// The mapping window needs redrawing (outlines, handles, corner table).
    mapping_view_dirty: bool,

    frames: u32,
    fps_since: Instant,
}

impl App {
    pub fn new(engine: Engine, comp: Composition) -> Self {
        let (load_tx, load_rx) = mpsc::channel();
        let browser_dir = browser::default_dir();
        let syphon_out = SyphonOutput::new(engine.device(), engine.queue());
        let mut app = Self {
            comp: Composition::default(),
            engine,
            syphon_out,
            clock: Instant::now(),
            tap: TapTempo::default(),
            beat_clock: BeatClock::new(comp.bpm),
            link: Link::new(comp.bpm),
            inputs: ModInputs::default(),
            last_frame: 0.0,
            analyzer: Analyzer::new(48_000),
            audio: None,
            audio_devices: vec!["Default input".into()],
            audio_device: 0,
            audio_status: "Off. Turn it on to drive mapped parameters from the FFT bands.".into(),
            midi: None,
            midi_port_list: Vec::new(),
            midi_scan_at: 0.0,
            midi_status: "Off".into(),
            midi_last: String::new(),
            midi_dirty: true,
            resolume_refs: None,
            selected: None,
            selected_layer: 0,
            armed: None,
            last_column: None,
            file: None,
            status: String::new(),
            escape_at: None,
            output_open: false,
            mapping_dirty: true,
            mapping: crate::mapping::Mapping::load(),
            mapping_ui: slint::Weak::default(),
            mapping_window_open: false,
            mapping_view_dirty: true,
            tab_request: None,
            browser_entries: browser::list(&browser_dir),
            browser_dir,
            thumbs: HashMap::new(),
            media: HashMap::new(),
            decoders: HashMap::new(),
            live: HashMap::new(),
            live_retry: HashMap::new(),
            cameras: live::list_cameras(),
            syphon_servers: live::list_syphon_servers(),
            syphon_polled: 0.0,
            load_tx,
            load_rx,
            layers_model: Rc::new(VecModel::default()),
            cell_models: Vec::new(),
            columns_model: Rc::new(VecModel::default()),
            decks_model: Rc::new(VecModel::default()),
            generators_model: Rc::new(VecModel::default()),
            inputs_model: Rc::new(VecModel::default()),
            files_model: Rc::new(VecModel::default()),
            bands_model: Rc::new(VecModel::from(vec![0.0; BAND_COUNT])),
            comp_params: Rc::new(VecModel::default()),
            audio_params: Rc::new(VecModel::default()),
            layer_params: Rc::new(VecModel::default()),
            clip_params: Rc::new(VecModel::default()),
            group_params: Rc::new(VecModel::default()),
            comp_fx: EffectModels::default(),
            layer_fx: EffectModels::default(),
            group_fx: EffectModels::default(),
            grid_dirty: true,
            props_dirty: true,
            effects_generation: None,
            isf_generators: Vec::new(),
            browser_dirty: true,
            images_dirty: true,
            frames: 0,
            fps_since: Instant::now(),
        };
        app.set_composition(comp, None);
        app.open_midi();
        app
    }

    /// Binds the long-lived models to the window. Call once.
    pub fn attach(&self, ui: &MainWindow) {
        ui.set_layers(ModelRc::from(self.layers_model.clone()));
        ui.set_columns(ModelRc::from(self.columns_model.clone()));
        ui.set_decks(ModelRc::from(self.decks_model.clone()));
        ui.set_generators(ModelRc::from(self.generators_model.clone()));
        ui.set_inputs(ModelRc::from(self.inputs_model.clone()));
        ui.set_files(ModelRc::from(self.files_model.clone()));
        let modes: Vec<SharedString> = BlendMode::ALL.iter().map(|m| m.name().into()).collect();
        ui.set_blend_modes(ModelRc::new(VecModel::from(modes)));
        ui.set_comp_params(ModelRc::from(self.comp_params.clone()));
        ui.set_audio_params(ModelRc::from(self.audio_params.clone()));
        ui.set_layer_params(ModelRc::from(self.layer_params.clone()));
        ui.set_clip_params(ModelRc::from(self.clip_params.clone()));
        ui.set_comp_effects(ModelRc::from(self.comp_fx.effects.clone()));
        ui.set_layer_effects(ModelRc::from(self.layer_fx.effects.clone()));
        ui.set_group_params(ModelRc::from(self.group_params.clone()));
        ui.set_group_effects(ModelRc::from(self.group_fx.effects.clone()));
        let names =
            |v: Vec<&str>| ModelRc::new(VecModel::from(v.into_iter().map(SharedString::from).collect::<Vec<_>>()));
        ui.set_play_modes(names(PlayMode::ALL.iter().map(|m| m.name()).collect()));
        ui.set_crossfade_curves(names(CrossfadeCurve::ALL.iter().map(|c| c.name()).collect()));
        ui.set_link_available(Link::AVAILABLE);
        ui.set_syphon_available(syphon_out::AVAILABLE);

        let names = ui.global::<ModNames>();
        let sources: Vec<SharedString> = ModSource::ALL.iter().map(|s| s.name().into()).collect();
        names.set_sources(ModelRc::new(VecModel::from(sources)));
        let rates: Vec<SharedString> = RATE_LABELS.iter().map(|r| (*r).into()).collect();
        names.set_rates(ModelRc::new(VecModel::from(rates)));
        let levels = ui.global::<AudioLevels>();
        levels.set_bands(ModelRc::from(self.bands_model.clone()));
        let band_names: Vec<SharedString> = openvj_audio::BAND_NAMES.iter().map(|n| (*n).into()).collect();
        levels.set_band_names(ModelRc::new(VecModel::from(band_names)));
    }

    pub fn now(&self) -> f64 {
        self.clock.elapsed().as_secs_f64()
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
        self.props_dirty = true;
    }

    // ---------------------------------------------------------------- composition

    pub fn set_composition(&mut self, comp: Composition, file: Option<PathBuf>) {
        self.decoders.clear();
        self.live_retry.clear();
        self.thumbs.clear();
        self.media.clear();
        self.comp = comp;
        self.file = file;
        self.resolume_refs = None;
        self.midi_dirty = true;
        self.selected = None;
        self.selected_layer = 0;
        self.last_column = None;
        self.engine.resize(self.comp.width, self.comp.height);
        self.engine.retain_clips(&HashSet::new());
        let media: Vec<_> = self
            .comp
            .every_clip()
            .filter_map(|c| {
                Some((c.id, c.source.media_path()?.to_owned(), matches!(c.source, ClipSource::Video { .. })))
            })
            .collect();
        for (id, path, is_video) in media {
            self.start_load(id, path, is_video);
        }
        self.grid_dirty = true;
        self.props_dirty = true;
        self.images_dirty = true;
    }

    /// Starts an empty composition. MIDI mappings carry over, since they belong to the
    /// controller more than to the set.
    pub fn new_composition(&mut self) {
        let midi = std::mem::take(&mut self.comp.midi);
        let mut comp = Composition::default();
        comp.midi = midi;
        self.set_composition(comp, None);
        self.set_status("New composition");
    }

    pub fn save(&mut self, path: &Path) {
        let result = self
            .comp
            .to_json()
            .map_err(|e| e.to_string())
            .and_then(|json| std::fs::write(path, json).map_err(|e| e.to_string()));
        match result {
            Ok(()) => {
                self.file = Some(path.to_owned());
                self.set_status(format!("Saved {}", path.display()));
            }
            Err(e) => self.set_status(format!("Could not save: {e}")),
        }
    }

    pub fn file(&self) -> Option<PathBuf> {
        self.file.clone()
    }

    pub fn open(&mut self, path: &Path) {
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("avc")) {
            return self.import_resolume(path);
        }
        let comp = std::fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|s| Composition::from_json(&s));
        match comp {
            Ok(comp) => {
                self.set_composition(comp, Some(path.to_owned()));
                self.set_status(format!("Opened {}", path.display()));
            }
            Err(e) => self.set_status(format!("Could not open {}: {e}", path.display())),
        }
    }

    /// Opens a Resolume `.avc` as a new, unsaved composition, so saving never overwrites the
    /// Resolume file.
    fn import_resolume(&mut self, path: &Path) {
        match openvj_resolume::import_file(path) {
            Ok(imported) => {
                let report = imported.report;
                for note in &report.notes {
                    eprintln!("resolume import: {note}");
                }
                let mut comp = imported.composition;
                let midi_msg = load_midi_preset_for(&mut comp, path, &imported.refs);
                self.set_composition(comp, None);
                self.resolume_refs = Some(imported.refs);
                let missing = self.missing_media();
                let mut msg = format!("Imported {} from Resolume: {}", path.display(), report.summary());
                if let Some(m) = midi_msg {
                    msg += &format!(". {m}");
                }
                if missing > 0 {
                    msg +=
                        &format!(". {missing} media file{} not found, use Find media to locate them", plural(missing));
                }
                self.set_status(msg);
            }
            Err(e) => self.set_status(format!("Could not import {}: {e}", path.display())),
        }
    }

    /// Clips whose media file doesn't exist on this machine.
    pub fn missing_media(&self) -> usize {
        self.comp.every_clip().filter_map(|c| c.source.media_path()).filter(|p| !p.exists()).count()
    }

    /// Points clips with missing media at files with the same name somewhere under `dir`.
    pub fn relink_media(&mut self, dir: &Path) {
        let before: HashMap<ClipId, PathBuf> =
            self.comp.every_clip().filter_map(|c| Some((c.id, c.source.media_path()?.to_owned()))).collect();
        let result = openvj_resolume::relink_missing(&mut self.comp, &[dir.to_owned()]);
        let changed: Vec<_> = self
            .comp
            .every_clip()
            .filter_map(|c| {
                let path = c.source.media_path()?;
                (before.get(&c.id).map(PathBuf::as_path) != Some(path))
                    .then(|| (c.id, path.to_owned(), matches!(c.source, ClipSource::Video { .. })))
            })
            .collect();
        for (id, path, is_video) in changed {
            self.decoders.remove(&id);
            self.start_load(id, path, is_video);
        }
        let still = result.missing.len();
        self.set_status(format!(
            "Found {} media file{} in {}{}",
            result.relinked,
            plural(result.relinked),
            dir.display(),
            if still > 0 { format!(", {still} still missing") } else { String::new() }
        ));
        self.grid_dirty = true;
    }

    /// Drops thumbnails, textures and decoders of clips that were removed.
    fn collect_garbage(&mut self) {
        // Clips in closed decks stay loaded so switching back is instant.
        let alive: HashSet<ClipId> = self.comp.every_clip().chain(self.comp.all_clips()).map(|c| c.id).collect();
        self.thumbs.retain(|id, _| alive.contains(id));
        self.media.retain(|id, _| alive.contains(id));
        self.decoders.retain(|id, _| alive.contains(id));
        self.engine.retain_clips(&alive);
    }

    // ---------------------------------------------------------------- clips & media

    fn place(&mut self, layer: usize, col: usize, source: ClipSource) {
        let media = source.media_path().map(|p| (p.to_owned(), matches!(source, ClipSource::Video { .. })));
        let Some(id) = self.comp.set_clip(layer, col, source) else { return };
        if let Some((path, is_video)) = media {
            self.start_load(id, path, is_video);
        }
        self.collect_garbage();
        self.selected = Some((layer, col));
        self.selected_layer = layer;
        self.grid_dirty = true;
        self.props_dirty = true;
    }

    fn start_load(&mut self, id: ClipId, path: PathBuf, is_video: bool) {
        self.media.insert(id, Media::Loading);
        let tx = self.load_tx.clone();
        let max = (self.comp.width, self.comp.height);
        std::thread::spawn(move || {
            let result = openvj_media::load_media(&path, is_video, max, THUMB_SIZE).map_err(|e| format!("{e:#}"));
            let _ = tx.send((id, result));
        });
    }

    fn poll_loads(&mut self) {
        while let Ok((id, result)) = self.load_rx.try_recv() {
            if !self.media.contains_key(&id) {
                continue; // Clip was removed while loading.
            }
            match result {
                Ok(m) => {
                    self.engine.set_media_frame(id, m.frame.width, m.frame.height, &m.frame.rgba);
                    // Playback modes need the video's length; the file knows it best.
                    if let Some(secs) = m.video.and_then(|v| v.duration).filter(|d| *d > 0.0) {
                        for clip in self.comp.every_clip_mut().filter(|c| c.id == id) {
                            clip.media_secs = Some(secs);
                        }
                    }
                    let thumb =
                        SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&m.thumb.rgba, m.thumb.width, m.thumb.height);
                    self.thumbs.insert(id, Image::from_rgba8(thumb));
                    let detail = match m.video {
                        Some(v) => {
                            let dur = v.duration.map(|d| format!(" · {d:.1} s")).unwrap_or_default();
                            let codec = m.codec.map(|c| format!(" · {c}")).unwrap_or_default();
                            format!("{}×{} · {:.2} fps{dur}{codec}", v.width, v.height, v.fps)
                        }
                        None => format!("{}×{} image", m.frame.width, m.frame.height),
                    };
                    self.media.insert(id, Media::Ready { video: m.video, detail });
                }
                Err(e) => {
                    self.set_status(format!("Could not load media: {e}"));
                    self.media.insert(id, Media::Failed(e));
                }
            }
            self.grid_dirty = true;
            self.props_dirty = true;
        }
    }

    /// Keeps one decoder per live video clip and uploads frames as clip time advances.
    fn update_videos(&mut self, now: f64, beat: f64) {
        let mut live: HashMap<ClipId, (Playing, usize)> = HashMap::new();
        for (li, layer) in self.comp.layers.iter().enumerate() {
            for p in layer.active.into_iter().chain(layer.fade.and_then(|f| f.from)) {
                live.insert(p.clip_id, (p, li));
            }
        }
        self.decoders.retain(|id, (started, _)| live.get(id).map(|l| l.0.started_at) == Some(*started));

        for (id, (playing, li)) in live {
            let started = playing.started_at;
            // Look up through the layer so clips held over from another deck keep playing.
            let Some(clip) = self.comp.layers[li].clip_by_id(id) else { continue };
            let ClipSource::Video { path } = &clip.source else { continue };
            let Some(Media::Ready { video: Some(info), .. }) = self.media.get(&id) else { continue };
            let time = playing.time(clip, now, beat).secs;
            if !self.decoders.contains_key(&id) {
                match self.open_player(path, *info) {
                    Ok(player) => {
                        self.decoders.insert(id, (started, player));
                    }
                    Err(e) => {
                        let msg = format!("{e:#}");
                        self.set_status(format!("Could not play video: {msg}"));
                        self.media.insert(id, Media::Failed(msg));
                        self.grid_dirty = true;
                        continue;
                    }
                }
            }
            match &mut self.decoders.get_mut(&id).expect("decoder just ensured").1 {
                Player::Ffmpeg(dec) => {
                    let (w, h) = (dec.width, dec.height);
                    if let Some(frame) = dec.frame_at(time) {
                        self.engine.set_media_frame(id, w, h, frame);
                    }
                }
                Player::Dxv(dec) => {
                    if let Some(frame) = dec.frame_at(time) {
                        self.engine.set_media_bc(id, &bc_frame(&frame));
                    }
                }
                Player::Hap(dec) => {
                    if let Some(frame) = dec.frame_at(time) {
                        self.engine.set_media_bc(id, &hap_bc_frame(&frame));
                    }
                }
            }
        }
    }

    /// Opens camera and Syphon inputs while a clip using them plays or is selected, and
    /// uploads their newest frames to every clip that shows them.
    fn update_live(&mut self, now: f64) {
        let mut shown: HashSet<ClipId> = self.selected_clip_id().into_iter().collect();
        for layer in &self.comp.layers {
            shown.extend(layer.active.into_iter().chain(layer.fade.and_then(|f| f.from)).map(|p| p.clip_id));
        }
        let mut clips: HashMap<LiveSource, Vec<ClipId>> = HashMap::new();
        let mut wanted: HashSet<LiveSource> = HashSet::new();
        for clip in self.comp.all_clips() {
            let Some(source) = live_source(&clip.source) else { continue };
            if shown.contains(&clip.id) {
                wanted.insert(source.clone());
            }
            clips.entry(source).or_default().push(clip.id);
        }

        self.live.retain(|source, input| {
            if wanted.contains(source) {
                input.idle_since = None;
                return true;
            }
            now - *input.idle_since.get_or_insert(now) < LIVE_LINGER
        });
        let (w, h) = (self.comp.width, self.comp.height);
        for source in wanted {
            if !self.live.contains_key(&source) && self.live_retry.get(&source).is_none_or(|t| now >= *t) {
                self.live_retry.remove(&source);
                self.live
                    .insert(source.clone(), LiveInput { stream: LiveStream::open(&source, w, h), idle_since: None });
            }
        }

        let mut failed = Vec::new();
        for (source, input) in &self.live {
            let ids = clips.get(source).map(Vec::as_slice).unwrap_or_default();
            let status = input.stream.status();
            for id in ids {
                let changed = match (&status, self.media.get(id)) {
                    (LiveStatus::Connecting, Some(Media::Loading | Media::Ready { .. })) => false,
                    (LiveStatus::Running { detail }, Some(Media::Ready { detail: d, .. })) => d != detail,
                    (LiveStatus::Failed(e), Some(Media::Failed(f))) => e != f,
                    _ => true,
                };
                if !changed {
                    continue;
                }
                let media = match &status {
                    LiveStatus::Connecting => Media::Loading,
                    LiveStatus::Running { detail } => Media::Ready { video: None, detail: detail.clone() },
                    LiveStatus::Failed(e) => Media::Failed(e.clone()),
                };
                self.media.insert(*id, media);
                self.grid_dirty = true;
                self.props_dirty = true;
            }
            if let LiveStatus::Failed(e) = &status {
                if !self.status.ends_with(e.as_str()) {
                    self.status = format!("Live input: {e}");
                    self.props_dirty = true;
                }
                failed.push(source.clone());
            }
            if let Some(frame) = input.stream.take_frame() {
                for id in ids {
                    self.engine.set_media_frame(*id, frame.width, frame.height, &frame.rgba);
                }
            }
        }
        for source in failed {
            self.live.remove(&source);
            self.live_retry.insert(source, now + LIVE_RETRY);
        }

        if now - self.syphon_polled >= SYPHON_POLL {
            self.syphon_polled = now;
            let servers = live::list_syphon_servers();
            if servers != self.syphon_servers {
                self.syphon_servers = servers;
                self.browser_dirty = true;
            }
        }
    }

    fn selected_clip_id(&self) -> Option<ClipId> {
        self.selected.and_then(|(l, c)| self.comp.clip(l, c)).map(|c| c.id)
    }

    /// DXV and HAP play on the GPU directly when it can sample BC textures; everything else,
    /// and any DXV or HAP file our decoders reject, goes through FFmpeg.
    fn open_player(&self, path: &Path, info: VideoInfo) -> anyhow::Result<Player> {
        if self.engine.supports_bc() {
            match DxvDecoder::open(path) {
                Ok(dec) => return Ok(Player::Dxv(dec)),
                Err(e) if dxv::is_dxv(path) => {
                    eprintln!("openVJ: playing {} through FFmpeg: {e:#}", path.display());
                }
                Err(_) => {}
            }
            match HapDecoder::open(path) {
                Ok(dec) => return Ok(Player::Hap(dec)),
                Err(e) if hap::is_hap(path) => {
                    eprintln!("openVJ: playing {} through FFmpeg: {e:#}", path.display());
                }
                Err(_) => {}
            }
        }
        let dec = VideoDecoder::open_with_info(path, info, self.comp.width, self.comp.height)?;
        Ok(Player::Ffmpeg(dec))
    }

    // ---------------------------------------------------------------- frame loop

    pub fn frame(&mut self, ui: &MainWindow, out: &OutputWindow) {
        let now = self.now();
        let dt = (now - self.last_frame).clamp(0.0, 0.25) as f32;
        self.last_frame = now;
        let beat = self.update_tempo(now);
        let live = self.comp.live_clip_ids();
        self.comp.tick(now, beat);
        if self.comp.live_clip_ids() != live {
            self.grid_dirty = true; // A play-once clip ended.
        }
        self.poll_loads();
        self.update_videos(now, beat);
        self.update_live(now);
        self.update_midi(now);
        self.inputs = ModInputs { beat, bands: self.update_audio(dt) };
        let preview = self.selected.and_then(|(l, c)| self.comp.clip(l, c)).map(|c| c.id);
        self.syphon_out.before_render(&mut self.engine);
        self.engine.render(&self.comp, now, &self.inputs, preview);
        if (self.output_open || self.mapping_window_open)
            && let Some(setup) = &self.mapping.setup
        {
            self.engine.map_output(setup, self.mapping.test_card);
            if std::mem::take(&mut self.mapping_dirty) {
                self.images_dirty = true;
            }
        }
        self.syphon_out.after_render(&self.engine);
        ui.set_syphon_on(self.syphon_out.is_enabled());
        ui.set_syphon_receiving(self.syphon_out.is_receiving());
        ui.set_syphon_error(self.syphon_out.error().unwrap_or_default().into());
        // Event inputs (such as Acuarela's Reset) fire for one frame.
        for fx in self
            .comp
            .effects
            .iter_mut()
            .chain(self.comp.layers.iter_mut().flat_map(|l| l.effects.iter_mut()))
            .chain(self.comp.groups.iter_mut().flat_map(|g| g.effects.iter_mut()))
        {
            fx.clear_events();
        }

        self.update_effect_library(ui);
        if self.escape_at.is_some_and(|t| now - t > 0.8) {
            self.escape_at = None;
            self.props_dirty = true;
        }
        ui.set_beat(self.inputs.beat.rem_euclid(1024.0) as f32);
        ui.set_link_on(self.link.is_enabled());
        ui.set_link_peers(self.link.peers() as i32);
        self.frames += 1;
        let elapsed = self.fps_since.elapsed().as_secs_f64();
        if elapsed >= 1.0 {
            ui.set_fps(format!("{:.0} fps", self.frames as f64 / elapsed).into());
            self.frames = 0;
            self.fps_since = Instant::now();
        }
        self.sync(ui, out);
        ui.window().request_redraw();
        if self.output_open {
            out.window().request_redraw();
        }
    }

    /// Pushes whatever changed since the last call to the UI.
    pub fn sync(&mut self, ui: &MainWindow, out: &OutputWindow) {
        if self.mapping_window_open && (self.images_dirty || self.mapping_view_dirty) {
            self.sync_mapping_window(self.images_dirty);
        }
        if self.images_dirty {
            self.images_dirty = false;
            let output = Image::try_from(self.engine.output().texture.clone()).expect("output texture is importable");
            let preview =
                Image::try_from(self.engine.preview().texture.clone()).expect("preview texture is importable");
            let mapped = self
                .mapping
                .setup
                .as_ref()
                .and(self.engine.screen(0))
                .map(|t| Image::try_from(t.texture.clone()).expect("screen texture is importable"));
            out.set_frame(mapped.unwrap_or_else(|| output.clone()));
            ui.set_output_image(output);
            ui.set_preview_image(preview);
            ui.set_comp_info(format!("{}×{}", self.comp.width, self.comp.height).into());
        }
        if self.grid_dirty {
            self.grid_dirty = false;
            self.sync_grid(ui);
        } else {
            self.sync_live_opacity();
        }
        if self.props_dirty {
            self.props_dirty = false;
            self.sync_props(ui);
        }
        if self.midi_dirty {
            self.midi_dirty = false;
            self.sync_midi(ui);
        }
        if self.browser_dirty {
            self.browser_dirty = false;
            self.sync_browser(ui);
        }
        self.sync_params(ui);
        if let Some(tab) = self.tab_request.take() {
            ui.set_props_tab(tab);
        }
    }

    /// Beat position for this frame, from Ableton Link when it's on.
    fn update_tempo(&mut self, now: f64) -> f64 {
        if self.link.is_enabled() {
            let (beat, bpm) = self.link.capture();
            let shown = (bpm * 10.0).round() / 10.0;
            if shown != self.comp.bpm {
                self.comp.bpm = shown;
                self.grid_dirty = true;
            }
            self.beat_clock.sync(now, beat, bpm);
        } else {
            self.beat_clock.set_bpm(now, self.comp.bpm);
        }
        self.beat_clock.beat_at(now)
    }

    /// Band levels for this frame (zero while the input is off).
    fn update_audio(&mut self, dt: f32) -> [f32; BAND_COUNT] {
        let Some(input) = &self.audio else { return [0.0; BAND_COUNT] };
        input.drain_into(&mut self.analyzer);
        self.analyzer.analyze(dt).bands
    }

    /// Parameter rows change every frame while modulated, so they are rebuilt each sync
    /// (only rows that changed are pushed to the UI).
    fn sync_params(&mut self, ui: &MainWindow) {
        let inputs = self.inputs;
        let levels = ui.global::<AudioLevels>();
        for (i, v) in inputs.bands.iter().enumerate() {
            if self.bands_model.row_data(i) != Some(*v) {
                self.bands_model.set_row_data(i, *v);
            }
        }
        levels.set_level(self.analyzer.levels().level);

        let comp = &self.comp;
        let xf = comp.crossfader;
        update_model(
            &self.comp_params,
            vec![
                param_row(0, "Master", &Param { value: comp.master, modulation: comp.master_mod }, 1.0, &inputs, pct),
                param_row(
                    CROSSFADER_INDEX,
                    "Crossfader",
                    &Param { value: xf.position, modulation: xf.position_mod },
                    0.5,
                    &inputs,
                    crossfade_label,
                ),
            ],
        );
        ui.set_live_master(comp.master_value(&inputs));
        ui.set_crossfader(xf.position);
        ui.set_live_crossfader(xf.position_value(&inputs));
        let gain = self.analyzer.gain;
        let release = self.analyzer.release;
        update_model(
            &self.audio_params,
            vec![
                fixed_row(1, "Gain", gain / MAX_GAIN, 1.0 / MAX_GAIN, format!("{gain:.2}×")),
                fixed_row(
                    2,
                    "Release",
                    release / MAX_RELEASE,
                    openvj_audio::DEFAULT_RELEASE / MAX_RELEASE,
                    format!("{:.0} ms", release * 1000.0),
                ),
            ],
        );
        self.comp_fx.update(&comp.effects, &inputs);

        match comp.layers.get(self.selected_layer) {
            Some(layer) => {
                let opacity = Param { value: layer.opacity, modulation: layer.opacity_mod };
                let t = layer.transition_secs;
                update_model(
                    &self.layer_params,
                    vec![
                        param_row(0, "Opacity", &opacity, 1.0, &inputs, pct),
                        fixed_row(1, "Transition", t / MAX_TRANSITION, 0.0, format!("{t:.1} s")),
                    ],
                );
                self.layer_fx.update(&layer.effects, &inputs);
            }
            None => {
                update_model(&self.layer_params, Vec::new());
                self.layer_fx.update(&[], &inputs);
            }
        }

        match comp.layers.get(self.selected_layer).and_then(|l| comp.group_of(l)) {
            Some(group) => {
                let opacity = Param { value: group.opacity, modulation: group.opacity_mod };
                update_model(&self.group_params, vec![param_row(0, "Opacity", &opacity, 1.0, &inputs, pct)]);
                self.group_fx.update(&group.effects, &inputs);
            }
            None => {
                update_model(&self.group_params, Vec::new());
                self.group_fx.update(&[], &inputs);
            }
        }

        let mut rows = Vec::new();
        if let Some(clip) = self.selected.and_then(|(l, c)| comp.clip(l, c)) {
            if let ClipSource::Generator { kind } = clip.source {
                for (i, spec) in kind.params().iter().enumerate() {
                    let p = Param { value: clip.params[i], modulation: clip.mods[i] };
                    rows.push(param_row(i, spec.name, &p, spec.default, &inputs, pct));
                }
            }
            if let ClipSource::Isf { generator } = &clip.source
                && let Some(ty) = generator.ty()
            {
                for (i, ch) in ty.channels.iter().enumerate() {
                    let input = ty.input(ch);
                    let p = generator.param(&ch.key).copied().unwrap_or(Param::new(ch.default));
                    let display = |v| input.display(ch.component, v);
                    rows.push(param_row(ISF_PARAM_BASE + i, &ch.label, &p, ch.default, &inputs, display));
                }
            }
            if !clip.source.is_live() {
                match clip.sync_beats {
                    Some(beats) => {
                        let i = beat_choice(beats);
                        let last = (SYNC_BEATS.len() - 1) as f32;
                        let label = if beats == 1.0 { "1 beat".to_owned() } else { format!("{beats} beats") };
                        let reset = beat_choice(4.0) as f32 / last;
                        rows.push(fixed_row(BEATS_INDEX, "Beats", i as f32 / last, reset, label));
                    }
                    None => {
                        let speed = clip.speed;
                        let label = format!("{speed:.2}×");
                        rows.push(fixed_row(SPEED_INDEX, "Speed", speed / MAX_SPEED, 1.0 / MAX_SPEED, label));
                    }
                }
            }
            if !matches!(clip.source, ClipSource::Video { .. }) && !clip.source.is_live() {
                let d = clip.duration.unwrap_or(DEFAULT_CLIP_SECS);
                let reset = DEFAULT_CLIP_SECS / MAX_DURATION;
                rows.push(fixed_row(DURATION_INDEX, "Duration", d / MAX_DURATION, reset, format!("{d:.1} s")));
            }
        }
        update_model(&self.clip_params, rows);
    }

    /// Modulated layer opacity moves every frame. Only those fields are pushed, so the
    /// grid isn't rebuilt at frame rate (that rebuild formats every cell's strings).
    fn sync_live_opacity(&self) {
        let n = self.comp.layers.len();
        if self.layers_model.row_count() != n {
            return;
        }
        for (li, layer) in self.comp.layers.iter().enumerate() {
            if !layer.opacity_mod.is_active() {
                continue;
            }
            let row = n - 1 - li;
            let live = layer.opacity_value(&self.inputs);
            if let Some(mut data) = self.layers_model.row_data(row)
                && data.live_opacity != live
            {
                data.live_opacity = live;
                self.layers_model.set_row_data(row, data);
            }
        }
    }

    fn sync_grid(&mut self, ui: &MainWindow) {
        // Generator and live input thumbnails are GPU textures; create their images once.
        let gen_ids: Vec<ClipId> =
            self.comp.all_clips().filter(|c| c.source.is_generator() || c.source.is_live()).map(|c| c.id).collect();
        for id in gen_ids {
            if !self.thumbs.contains_key(&id) {
                let texture = self.engine.thumb(id).texture.clone();
                self.thumbs.insert(id, Image::try_from(texture).expect("thumb texture is importable"));
            }
        }

        let columns: Vec<ColumnData> = self
            .comp
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| ColumnData { name: c.name.as_str().into(), active: self.last_column == Some(i) })
            .collect();
        update_model(&self.columns_model, columns);
        let decks: Vec<DeckData> = self
            .comp
            .decks
            .iter()
            .enumerate()
            .map(|(i, d)| DeckData { name: d.name.as_str().into(), active: i == self.comp.current_deck })
            .collect();
        update_model(&self.decks_model, decks);

        let n = self.comp.layers.len();
        self.cell_models.resize_with(n, || Rc::new(VecModel::default()));
        let mut rows = Vec::with_capacity(n);
        // Display the top of the mix first.
        for (row, li) in (0..n).rev().enumerate() {
            let layer = &self.comp.layers[li];
            let playing = layer.playing_column();
            let cells: Vec<CellData> = layer
                .clips
                .iter()
                .enumerate()
                .map(|(ci, clip)| {
                    let selected = self.selected == Some((li, ci));
                    let Some(clip) = clip else { return CellData { selected, ..Default::default() } };
                    let thumb = self.thumbs.get(&clip.id).cloned();
                    let media = self.media.get(&clip.id);
                    CellData {
                        name: clip.name.as_str().into(),
                        kind: clip.source.kind_label().into(),
                        has_thumb: thumb.is_some(),
                        thumb: thumb.unwrap_or_default(),
                        filled: true,
                        playing: playing == Some(ci),
                        selected,
                        loading: matches!(media, Some(Media::Loading)),
                        error: matches!(media, Some(Media::Failed(_))),
                        modulated: clip.mods.iter().any(Modulation::is_active),
                    }
                })
                .collect();
            update_model(&self.cell_models[row], cells);
            rows.push(LayerData {
                index: li as i32,
                name: layer.name.as_str().into(),
                opacity: layer.opacity,
                live_opacity: layer.opacity_value(&self.inputs),
                blend: layer.blend.shader_id() as i32,
                bypass: layer.bypass,
                solo: layer.solo,
                selected: li == self.selected_layer,
                live: layer.active.is_some(),
                fx_count: layer.effects.len() as i32,
                side: match layer.crossfade {
                    CrossfadeSide::None => SharedString::default(),
                    side => side.label().into(),
                },
                group: self.comp.group_of(layer).map(|g| g.name.as_str().into()).unwrap_or_default(),
                cells: ModelRc::from(self.cell_models[row].clone()),
            });
        }
        update_model(&self.layers_model, rows);

        if let Some(layer) = self.comp.layers.get(self.selected_layer) {
            ui.set_layer_blend_value(layer.blend.shader_id() as i32);
        }
        ui.set_master(self.comp.master);
        ui.set_bpm(self.comp.bpm);
        ui.set_comp_name(self.comp_title().into());
        ui.set_output_open(self.output_open);
        ui.set_mapping_status(self.mapping.status().into());
        ui.set_mapped(self.mapping.setup.is_some());
        ui.set_test_card(self.mapping.test_card);
        ui.set_missing_media(self.missing_media() as i32);
    }

    fn comp_title(&self) -> String {
        let file = self.file.as_ref().and_then(|f| f.file_name()).map(|f| f.to_string_lossy().into_owned());
        file.unwrap_or_else(|| self.comp.name.clone())
    }

    fn sync_props(&mut self, ui: &MainWindow) {
        let clip = self.selected.and_then(|(l, c)| self.comp.clip(l, c));
        ui.set_clip_valid(clip.is_some());
        if let Some(clip) = clip {
            let kind = clip.source.kind_label();
            let (l, c) = self.selected.unwrap();
            let kind_name = match kind {
                "gen" => "generator",
                other => other,
            };
            ui.set_clip_title(format!("{} · {kind_name} · layer {} / column {}", clip.name, l + 1, c + 1).into());
            ui.set_clip_kind(kind.into());
            ui.set_clip_name(clip.name.as_str().into());
            let detail = match (&clip.source, self.media.get(&clip.id)) {
                (ClipSource::Generator { kind }, _) => kind.description().to_owned(),
                (ClipSource::Isf { generator }, _) => match generator.ty() {
                    Some(t) => t.def.description.clone(),
                    None => format!("⚠ ISF generator \"{}\" isn't installed", generator.kind),
                },
                (_, Some(Media::Ready { detail, .. })) => detail.clone(),
                (_, Some(Media::Failed(e))) => format!("⚠ {e}"),
                (source, None) if source.is_live() => "connects when the clip plays or is selected".to_owned(),
                (source, Some(Media::Loading)) if source.is_live() => "connecting…".to_owned(),
                (_, _) => "loading…".to_owned(),
            };
            ui.set_clip_detail(detail.into());
            let path = match &clip.source {
                ClipSource::Camera { device, .. } => format!("Camera: {device}"),
                ClipSource::Syphon { .. } => format!("Syphon: {}", clip.source.live_name().unwrap_or_default()),
                source => source.media_path().map(|p| browser::short_path(p, 44)).unwrap_or_default(),
            };
            ui.set_clip_path(path.into());
            ui.set_preview_label(clip.name.as_str().into());
            ui.set_clip_play_mode(clip.play_mode.index() as i32);
            ui.set_clip_synced(clip.sync_beats.is_some());
        } else {
            ui.set_preview_label(SharedString::default());
        }

        let layer = self.comp.layers.get(self.selected_layer);
        ui.set_layer_valid(layer.is_some());
        if let Some(layer) = layer {
            ui.set_layer_title(format!("{} of {}", self.selected_layer + 1, self.comp.layers.len()).into());
            ui.set_layer_name(layer.name.as_str().into());
            ui.set_layer_blend_value(layer.blend.shader_id() as i32);
            ui.set_layer_side(match layer.crossfade {
                CrossfadeSide::None => SharedString::default(),
                side => side.label().into(),
            });
        }
        let mut choices: Vec<SharedString> = vec!["No group".into()];
        choices.extend(self.comp.groups.iter().map(|g| SharedString::from(g.name.as_str())));
        choices.push("New group".into());
        ui.set_group_choices(ModelRc::new(VecModel::from(choices)));
        let group = layer.and_then(|l| l.group);
        ui.set_layer_group_choice(group.map_or(0, |g| g as i32 + 1));
        let group = group.and_then(|g| self.comp.groups.get(g).map(|x| (g, x)));
        ui.set_group_valid(group.is_some());
        if let Some((g, group)) = group {
            ui.set_group_name(group.name.as_str().into());
            let n = self.comp.group_members(g).len();
            ui.set_group_title(format!("{n} layer{}", plural(n)).into());
            ui.set_group_blend_value(group.blend.shader_id() as i32);
            ui.set_group_bypass(group.bypass);
            ui.set_group_solo(group.solo);
        } else if ui.get_props_tab() == 3 {
            ui.set_props_tab(1);
        }
        ui.set_crossfade_curve(self.comp.crossfader.curve.index() as i32);

        ui.set_audio_on(self.audio.is_some());
        let devices: Vec<SharedString> = self.audio_devices.iter().map(|d| d.as_str().into()).collect();
        ui.set_audio_devices(ModelRc::new(VecModel::from(devices)));
        ui.set_audio_device_index(self.audio_device as i32);
        ui.set_audio_status(self.audio_status.as_str().into());

        let status = if self.escape_at.is_some() {
            "Press Esc again to clear all layers".to_owned()
        } else {
            self.status.clone()
        };
        ui.set_status(status.into());
        let armed = self.armed.as_ref().map(|a| match a {
            Armed::Generator(i) => GeneratorKind::ALL[*i].name().to_owned(),
            Armed::Isf(id) => library().generator(id).map_or_else(|| id.clone(), |t| t.name().to_owned()),
            Armed::File(p) => p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(),
            Armed::Input(source) => source.live_name().unwrap_or_default(),
        });
        ui.set_armed(armed.is_some());
        ui.set_armed_label(armed.unwrap_or_default().into());
    }

    fn sync_browser(&mut self, ui: &MainWindow) {
        let mut gens: Vec<BrowserItem> = GeneratorKind::ALL
            .iter()
            .enumerate()
            .map(|(i, k)| BrowserItem {
                name: k.name().into(),
                detail: k.description().into(),
                kind: "gen".into(),
                armed: self.armed == Some(Armed::Generator(i)),
            })
            .collect();
        // ISF generators after the built-ins, by category, then name.
        let lib = library();
        let mut isf: Vec<_> = lib.generators().iter().collect();
        let category = |t: &openvj_core::EffectType| t.def.categories.first().cloned().unwrap_or_default();
        isf.sort_by_cached_key(|t| (category(t).to_lowercase(), t.name().to_lowercase()));
        self.isf_generators = isf.iter().map(|t| t.id().to_owned()).collect();
        gens.extend(isf.iter().map(|t| BrowserItem {
            name: t.name().into(),
            detail: match category(t) {
                c if c.is_empty() => "ISF generator".into(),
                c => format!("ISF · {c}").into(),
            },
            kind: "gen".into(),
            armed: self.armed.as_ref() == Some(&Armed::Isf(t.id().to_owned())),
        }));
        update_model(&self.generators_model, gens);
        let inputs: Vec<BrowserItem> = self
            .input_sources()
            .into_iter()
            .map(|source| BrowserItem {
                name: source.live_name().unwrap_or_default().into(),
                detail: if matches!(source, ClipSource::Camera { .. }) { "camera" } else { "Syphon" }.into(),
                kind: source.kind_label().into(),
                armed: self.armed == Some(Armed::Input(source)),
            })
            .collect();
        update_model(&self.inputs_model, inputs);
        let hint = match (live::CAMERA_SUPPORTED, live::SYPHON_SUPPORTED) {
            (true, true) => {
                "No cameras or Syphon servers found. Start the app that sends Syphon (e.g. Synesthesia) or plug in a camera, then Refresh."
            }
            (true, false) => "No cameras found. Plug one in and press Refresh. (Syphon is macOS only.)",
            (false, true) => "No Syphon servers found. This build has no camera support.",
            (false, false) => "This build has no camera or Syphon support.",
        };
        ui.set_inputs_hint(hint.into());
        let files: Vec<BrowserItem> = self
            .browser_entries
            .iter()
            .map(|e| BrowserItem {
                name: e.name.as_str().into(),
                detail: e.detail.as_str().into(),
                kind: e.kind().into(),
                armed: self.armed == Some(Armed::File(e.path.clone())),
            })
            .collect();
        update_model(&self.files_model, files);
        ui.set_browser_path(browser::short_path(&self.browser_dir, 56).into());
    }

    // ---------------------------------------------------------------- grid actions

    pub fn cell_clicked(&mut self, layer: usize, col: usize) {
        if let Some(armed) = self.armed.take() {
            let source = self.armed_source(&armed);
            self.browser_dirty = true;
            if let Some(source) = source {
                self.place(layer, col, source);
            }
            return;
        }
        let now = self.now();
        if self.comp.clip(layer, col).is_some() {
            self.comp.trigger(layer, col, now);
            self.last_column = None;
        }
        self.selected = Some((layer, col));
        self.selected_layer = layer;
        self.grid_dirty = true;
        self.props_dirty = true;
    }

    pub fn cell_right_clicked(&mut self, layer: usize, col: usize) {
        self.tab_request = Some(2);
        self.selected = Some((layer, col));
        self.selected_layer = layer;
        self.grid_dirty = true;
        self.props_dirty = true;
    }

    pub fn column_clicked(&mut self, col: usize) {
        let now = self.now();
        self.comp.trigger_column(col, now);
        self.last_column = Some(col);
        self.grid_dirty = true;
    }

    pub fn clear_all(&mut self) {
        let now = self.now();
        self.comp.clear_all(now);
        self.last_column = None;
        self.grid_dirty = true;
    }

    /// `-1` means the selected layer.
    fn layer_index(&self, layer: i32) -> Option<usize> {
        let i = if layer < 0 { self.selected_layer } else { layer as usize };
        (i < self.comp.layers.len()).then_some(i)
    }

    pub fn select_layer(&mut self, layer: i32) {
        if let Some(l) = self.layer_index(layer) {
            self.selected_layer = l;
            self.tab_request = Some(1);
            self.grid_dirty = true;
            self.props_dirty = true;
        }
    }

    pub fn layer_opacity(&mut self, layer: i32, v: f32) {
        if let Some(l) = self.layer_index(layer) {
            self.comp.layers[l].opacity = v.clamp(0.0, 1.0);
            self.grid_dirty = true;
        }
    }

    pub fn layer_blend(&mut self, layer: i32, mode: i32) {
        if let Some(l) = self.layer_index(layer) {
            self.comp.layers[l].blend = BlendMode::from_index(mode.max(0) as usize);
            self.grid_dirty = true;
        }
    }

    pub fn layer_bypass(&mut self, layer: i32) {
        if let Some(l) = self.layer_index(layer) {
            self.comp.layers[l].bypass ^= true;
            self.grid_dirty = true;
        }
    }

    pub fn layer_solo(&mut self, layer: i32) {
        if let Some(l) = self.layer_index(layer) {
            self.comp.layers[l].solo ^= true;
            self.grid_dirty = true;
        }
    }

    pub fn layer_clear(&mut self, layer: i32) {
        if let Some(l) = self.layer_index(layer) {
            let now = self.now();
            self.comp.clear_layer(l, now);
            self.grid_dirty = true;
        }
    }

    pub fn layer_renamed(&mut self, name: &str) {
        if let Some(l) = self.layer_index(-1) {
            self.comp.layers[l].name = name.trim().to_owned();
            self.grid_dirty = true;
        }
    }

    pub fn add_layer(&mut self) {
        self.selected_layer = self.comp.add_layer();
        self.grid_dirty = true;
        self.props_dirty = true;
    }

    pub fn remove_layer(&mut self) {
        if self.comp.layers.len() <= 1 {
            self.set_status("A composition needs at least one layer");
            return;
        }
        self.comp.remove_layer(self.selected_layer);
        self.selected_layer = self.selected_layer.min(self.comp.layers.len() - 1);
        self.selected = None;
        self.collect_garbage();
        self.grid_dirty = true;
        self.props_dirty = true;
    }

    pub fn add_column(&mut self) {
        self.comp.add_column();
        self.grid_dirty = true;
    }

    // ---------------------------------------------------------------- decks

    /// Shows another deck. What's playing keeps playing.
    pub fn deck_clicked(&mut self, index: usize) {
        if index == self.comp.current_deck {
            return;
        }
        self.comp.switch_deck(index);
        self.after_deck_change();
    }

    pub fn deck_add(&mut self) {
        self.comp.add_deck();
        self.after_deck_change();
    }

    pub fn deck_renamed(&mut self, index: usize, name: &str) {
        self.comp.rename_deck(index, name);
        self.grid_dirty = true;
    }

    pub fn deck_remove(&mut self, index: usize) {
        self.comp.remove_deck(index);
        self.after_deck_change();
        self.collect_garbage();
    }

    fn after_deck_change(&mut self) {
        self.selected = None;
        self.last_column = None;
        self.grid_dirty = true;
        self.props_dirty = true;
        self.set_status(format!("Deck: {}", self.comp.deck_name()));
    }

    // ---------------------------------------------------------------- clip properties

    pub fn clip_renamed(&mut self, name: &str) {
        if let Some(clip) = self.selected.and_then(|(l, c)| self.comp.clip_mut(l, c)) {
            clip.name = name.trim().to_owned();
            self.grid_dirty = true;
            self.props_dirty = true;
        }
    }

    pub fn clip_restart(&mut self) {
        if let Some((l, c)) = self.selected {
            let now = self.now();
            self.comp.trigger(l, c, now);
            self.grid_dirty = true;
        }
    }

    pub fn clip_play_mode(&mut self, mode: usize) {
        if let Some(clip) = self.selected.and_then(|(l, c)| self.comp.clip_mut(l, c)) {
            clip.play_mode = PlayMode::from_index(mode);
            self.props_dirty = true;
        }
    }

    /// Switches the selected clip between its own speed and beat sync.
    pub fn clip_synced(&mut self, synced: bool) {
        if let Some(clip) = self.selected.and_then(|(l, c)| self.comp.clip_mut(l, c)) {
            clip.sync_beats = synced.then_some(clip.sync_beats.unwrap_or(4.0));
            self.props_dirty = true;
        }
    }

    pub fn layer_side(&mut self, i: usize) {
        if let Some(l) = self.layer_index(-1) {
            self.comp.layers[l].crossfade = CrossfadeSide::ALL.get(i).copied().unwrap_or_default();
            self.grid_dirty = true;
            self.props_dirty = true;
        }
    }

    /// Picks the selected layer's group: 0 none, then the groups, then a new group.
    pub fn layer_group(&mut self, choice: usize) {
        let Some(l) = self.layer_index(-1) else { return };
        let groups = self.comp.groups.len();
        let selected_clip = self.selected.and_then(|(l, c)| self.comp.clip(l, c)).map(|c| c.id);
        let name = self.comp.layers[l].name.clone();
        let msg = match choice {
            0 => {
                self.selected_layer = self.comp.set_layer_group(l, None);
                format!("{name} left its group")
            }
            g if g <= groups => {
                self.selected_layer = self.comp.set_layer_group(l, Some(g - 1));
                format!("{name} joined {}", self.comp.groups[g - 1].name)
            }
            _ => {
                self.comp.set_layer_group(l, None);
                let g = self.comp.group_layers(&[l]);
                self.tab_request = Some(3);
                format!("Made {} for {name}", g.map(|g| self.comp.groups[g].name.clone()).unwrap_or_default())
            }
        };
        // Joining a group can move the layer; keep the same clip selected.
        self.selected = selected_clip.and_then(|id| self.comp.find_clip(id));
        self.set_status(msg);
        self.grid_dirty = true;
    }

    fn selected_group_mut(&mut self) -> Option<&mut openvj_core::LayerGroup> {
        let g = self.comp.layers.get(self.selected_layer)?.group?;
        self.comp.groups.get_mut(g)
    }

    pub fn group_renamed(&mut self, name: &str) {
        if let Some(g) = self.selected_group_mut()
            && !name.trim().is_empty()
        {
            g.name = name.trim().to_owned();
        }
        self.grid_dirty = true;
        self.props_dirty = true;
    }

    pub fn group_blend(&mut self, mode: i32) {
        if let Some(g) = self.selected_group_mut() {
            g.blend = BlendMode::from_index(mode.max(0) as usize);
        }
        self.props_dirty = true;
    }

    pub fn group_bypass(&mut self) {
        if let Some(g) = self.selected_group_mut() {
            g.bypass ^= true;
        }
        self.grid_dirty = true;
        self.props_dirty = true;
    }

    pub fn group_solo(&mut self) {
        if let Some(g) = self.selected_group_mut() {
            g.solo ^= true;
        }
        self.grid_dirty = true;
        self.props_dirty = true;
    }

    /// Dissolves the selected layer's group; its layers stay where they are.
    pub fn group_ungroup(&mut self) {
        let Some(g) = self.comp.layers.get(self.selected_layer).and_then(|l| l.group) else { return };
        let name = self.comp.groups[g].name.clone();
        for l in self.comp.group_members(g) {
            self.comp.layers[l].group = None;
        }
        self.comp.set_layer_group(self.selected_layer, None); // Drops the now-empty group.
        self.set_status(format!("Ungrouped {name}"));
        self.grid_dirty = true;
    }

    pub fn crossfade_curve(&mut self, i: usize) {
        self.comp.crossfader.curve = CrossfadeCurve::from_index(i);
        self.props_dirty = true;
    }

    pub fn clip_remove(&mut self) {
        if let Some((l, c)) = self.selected
            && self.comp.remove_clip(l, c).is_some()
        {
            self.collect_garbage();
            self.grid_dirty = true;
            self.props_dirty = true;
        }
    }

    // ---------------------------------------------------------------- browser

    fn armed_source(&self, armed: &Armed) -> Option<ClipSource> {
        match armed {
            Armed::Generator(i) => GeneratorKind::ALL.get(*i).map(|k| ClipSource::Generator { kind: *k }),
            Armed::Isf(id) => Some(ClipSource::isf(id)),
            Armed::File(p) => ClipSource::from_path(p),
            Armed::Input(source) => Some(source.clone()),
        }
    }

    /// Cameras, then Syphon servers, as clip sources for the browser.
    fn input_sources(&self) -> Vec<ClipSource> {
        let cameras =
            self.cameras.iter().map(|c| ClipSource::Camera { device: c.name.clone(), device_id: c.id.clone() });
        let servers =
            self.syphon_servers.iter().map(|s| ClipSource::Syphon { app: s.app.clone(), server: s.server.clone() });
        cameras.chain(servers).collect()
    }

    pub fn input_clicked(&mut self, i: usize) {
        if let Some(source) = self.input_sources().into_iter().nth(i) {
            self.arm(Armed::Input(source));
        }
    }

    pub fn input_double_clicked(&mut self, i: usize) {
        if let Some(source) = self.input_sources().into_iter().nth(i) {
            self.place_in_selection(source);
        }
    }

    pub fn refresh_inputs(&mut self) {
        self.cameras = live::list_cameras();
        self.syphon_servers = live::list_syphon_servers();
        self.syphon_polled = self.now();
        self.live_retry.clear();
        self.browser_dirty = true;
        let (c, s) = (self.cameras.len(), self.syphon_servers.len());
        self.set_status(format!("Found {c} camera{} and {s} Syphon server{}", plural(c), plural(s)));
    }

    fn arm(&mut self, armed: Armed) {
        self.armed = if self.armed.as_ref() == Some(&armed) { None } else { Some(armed) };
        self.browser_dirty = true;
        self.props_dirty = true;
    }

    /// Places straight into the selected slot, or the first empty slot of the selected layer.
    fn place_in_selection(&mut self, source: ClipSource) {
        let target = self.selected.or_else(|| {
            let layer = self.comp.layers.get(self.selected_layer)?;
            layer.clips.iter().position(Option::is_none).map(|c| (self.selected_layer, c))
        });
        self.armed = None;
        self.browser_dirty = true;
        match target {
            Some((l, c)) => self.place(l, c, source),
            None => self.set_status("Select a slot first (right-click a cell)"),
        }
    }

    /// The browser's generator row `i`: the built-ins, then ISF generators.
    fn generator_row(&self, i: usize) -> Option<Armed> {
        match i.checked_sub(GeneratorKind::ALL.len()) {
            None => Some(Armed::Generator(i)),
            Some(j) => self.isf_generators.get(j).cloned().map(Armed::Isf),
        }
    }

    pub fn generator_clicked(&mut self, i: usize) {
        if let Some(armed) = self.generator_row(i) {
            self.arm(armed);
        }
    }

    pub fn generator_double_clicked(&mut self, i: usize) {
        if let Some(source) = self.generator_row(i).and_then(|a| self.armed_source(&a)) {
            self.place_in_selection(source);
        }
    }

    pub fn file_clicked(&mut self, i: usize) {
        let Some(entry) = self.browser_entries.get(i).cloned() else { return };
        if entry.source.is_none() {
            self.set_browser_dir(entry.path);
        } else {
            self.arm(Armed::File(entry.path));
        }
    }

    pub fn file_double_clicked(&mut self, i: usize) {
        if let Some(source) = self.browser_entries.get(i).and_then(|e| e.source.clone()) {
            self.place_in_selection(source);
        }
    }

    pub fn browser_up(&mut self) {
        if let Some(parent) = self.browser_dir.parent().map(Path::to_owned) {
            self.set_browser_dir(parent);
        }
    }

    pub fn set_browser_dir(&mut self, dir: PathBuf) {
        self.browser_entries = browser::list(&dir);
        self.browser_dir = dir;
        self.browser_dirty = true;
    }

    // ---------------------------------------------------------------- transport & keys

    pub fn tap(&mut self) {
        let now = self.now();
        if let Some(bpm) = self.tap.tap(now) {
            self.set_bpm((bpm * 10.0).round() / 10.0);
        }
    }

    pub fn nudge_bpm(&mut self, delta: f32) {
        self.set_bpm((self.comp.bpm + delta).clamp(20.0, 300.0).round());
    }

    fn set_bpm(&mut self, bpm: f32) {
        self.comp.bpm = bpm;
        if self.link.is_enabled() {
            self.link.set_tempo(bpm);
        }
        self.grid_dirty = true;
    }

    pub fn syphon_toggle(&mut self) {
        let on = !self.syphon_out.is_enabled();
        self.syphon_out.set_enabled(on);
        self.set_status(if on {
            "Syphon output on: other apps can pick openVJ as a source"
        } else {
            "Syphon output off"
        });
    }

    pub fn link_toggle(&mut self) {
        if !Link::AVAILABLE {
            self.set_status("This build has no Ableton Link support (built without the `link` feature)");
            return;
        }
        let on = !self.link.is_enabled();
        self.link.set_enabled(on, self.comp.bpm);
        self.set_status(if on {
            "Ableton Link on: tempo and beat follow the Link session"
        } else {
            "Ableton Link off"
        });
    }

    // ---------------------------------------------------------------- audio

    pub fn audio_toggle(&mut self) {
        if self.audio.take().is_some() {
            self.analyzer.reset();
            self.audio_status = "Off".into();
            self.set_status("Audio input off");
        } else {
            self.open_audio();
        }
    }

    /// Picks an entry of the device list (0 = the system default).
    pub fn audio_pick(&mut self, i: usize) {
        self.audio_device = i.min(self.audio_devices.len().saturating_sub(1));
        self.audio = None;
        self.open_audio();
    }

    fn open_audio(&mut self) {
        // Enumerate lazily: some hosts are slow to list devices or ask for permission.
        if self.audio_devices.len() == 1 {
            self.audio_devices.extend(openvj_audio::input_device_names());
        }
        let name = (self.audio_device > 0).then(|| self.audio_devices.get(self.audio_device).cloned()).flatten();
        match AudioInput::open(name.as_deref()) {
            Ok(input) => {
                let (gain, release) = (self.analyzer.gain, self.analyzer.release);
                self.analyzer = Analyzer::new(input.sample_rate());
                self.analyzer.gain = gain;
                self.analyzer.release = release;
                self.audio_status = format!("{} · {} Hz", input.name(), input.sample_rate());
                self.set_status(format!("Audio input: {}", input.name()));
                self.audio = Some(input);
            }
            Err(e) => {
                self.audio_status = format!("⚠ {e:#}");
                self.set_status(format!("Could not open audio input: {e:#}"));
            }
        }
    }

    // ---------------------------------------------------------------- MIDI

    pub fn midi_toggle(&mut self) {
        if self.midi.take().is_some() {
            self.midi_status = "Off".into();
            self.midi_dirty = true;
        } else {
            self.open_midi();
        }
    }

    /// (Re)connects to every MIDI input.
    fn open_midi(&mut self) {
        self.midi = None;
        let (ports, errors) = MidiPorts::open_all();
        for e in &errors {
            eprintln!("midi: {e}");
        }
        self.midi_port_list = ports.names().map(str::to_owned).collect();
        self.midi_status = if ports.is_empty() {
            "On · no MIDI inputs found yet".into()
        } else {
            format!("On · {}", self.midi_port_list.join(", "))
        };
        self.midi_scan_at = self.now();
        self.midi = Some(ports);
        self.midi_dirty = true;
    }

    /// Applies incoming MIDI to the composition. Runs once per frame.
    fn update_midi(&mut self, now: f64) {
        if self.midi.is_none() {
            return;
        }
        // Pick up controllers plugged in (or out) while running.
        if now - self.midi_scan_at > 2.0 {
            self.midi_scan_at = now;
            match midi::port_names() {
                Some(names) if names != self.midi_port_list => self.open_midi(),
                Some(_) => {}
                // No MIDI service at all: stop asking until MIDI is switched off and on.
                None => self.midi_scan_at = f64::INFINITY,
            }
        }
        let Some(ports) = &self.midi else { return };
        let connected: Vec<String> = ports.names().map(str::to_owned).collect();
        let messages: Vec<(String, Vec<u8>)> = ports.drain().into_iter().map(|(p, m)| (p.to_owned(), m)).collect();
        for (port, bytes) in messages {
            let Some(msg) = MidiMessage::parse(&bytes) else { continue };
            let actions = self.comp.midi.handle_from(msg, &port, &connected);
            self.midi_last =
                format!("{port}: {}{}", message_label(msg), if actions.is_empty() { " · not mapped" } else { "" });
            self.midi_dirty = true;
            for action in actions {
                match apply_midi(&mut self.comp, action, now, self.selected_layer) {
                    MidiEffect::Changed => {
                        if matches!(action.target, openvj_core::MidiTarget::Tempo) {
                            self.set_bpm(self.comp.bpm);
                        }
                        self.grid_dirty = true;
                    }
                    MidiEffect::SelectLayer(l) => self.select_layer(l as i32),
                    MidiEffect::TapTempo => self.tap(),
                    MidiEffect::OpenDeck(d) => self.deck_clicked(d),
                    MidiEffect::StepColumn { forward } => {
                        let n = self.comp.columns.len();
                        let col = match self.last_column {
                            Some(c) if forward => (c + 1) % n,
                            Some(c) => (c + n - 1) % n,
                            None => 0,
                        };
                        self.column_clicked(col);
                    }
                    MidiEffect::ResyncTempo => {
                        if !self.link.is_enabled() {
                            self.beat_clock.sync(now, 0.0, self.comp.bpm);
                        }
                    }
                    MidiEffect::Ignored => {}
                }
            }
        }
    }

    /// Loads a Resolume MIDI shortcut preset into the composition, replacing its mappings.
    pub fn midi_import(&mut self, path: &Path) {
        match openvj_resolume::import_midi_preset_file(path, &self.comp, self.resolume_refs.as_ref()) {
            Ok(imported) => {
                for note in &imported.notes {
                    eprintln!("midi import: {note}");
                }
                let msg = format!("Imported MIDI preset {}: {}", imported.map.name, imported.summary());
                self.comp.midi = imported.map;
                self.set_status(msg);
                self.midi_dirty = true;
            }
            Err(e) => self.set_status(format!("Could not import {}: {e}", path.display())),
        }
    }

    pub fn midi_clear(&mut self) {
        self.comp.midi = Default::default();
        self.midi_dirty = true;
        self.set_status("MIDI mappings cleared");
    }

    fn sync_midi(&mut self, ui: &MainWindow) {
        ui.set_midi_on(self.midi.is_some());
        ui.set_midi_status(self.midi_status.as_str().into());
        let map = &self.comp.midi;
        let preset = if map.is_empty() {
            "No mappings. Import a Resolume MIDI preset to control openVJ from a controller.".to_owned()
        } else {
            let name = if map.name.is_empty() { "Mappings" } else { map.name.as_str() };
            let n = map.mappings.len();
            let device = map.device.as_deref().map(|d| format!(" for {d}")).unwrap_or_default();
            format!("{name}: {n} mapping{}{device}", plural(n))
        };
        ui.set_midi_preset(preset.into());
        ui.set_midi_has_mappings(!map.is_empty());
        ui.set_midi_last(self.midi_last.as_str().into());
    }

    // ---------------------------------------------------------------- parameters & effects

    fn effects_mut(&mut self, scope: i32) -> Option<&mut Vec<Effect>> {
        match scope {
            scope::COMP_FX => Some(&mut self.comp.effects),
            scope::LAYER_FX => self.comp.layers.get_mut(self.selected_layer).map(|l| &mut l.effects),
            scope::GROUP_FX => self.selected_group_mut().map(|g| &mut g.effects),
            _ => None,
        }
    }

    fn effect_param_mut(&mut self, scope: i32, owner: i32, index: usize) -> Option<&mut Param> {
        let fx = self.effects_mut(scope)?.get_mut(usize::try_from(owner).ok()?)?;
        if index == MIX_INDEX {
            return Some(&mut fx.mix);
        }
        let key = fx.ty()?.channels.get(index)?.key.clone();
        fx.param_mut(&key)
    }

    /// Channel `index` of the selected clip's ISF generator.
    fn isf_param_mut(&mut self, index: usize) -> Option<&mut Param> {
        let (l, c) = self.selected?;
        let ClipSource::Isf { generator } = &mut self.comp.clip_mut(l, c)?.source else { return None };
        let key = generator.ty()?.channels.get(index)?.key.clone();
        generator.param_mut(&key)
    }

    fn modulation_mut(&mut self, scope: i32, owner: i32, index: usize) -> Option<&mut Modulation> {
        match (scope, index) {
            (scope::COMP, 0) => Some(&mut self.comp.master_mod),
            (scope::COMP, CROSSFADER_INDEX) => Some(&mut self.comp.crossfader.position_mod),
            (scope::GROUP, 0) => self.selected_group_mut().map(|g| &mut g.opacity_mod),
            (scope::COMP_FX | scope::LAYER_FX | scope::GROUP_FX, _) => {
                self.effect_param_mut(scope, owner, index).map(|p| &mut p.modulation)
            }
            (scope::LAYER, 0) => {
                let l = self.layer_index(owner)?;
                Some(&mut self.comp.layers[l].opacity_mod)
            }
            (scope::CLIP, i) if i >= ISF_PARAM_BASE => {
                self.isf_param_mut(i - ISF_PARAM_BASE).map(|p| &mut p.modulation)
            }
            (scope::CLIP, i) => {
                let (l, c) = self.selected?;
                self.comp.clip_mut(l, c)?.mods.get_mut(i)
            }
            _ => None,
        }
    }

    pub fn param_set(&mut self, scope: i32, owner: i32, index: usize, v: f32) {
        let v = v.clamp(0.0, 1.0);
        match scope {
            scope::COMP => match index {
                0 => self.comp.master = v,
                1 => self.analyzer.gain = (v * MAX_GAIN * 100.0).round() / 100.0,
                2 => self.analyzer.release = (v * MAX_RELEASE).max(0.01),
                CROSSFADER_INDEX => self.comp.crossfader.position = v,
                _ => {}
            },
            scope::GROUP => {
                if let (0, Some(g)) = (index, self.selected_group_mut()) {
                    g.opacity = v;
                }
            }
            scope::COMP_FX | scope::LAYER_FX | scope::GROUP_FX => {
                if let Some(p) = self.effect_param_mut(scope, owner, index) {
                    p.value = v;
                }
            }
            scope::LAYER => {
                let Some(l) = self.layer_index(owner) else { return };
                let layer = &mut self.comp.layers[l];
                match index {
                    0 => layer.opacity = v,
                    1 => layer.transition_secs = (v * MAX_TRANSITION * 10.0).round() / 10.0,
                    _ => {}
                }
            }
            scope::CLIP if index >= ISF_PARAM_BASE => {
                if let Some(p) = self.isf_param_mut(index - ISF_PARAM_BASE) {
                    p.value = v;
                }
            }
            scope::CLIP => {
                let Some(clip) = self.selected.and_then(|(l, c)| self.comp.clip_mut(l, c)) else { return };
                if index == SPEED_INDEX {
                    clip.speed = (v * MAX_SPEED * 100.0).round() / 100.0;
                } else if index == BEATS_INDEX {
                    let i = (v * (SYNC_BEATS.len() - 1) as f32).round() as usize;
                    clip.sync_beats = Some(SYNC_BEATS[i.min(SYNC_BEATS.len() - 1)]);
                } else if index == DURATION_INDEX {
                    clip.duration = Some(((v * MAX_DURATION * 10.0).round() / 10.0).max(0.1));
                } else if let Some(p) = clip.params.get_mut(index) {
                    *p = v;
                }
            }
            _ => {}
        }
        self.grid_dirty = true;
    }

    pub fn param_mod(&mut self, scope: i32, owner: i32, index: usize, source: usize) {
        if let Some(m) = self.modulation_mut(scope, owner, index) {
            m.source = ModSource::from_index(source);
            self.grid_dirty = true;
        }
    }

    pub fn param_mod_amount(&mut self, scope: i32, owner: i32, index: usize, amount: f32) {
        if let Some(m) = self.modulation_mut(scope, owner, index) {
            m.amount = (amount.clamp(-1.0, 1.0) * 100.0).round() / 100.0;
        }
    }

    pub fn param_mod_rate(&mut self, scope: i32, owner: i32, index: usize, rate: usize) {
        if let Some(m) = self.modulation_mut(scope, owner, index) {
            m.beats = MOD_RATES[rate.min(MOD_RATES.len() - 1)];
        }
    }

    /// Refreshes the Effects tab when effect folders changed.
    fn update_effect_library(&mut self, ui: &MainWindow) {
        let lib = library();
        if self.effects_generation == Some(lib.generation) {
            return;
        }
        let first = self.effects_generation.is_none();
        self.effects_generation = Some(lib.generation);
        let mut items: Vec<BrowserItem> = lib
            .all()
            .iter()
            .map(|k| BrowserItem {
                name: k.name().into(),
                detail: k.def.description.as_str().into(),
                kind: "fx".into(),
                armed: false,
            })
            .collect();
        // Broken files come last, so a row index is still an index into `lib.all()`.
        items.extend(lib.errors.iter().map(|e| BrowserItem {
            name: e.name.as_str().into(),
            detail: e.message.lines().find(|l| !l.trim().is_empty()).unwrap_or_default().trim().into(),
            kind: "fx-error".into(),
            armed: false,
        }));
        ui.set_effects(ModelRc::new(VecModel::from(items)));
        let folder = openvj_core::effect_folders::user_dir()
            .map(|d| browser::short_path(&d, 60))
            .unwrap_or_else(|| "the openVJ effects folder".into());
        ui.set_effects_hint(format!("Drop ISF effects (.fs) into {folder}").into());
        // Cards show effect names and parameters, and the Sources tab lists ISF generators,
        // which a reload can change.
        self.props_dirty = true;
        self.browser_dirty = true;
        if !first && lib.generation > 1 {
            let added = lib.all().iter().filter(|t| t.path.is_some()).count();
            let broken = match lib.errors.len() {
                0 => String::new(),
                n => format!(", {n} with errors (see the Effects tab)"),
            };
            self.set_status(format!("Effects reloaded: {added} from folders{broken}"));
        }
    }

    /// Opens the openVJ effects folder in Finder (or the system file manager).
    pub fn open_effects_folder(&mut self) {
        let Some(dir) = openvj_core::effect_folders::user_dir() else { return };
        let _ = std::fs::create_dir_all(&dir);
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(windows) {
            "explorer"
        } else {
            "xdg-open"
        };
        if let Err(e) = std::process::Command::new(opener).arg(&dir).spawn() {
            self.set_status(format!("Couldn't open {}: {e}", dir.display()));
        }
    }

    pub fn effect_add(&mut self, scope: i32, kind: usize) {
        let lib = library();
        let Some(kind) = lib.all().get(kind) else { return };
        let layer = self.comp.layers.get(self.selected_layer);
        let target = match scope {
            scope::COMP_FX => "the composition".to_owned(),
            scope::GROUP_FX => layer.and_then(|l| self.comp.group_of(l)).map(|g| g.name.clone()).unwrap_or_default(),
            _ => layer.map(|l| l.name.clone()).unwrap_or_default(),
        };
        let Some(chain) = self.effects_mut(scope) else { return };
        chain.push(Effect::new(kind.id()));
        self.tab_request = Some(match scope {
            scope::COMP_FX => 0,
            scope::GROUP_FX => 3,
            _ => 1,
        });
        self.grid_dirty = true;
        self.set_status(format!("Added {} to {target}", kind.name()));
    }

    pub fn effect_toggle(&mut self, scope: i32, i: usize) {
        if let Some(fx) = self.effects_mut(scope).and_then(|c| c.get_mut(i)) {
            fx.enabled ^= true;
        }
    }

    pub fn effect_remove(&mut self, scope: i32, i: usize) {
        if let Some(chain) = self.effects_mut(scope)
            && i < chain.len()
        {
            chain.remove(i);
            self.grid_dirty = true;
        }
    }

    pub fn effect_move(&mut self, scope: i32, i: usize, delta: i32) {
        if let Some(chain) = self.effects_mut(scope) {
            move_effect(chain, i, delta);
        }
    }

    pub fn master(&mut self, v: f32) {
        self.comp.master = v.clamp(0.0, 1.0);
        self.grid_dirty = true;
    }

    /// Keyboard shortcuts. Returns true if handled.
    pub fn key(&mut self, text: &str) -> bool {
        match text {
            "\u{1b}" => {
                if self.armed.take().is_some() {
                    self.browser_dirty = true;
                } else if self.escape_at.take().is_some() {
                    self.clear_all();
                } else {
                    self.escape_at = Some(self.now());
                }
                self.props_dirty = true;
                true
            }
            "\u{7f}" | "\u{8}" => {
                self.clip_remove();
                true
            }
            _ => match text.parse::<usize>() {
                Ok(n @ 1..=9) if n <= self.comp.columns.len() => {
                    self.column_clicked(n - 1);
                    true
                }
                _ => false,
            },
        }
    }

    pub fn output_shown(&mut self, open: bool) {
        self.output_open = open;
        self.grid_dirty = true;
        self.mapping_dirty = true;
    }

    pub fn import_mapping(&mut self, path: &Path) {
        match self.mapping.import(path) {
            Ok(msg) => self.set_status(msg),
            Err(e) => self.set_status(format!("Could not import {}: {e}", path.display())),
        }
        self.mapping_changed();
    }

    pub fn clear_mapping(&mut self) {
        self.mapping.clear();
        self.set_status("Mapping cleared: the output window shows the whole composition.");
        self.mapping_changed();
    }

    pub fn toggle_test_card(&mut self) {
        self.mapping.test_card = !self.mapping.test_card && self.mapping.setup.is_some();
        self.grid_dirty = true;
        self.mapping_view_dirty = true;
    }

    fn mapping_changed(&mut self) {
        self.mapping_view_dirty = true;
        self.mapping_dirty = true;
        self.images_dirty = true;
        self.grid_dirty = true;
    }

    pub fn status_message(&mut self, msg: &str) {
        self.set_status(msg);
    }
}

/// Replaces a model's rows, touching only rows that changed (keeps widget state stable).
fn update_model<T: Clone + PartialEq + 'static>(model: &VecModel<T>, rows: Vec<T>) {
    if model.row_count() != rows.len() {
        model.set_vec(rows);
        return;
    }
    for (i, row) in rows.into_iter().enumerate() {
        if model.row_data(i).as_ref() != Some(&row) {
            model.set_row_data(i, row);
        }
    }
}

fn pct(v: f32) -> String {
    format!("{:.0}%", v * 100.0)
}

fn crossfade_label(v: f32) -> String {
    match (v * 100.0).round() as i32 {
        50 => "centre".into(),
        p if p < 50 => format!("A {}%", 100 - p),
        p => format!("B {p}%"),
    }
}

/// Index of the [`SYNC_BEATS`] entry closest to `beats`.
fn beat_choice(beats: f32) -> usize {
    SYNC_BEATS
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1 - beats).abs().total_cmp(&(b.1 - beats).abs()))
        .map_or(0, |(i, _)| i)
}

/// A modulatable row; the display shows the live value. Double-clicking the bar resets it to `reset`.
fn param_row(
    index: usize,
    name: &str,
    p: &Param,
    reset: f32,
    inputs: &ModInputs,
    display: impl Fn(f32) -> String,
) -> ParamData {
    let live = p.eval(inputs);
    let m = p.modulation;
    ParamData {
        index: index as i32,
        name: name.into(),
        value: p.value,
        live,
        reset,
        display: display(live).into(),
        modulatable: true,
        mod_source: m.source.index() as i32,
        mod_amount: m.amount,
        mod_rate: m.rate_index() as i32,
    }
}

/// A row without a modulator (speed, transition, analyser settings).
fn fixed_row(index: usize, name: &str, value: f32, reset: f32, display: String) -> ParamData {
    ParamData {
        index: index as i32,
        name: name.into(),
        value,
        live: value,
        reset,
        display: display.into(),
        modulatable: false,
        ..Default::default()
    }
}

/// UI models for one effect chain, with a nested parameter model per card so cards
/// update in place.
#[derive(Default)]
struct EffectModels {
    effects: Rc<VecModel<EffectData>>,
    params: Vec<Rc<VecModel<ParamData>>>,
}

impl EffectModels {
    fn update(&mut self, chain: &[Effect], inputs: &ModInputs) {
        self.params.resize_with(chain.len(), || Rc::new(VecModel::default()));
        let mut rows = Vec::with_capacity(chain.len());
        for (fx, model) in chain.iter().zip(&self.params) {
            let mut params: Vec<ParamData> = match fx.ty() {
                Some(ty) => ty
                    .channels
                    .iter()
                    .enumerate()
                    .map(|(i, ch)| {
                        let input = ty.input(ch);
                        let p = fx.param(&ch.key).copied().unwrap_or(Param::new(ch.default));
                        param_row(i, &ch.label, &p, ch.default, inputs, |v| input.display(ch.component, v))
                    })
                    .collect(),
                None => Vec::new(),
            };
            params.push(param_row(MIX_INDEX, "Mix", &fx.mix, 1.0, inputs, pct));
            update_model(model, params);
            rows.push(EffectData { name: fx.name().into(), enabled: fx.enabled, params: ModelRc::from(model.clone()) });
        }
        update_model(&self.effects, rows);
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn message_label(msg: MidiMessage) -> String {
    match msg {
        MidiMessage::NoteOn { channel, note, velocity } => format!("Note {note} on, ch {} vel {velocity}", channel + 1),
        MidiMessage::NoteOff { channel, note } => format!("Note {note} off, ch {}", channel + 1),
        MidiMessage::ControlChange { channel, controller, value } => {
            format!("CC {controller} = {value}, ch {}", channel + 1)
        }
    }
}

/// Loads the MIDI preset an `.avc` was saved with into `comp`, if it can be found next to the
/// `.avc` or in Resolume's preset folders. Returns a line for the status bar.
fn load_midi_preset_for(comp: &mut Composition, avc: &Path, refs: &openvj_resolume::ResolumeRefs) -> Option<String> {
    let name = refs.midi_preset.as_deref()?;
    let mut dirs: Vec<PathBuf> = avc.parent().map(Path::to_owned).into_iter().collect();
    dirs.extend(openvj_resolume::midi_preset_dirs());
    let Some(path) = openvj_resolume::find_midi_preset(name, &dirs) else {
        return Some(format!("MIDI preset \"{name}\" not found, import it from the Composition tab"));
    };
    match openvj_resolume::import_midi_preset_file(&path, comp, Some(refs)) {
        Ok(imported) => {
            for note in &imported.notes {
                eprintln!("midi import: {note}");
            }
            let line = format!("MIDI preset {name}: {}", imported.summary());
            comp.midi = imported.map;
            Some(line)
        }
        Err(e) => Some(format!("Could not read MIDI preset {}: {e}", path.display())),
    }
}
