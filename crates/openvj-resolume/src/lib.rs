//! Imports Resolume Arena/Avenue compositions (`.avc`) into an openVJ [`Composition`].
//!
//! An `.avc` file is XML. The parts openVJ understands are read and everything else is
//! listed in the [`ImportReport`], so an import never fails just because a composition uses
//! something openVJ doesn't have (Wire patches, text generators, clip effects and so on).
//!
//! Resolume only writes a parameter when it differs from its default, so every lookup here
//! falls back to Resolume's default when the element is missing.
//!
//! Resolume has several decks per composition and openVJ has one grid, so only the deck that
//! was open when the file was saved is imported.

mod advanced_output;
mod midi;
mod relink;

use std::collections::HashMap;
use std::path::Path;

use openvj_core::{
    BlendMode, Clip, ClipSource, Column, Composition, CrossfadeCurve, CrossfadeSide, Effect, GeneratorKind, LayerGroup,
    Param, PlayMode, default_transform, library,
};
use roxmltree::{Document, Node};

pub use advanced_output::{ImportedOutput, import_output_file, import_output_str};
pub use midi::{MidiImport, find_midi_preset, import_midi_preset, import_midi_preset_file, midi_preset_dirs};
pub use relink::{RelinkResult, file_name, relink_missing};

/// A composition read from an `.avc` file, plus what could not be carried over.
#[derive(Debug, Clone)]
pub struct Imported {
    pub composition: Composition,
    pub report: ImportReport,
    /// How Resolume's names for things map onto the imported composition, for resolving
    /// the paths in MIDI shortcut presets.
    pub refs: ResolumeRefs,
}

/// Resolume addresses layers, effects and clips by path (such as
/// `/composition/layers/2/video/effects/blur2/effect/blur`). This records where the things
/// those paths name ended up in the imported composition.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResolumeRefs {
    /// Layer `uniqueId`s (used in `/composition/objects/<id>/...` paths) to layer index.
    pub layer_ids: HashMap<u64, usize>,
    /// (layer index or `None` for the composition, effect path name such as "blur2") to
    /// the effect's index in the imported chain. Effects that weren't imported are absent.
    pub effects: HashMap<(Option<usize>, String), usize>,
    /// The MIDI shortcut preset the composition was saved with, such as "Circuit Tracks".
    pub midi_preset: Option<String>,
}

/// What happened during an import, for showing to the user.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImportReport {
    /// The Resolume version that saved the file, such as "Resolume Arena 7.22.4".
    pub saved_by: Option<String>,
    /// Names of the decks that were imported, left to right in the grid.
    pub decks: Vec<String>,
    /// Clips placed in the grid.
    pub clips: usize,
    /// Clips left empty because openVJ has no equivalent source.
    pub skipped_clips: usize,
    /// Layer and composition effects that were carried over.
    pub effects: usize,
    /// One line per thing that was dropped or approximated.
    pub notes: Vec<String>,
}

impl ImportReport {
    /// A one-line summary for a status bar.
    pub fn summary(&self) -> String {
        let mut s = format!("{} clip{}", self.clips, plural(self.clips));
        if self.effects > 0 {
            s += &format!(", {} effect{}", self.effects, plural(self.effects));
        }
        if self.skipped_clips > 0 {
            s += &format!(", {} clip{} skipped", self.skipped_clips, plural(self.skipped_clips));
        }
        let others = self.notes.len().saturating_sub(self.skipped_clips);
        if others > 0 {
            s += &format!(", {} other note{}", others, plural(others));
        }
        s
    }

    fn note(&mut self, msg: impl Into<String>) {
        self.notes.push(msg.into());
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// Reads an `.avc` file and points missing media at copies found next to it.
pub fn import_file(path: &Path) -> Result<Imported, String> {
    let xml = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut imported = import_str(&xml)?;
    if let Some(dir) = path.parent() {
        let relinked = relink_missing(&mut imported.composition, &[dir.to_owned()]);
        if relinked.relinked > 0 {
            imported.report.note(format!(
                "Found {} missing media file{} next to the composition",
                relinked.relinked,
                plural(relinked.relinked)
            ));
        }
        for missing in relinked.missing {
            imported.report.note(format!("Media not found: {}", missing.display()));
        }
    }
    Ok(imported)
}

/// Parses the text of an `.avc` file. Does not touch the file system.
pub fn import_str(xml: &str) -> Result<Imported, String> {
    let doc = Document::parse(xml).map_err(|e| format!("not valid XML: {e}"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "Composition" {
        return Err("not a Resolume composition (the root element isn't <Composition>)".into());
    }
    let saved_by = child(root, "versionInfo").map(|v| {
        let app = v.attribute("name").unwrap_or("Resolume");
        let part = |a| v.attribute(a).unwrap_or("?");
        format!("{app} {}.{}.{}", part("majorVersion"), part("minorVersion"), part("microVersion"))
    });
    let mut report = ImportReport { saved_by, ..Default::default() };
    let mut refs = ResolumeRefs {
        midi_preset: param_str(root, "MidiShortcutPreset")
            .filter(|n| !n.is_empty() && *n != "Default")
            .map(str::to_owned),
        ..Default::default()
    };

    let (width, height) = resolution(root);
    let layers: Vec<Node> = children(root, "Layer").collect();
    let n_layers = layers.len().max(1);
    // Every deck with clips becomes an openVJ deck, in Resolume's tab order. Empty decks
    // are left out.
    let decks: Vec<(Node, usize)> = decks_to_import(root).into_iter().map(|d| (d, deck_width(d))).collect();
    let n_cols = decks.first().map_or(1, |d| d.1);

    let mut comp = Composition::new(width, height, n_layers, n_cols);
    comp.name = param_str(root, "Name").or_else(|| root.attribute("name")).unwrap_or("Resolume import").to_owned();
    if let Some(bpm) = child(root, "TempoController").and_then(|t| param_f64(t, "Tempo")) {
        comp.bpm = bpm as f32;
    }
    if let Some(vt) = child(root, "VideoTrack") {
        if let Some(master) = param_f64(vt, "Opacity").or_else(|| mixer_opacity(vt)) {
            comp.master = master.clamp(0.0, 1.0) as f32;
        }
        comp.effects = effect_chain(vt, width, height, "composition", None, &mut report, &mut |name, i| {
            refs.effects.insert((None, name), i);
        });
    }

    // Layers sit bottom-up in both apps: Resolume's layerIndex 0 is openVJ's layer 0.
    // Groups come before their layers, which refer to them by position.
    for (i, node) in children(root, "Group").enumerate() {
        let label = format!("group {}", i + 1);
        let mut group = LayerGroup::new(format!("Group {}", i + 1));
        if let Some(name) = param_str(node, "Name").filter(|n| !n.trim().is_empty() && *n != "Group") {
            group.name = name.trim().to_owned();
        }
        let track = track_settings(node, width, height, &label, false, &mut report, &mut |_, _| {});
        (group.opacity, group.blend, group.effects) = (track.opacity, track.blend, track.effects);
        group.bypass = track.bypass;
        group.solo = track.solo;
        comp.groups.push(group);
    }
    if !comp.groups.is_empty() {
        report.note(format!("{} layer group{} imported", comp.groups.len(), plural(comp.groups.len())));
    }

    for (i, node) in layers.iter().enumerate() {
        let idx = attr_usize(*node, "layerIndex").filter(|&l| l < n_layers).unwrap_or(i);
        let label = format!("layer {}", idx + 1);
        if let Some(id) = node.attribute("uniqueId").and_then(|v| v.parse().ok()) {
            refs.layer_ids.insert(id, idx);
        }
        let layer = &mut comp.layers[idx];
        // Resolume replaces "#" in a layer name with the layer's number.
        if let Some(name) = param_str(*node, "Name").map(|n| n.replace('#', &(idx + 1).to_string()))
            && !name.trim().is_empty()
        {
            layer.name = name.trim().to_owned();
        }
        if let Some(secs) = child(*node, "ClipTransition").and_then(|t| param_f64(t, "Duration")) {
            layer.transition_secs = secs.max(0.0) as f32;
        }
        let track = track_settings(*node, width, height, &label, true, &mut report, &mut |name, i| {
            refs.effects.insert((Some(idx), name), i);
        });
        (layer.opacity, layer.blend, layer.effects) = (track.opacity, track.blend, track.effects);
        layer.transform = track.transform;
        layer.bypass = track.bypass;
        layer.solo = track.solo;
        layer.group = attr_usize(*node, "layerGroup").filter(|&g| g < comp.groups.len());
        // Resolume's crossfader groups: 0 none, 1 side A, 2 side B.
        layer.crossfade = match param_f64(*node, "CrossFaderGroup").map(|v| v.round() as i64) {
            Some(1) => CrossfadeSide::A,
            Some(2) => CrossfadeSide::B,
            _ => CrossfadeSide::None,
        };
    }
    report.effects = comp.effects.len()
        + comp.layers.iter().map(|l| l.effects.len()).sum::<usize>()
        + comp.groups.iter().map(|g| g.effects.len()).sum::<usize>();
    import_crossfader(&mut comp, root, &mut report);

    let saved_open = attr_usize(root, "currentDeckIndex");
    let mut open_after = 0;
    for (i, (deck, cols)) in decks.into_iter().enumerate() {
        if i > 0 {
            comp.add_deck();
            while comp.columns.len() < cols {
                comp.add_column();
            }
            while comp.columns.len() > cols {
                comp.remove_column(comp.columns.len() - 1);
            }
        }
        let deck_name = param_str(deck, "Name").unwrap_or("Deck").to_owned();
        comp.rename_deck(i, &deck_name);
        for (c, column) in comp.columns.iter_mut().enumerate() {
            column.name = (c + 1).to_string();
        }
        for col in deck_columns(deck) {
            if let (Some(idx), Some(name)) =
                (attr_usize(col, "columnIndex").filter(|&c| c < cols), param_str(col, "Name").filter(|n| !n.is_empty()))
            {
                comp.columns[idx] = Column { name: name.to_owned() };
            }
        }
        for clip in children(deck, "Clip") {
            import_clip(&mut comp, clip, &mut report);
        }
        if saved_open.is_some() && attr_usize(deck, "deckIndex") == saved_open {
            open_after = i;
        }
        report.decks.push(deck_name);
    }
    // Open the deck that was open in Resolume when the file was saved.
    comp.switch_deck(open_after);
    playback_note(&comp, &mut report);
    Ok(Imported { composition: comp, report, refs })
}

fn import_clip(comp: &mut Composition, node: Node, report: &mut ImportReport) {
    let (Some(layer), Some(col)) = (attr_usize(node, "layerIndex"), attr_usize(node, "columnIndex")) else {
        return;
    };
    // Empty slots are saved as a bare <Clip> with no source.
    let Some(source_node) = node.descendants().find(|n| n.has_tag_name("VideoSource")) else { return };
    let name = param_str(node, "Name").map(str::to_owned);
    let label = format!("\"{}\" (layer {}, column {})", name.as_deref().unwrap_or("clip"), layer + 1, col + 1);

    let source = match source_node.attribute("type") {
        Some("GeneratorVideoSource") => {
            let pass = source_node.children().filter(|n| n.has_tag_name("RenderPass"));
            let ty = pass
                .clone()
                .find(|n| n.attribute("baseType") == Some("Generator"))
                .or_else(|| pass.clone().find(|n| n.attribute("type") != Some("RenderPassChain")))
                .and_then(|n| n.attribute("type"))
                .unwrap_or("?");
            match generator(ty) {
                Some(kind) => {
                    report.note(format!("{label}: Resolume's {ty} generator was swapped for openVJ's {}", kind.name()));
                    Ok(ClipSource::Generator { kind })
                }
                None => Err(format!("the {ty} generator has no openVJ equivalent")),
            }
        }
        Some("CaptureDeviceVideoSource") => camera(node, source_node),
        Some("SyphonVideoSource") => source_node
            .attribute("serverName")
            .or_else(|| source_node.children().find(|n| n.has_tag_name("SyphonInfo"))?.attribute("serverName"))
            .filter(|s| !s.is_empty())
            .map(ClipSource::syphon_from_label)
            .ok_or_else(|| "the Syphon input has no server name".to_owned()),
        ty => match node.descendants().find(|n| n.has_tag_name("VideoFile")).and_then(|n| n.attribute("value")) {
            Some(path) => ClipSource::from_path(Path::new(path))
                .ok_or_else(|| format!("unsupported media type: {}", file_name(path).unwrap_or(path))),
            None => Err(format!("{} isn't supported yet", source_kind(ty.unwrap_or("?")))),
        },
    };
    let source = match source {
        Ok(s) => s,
        Err(why) => {
            report.skipped_clips += 1;
            report.note(format!("{label} skipped: {why}"));
            return;
        }
    };
    let Some(_) = comp.set_clip(layer, col, source) else {
        report.skipped_clips += 1;
        report.note(format!("{label} skipped: outside the grid"));
        return;
    };
    let clip = comp.clip_mut(layer, col).expect("clip was just placed");
    if let Some(name) = name.filter(|n| !n.is_empty()) {
        clip.name = name;
    }
    if let Some(speed) = child(node, "Transport").and_then(|t| deep_param_f64(t, "Speed")) {
        clip.speed = speed as f32;
    }
    transport(clip, node, &label, report);
    report.clips += 1;
    if let Some(vt) = child(node, "VideoTrack") {
        let (width, height) = (comp.width, comp.height);
        let clip = comp.clip_mut(layer, col).expect("clip was just placed");
        let mut transform = default_transform();
        clip.effects = effect_chain(vt, width, height, &label, Some(&mut transform), report, &mut |_, _| {});
        clip.transform = transform;
        report.effects += clip.effects.len();
    }
}

/// Clip playback: transport mode (timeline or BPM sync), play mode and duration.
fn transport(clip: &mut Clip, node: Node, label: &str, report: &mut ImportReport) {
    let Some(position) = child(node, "Transport").and_then(|t| param(t, "Position")) else { return };
    let timeline = position.children().find(|n| n.tag_name().name().starts_with("PhaseSource"));
    let duration = child(position, "DurationSource");

    // TransportType: 0 Timeline, 1 BPM Sync. Others (SMPTE, Denon) follow a time code.
    match param_f64(node, "TransportType").map(|v| v.round() as i64).unwrap_or(0) {
        0 => {}
        1 => {
            let manual = timeline.and_then(|t| child(t, "Beats_d")).and_then(|b| b.attribute("numManualBeats"));
            let saved = duration.and_then(|d| d.attribute("defaultDuration")).and_then(|d| d.strip_suffix('b'));
            let beats = manual.or(saved).and_then(|b| b.trim().parse::<f32>().ok()).filter(|b| *b > 0.0);
            clip.sync_beats = Some(beats.unwrap_or(4.0));
        }
        other => {
            report.note(format!("{label}: transport mode {other} (time code) isn't supported, so it plays freely"))
        }
    }

    let mode = timeline.and_then(|t| param_f64(t, "PlayMode")).map(|v| v.round() as i64).unwrap_or(0);
    // Resolume's play mode menu: Loop, Bounce, Random, Play Once & Clear, Play Once & Hold.
    clip.play_mode = match mode {
        0..=4 => PlayMode::from_index(mode as usize),
        _ => {
            report.note(format!("{label}: play mode {mode} isn't known, using Loop"));
            PlayMode::Loop
        }
    };

    match clip.source {
        ClipSource::Video { .. } => {
            // The file's own length, as Resolume measured it.
            let ms =
                timeline.and_then(|t| t.attribute("defaultMillisecondsDuration")).and_then(|v| v.parse::<f64>().ok());
            clip.media_secs = ms.filter(|ms| *ms > 0.0).map(|ms| ms / 1000.0);
        }
        // Stills and generators only have the duration the user typed in.
        _ => {
            let user = duration.and_then(|d| d.attribute("userDuration")).and_then(|v| v.parse::<f32>().ok());
            clip.duration = user.filter(|d| *d > 0.0);
        }
    }
}

/// One note summing up the clips that don't simply loop on their own clock.
fn playback_note(comp: &Composition, report: &mut ImportReport) {
    let synced = comp.every_clip().filter(|c| c.sync_beats.is_some()).count();
    let mut parts = Vec::new();
    if synced > 0 {
        parts.push(format!("{synced} beat-synced"));
    }
    for mode in PlayMode::ALL.into_iter().skip(1) {
        let n = comp.every_clip().filter(|c| c.play_mode == mode).count();
        if n > 0 {
            parts.push(format!("{n} {}", mode.name().to_lowercase()));
        }
    }
    if !parts.is_empty() {
        report.note(format!("Clip playback: {}", parts.join(", ")));
    }
}

/// The settings every Resolume track (layer or group) has.
struct Track {
    opacity: f32,
    blend: BlendMode,
    bypass: bool,
    solo: bool,
    effects: Vec<Effect>,
    transform: Effect,
}

/// A layer's or group's mix settings and effects. With `fixed_transform`, Resolume's
/// Transform goes into the track's fixed Transform rather than its chain.
fn track_settings(
    node: Node,
    width: u32,
    height: u32,
    label: &str,
    fixed_transform: bool,
    report: &mut ImportReport,
    imported: &mut dyn FnMut(String, usize),
) -> Track {
    let mut t = Track {
        opacity: 1.0,
        blend: BlendMode::Normal,
        bypass: param_bool(node, "Bypassed").unwrap_or(false),
        solo: param_bool(node, "Solo").unwrap_or(false),
        effects: Vec::new(),
        transform: default_transform(),
    };
    let master = param_f64(node, "Master").unwrap_or(1.0);
    let Some(vt) = child(node, "VideoTrack") else {
        t.opacity = master.clamp(0.0, 1.0) as f32;
        return t;
    };
    let blend = mixer(vt, "Blend");
    if let Some(ty) = blend.and_then(|b| b.attribute("type")) {
        t.blend = blend_mode(ty).unwrap_or_else(|| {
            report.note(format!("{label}: blend mode \"{ty}\" isn't available, using Normal"));
            BlendMode::Normal
        });
    }
    // The video track's Opacity is the opacity fader. The blend mixer keeps its own copy,
    // which can be stale (Arena 7.26 saved 31% there for a layer showing 0%), so it's only
    // used when the track has none, as in files from 7.22.
    let opacity = param_f64(vt, "Opacity").or_else(|| blend.and_then(|b| param_f64(b, "Opacity")));
    t.opacity = (opacity.unwrap_or(1.0) * master).clamp(0.0, 1.0) as f32;
    let transform = fixed_transform.then_some(&mut t.transform);
    t.effects = effect_chain(vt, width, height, label, transform, report, imported);
    t
}

/// The composition's A/B crossfader: its position is the opacity of its mixer.
fn import_crossfader(comp: &mut Composition, root: Node, report: &mut ImportReport) {
    let Some(node) = child(root, "CrossFader") else { return };
    let vt = child(node, "VideoTrack");
    let position =
        vt.and_then(|vt| mixer(vt, "Blend mode").or_else(|| mixer(vt, "Blend"))).and_then(|m| param_f64(m, "Opacity"));
    if let Some(p) = position {
        comp.crossfader.position = p.clamp(0.0, 1.0) as f32;
    }
    let curve = param_f64(node, "Curve").map(|v| v.round() as i64).unwrap_or(0);
    comp.crossfader.curve = if curve == 0 { CrossfadeCurve::Linear } else { CrossfadeCurve::FullCentre };
    let sides = comp.layers.iter().filter(|l| l.crossfade != CrossfadeSide::None).count();
    if sides > 0 {
        report.note(format!(
            "Crossfader at {:.0}% towards B, with {sides} layer{} on a side",
            comp.crossfader.position * 100.0,
            plural(sides)
        ));
        if curve != 0 {
            report.note(format!(
                "Crossfader curve {curve} was imported as \"{}\", check it",
                comp.crossfader.curve.name()
            ));
        }
    }
}

/// A capture device. Resolume saves the device's AVFoundation id as
/// `CaptureAVFoundation_<uniqueID>_input`, and its name as the clip name's default (the
/// clip name itself may have been renamed).
fn camera(clip: Node, source: Node) -> Result<ClipSource, String> {
    let device_id = source
        .children()
        .find(|n| n.has_tag_name("CaptureSource"))
        .and_then(|n| n.attribute("deviceId"))
        .map(|id| id.strip_prefix("CaptureAVFoundation_").unwrap_or(id))
        .map(|id| id.strip_suffix("_input").unwrap_or(id))
        .unwrap_or_default()
        .to_owned();
    let device = param(clip, "Name").and_then(|p| p.attribute("default").or_else(|| p.attribute("value")));
    match device.filter(|d| !d.is_empty()) {
        Some(device) => Ok(ClipSource::Camera { device: device.to_owned(), device_id }),
        None => Err("the camera input has no device name".into()),
    }
}

/// A readable name for a Resolume source type openVJ can't play.
fn source_kind(ty: &str) -> String {
    match ty {
        "SpoutVideoSource" => "Spout input".into(),
        "NDIVideoSource" => "NDI input".into(),
        "CompositionRouterVideoSource" | "LayerRouterVideoSource" => "the video router".into(),
        other => format!("the \"{other}\" source"),
    }
}

// ---------------------------------------------------------------- composition structure

fn resolution(root: Node) -> (u32, u32) {
    let info = child(root, "CompositionInfo");
    let vt = child(root, "VideoTrack");
    let dim = |attr: &str, param: &str| {
        info.and_then(|i| attr_usize(i, attr))
            .map(|v| v as f64)
            .or_else(|| vt.and_then(|v| param_f64(v, param)))
            .filter(|v| *v >= 1.0)
            .map(|v| v.round() as u32)
    };
    (dim("width", "Width").unwrap_or(1920), dim("height", "Height").unwrap_or(1080))
}

/// Decks that contain clips, in tab order. If none do, the deck that was open, so the import
/// still gets a grid.
fn decks_to_import<'a, 'i>(root: Node<'a, 'i>) -> Vec<Node<'a, 'i>> {
    let decks: Vec<Node> = children(root, "Deck").collect();
    let with_clips: Vec<Node> = decks.iter().copied().filter(|d| has_clips(*d)).collect();
    if !with_clips.is_empty() {
        return with_clips;
    }
    let current = attr_usize(root, "currentDeckIndex").unwrap_or(0);
    decks
        .iter()
        .find(|d| attr_usize(**d, "deckIndex") == Some(current))
        .or(decks.first())
        .copied()
        .into_iter()
        .collect()
}

fn has_clips(deck: Node) -> bool {
    deck.descendants().any(|n| n.has_tag_name("VideoSource"))
}

/// Columns up to the last one with a clip or a name. Resolume pads decks with empty columns.
fn deck_width(deck: Node) -> usize {
    let clips = children(deck, "Clip")
        .filter(|c| c.descendants().any(|n| n.has_tag_name("VideoSource")))
        .filter_map(|c| attr_usize(c, "columnIndex"));
    let named =
        deck_columns(deck).filter(|c| param_str(*c, "Name").is_some()).filter_map(|c| attr_usize(c, "columnIndex"));
    clips.chain(named).max().map_or(1, |c| c + 1)
}

/// Deck columns, without the per-group columns Resolume also stores there.
fn deck_columns<'a, 'i>(deck: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    children(deck, "Column").filter(|c| c.attribute("groupIndex").is_none())
}

/// The mixer chosen in a `<ChoosableMixer name=...>`, such as a layer's blend mode.
fn mixer<'a, 'i>(track: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    track
        .children()
        .find(|n| n.has_tag_name("ChoosableMixer") && n.attribute("name") == Some(name))?
        .children()
        .find(|n| n.has_tag_name("RenderPass"))
}

fn mixer_opacity(track: Node) -> Option<f64> {
    mixer(track, "Blend").and_then(|m| param_f64(m, "Opacity"))
}

fn blend_mode(resolume: &str) -> Option<BlendMode> {
    // "50 Add" and friends are Resolume's default blends: the same maths with the layer
    // faded to half at the crossfader's midpoint, which openVJ doesn't have.
    let n = normalise(resolume);
    let n = n.strip_suffix("50").or_else(|| n.strip_prefix("50")).unwrap_or(&n);
    Some(match n {
        "alpha" | "normal" => BlendMode::Normal,
        "add" => BlendMode::Add,
        "screen" => BlendMode::Screen,
        "multiply" => BlendMode::Multiply,
        "overlay" => BlendMode::Overlay,
        "difference" => BlendMode::Difference,
        "lighten" => BlendMode::Lighten,
        "darken" => BlendMode::Darken,
        "subtract" => BlendMode::Subtract,
        _ => return None,
    })
}

/// Resolume generators with a reasonably close openVJ generator.
fn generator(resolume: &str) -> Option<GeneratorKind> {
    let n = normalise(resolume);
    let n = n.strip_suffix("generator").unwrap_or(&n);
    Some(match n {
        "gradient" | "lineargradient" | "radialgradient" => GeneratorKind::Gradient,
        "checkered" | "checker" | "checkerboard" => GeneratorKind::Checker,
        "lines" | "bars" | "stripes" => GeneratorKind::Bars,
        "strobe" => GeneratorKind::Strobe,
        "noise" | "perlinnoise" | "staticnoise" => GeneratorKind::Noise,
        "plasma" => GeneratorKind::Plasma,
        "tunnel" => GeneratorKind::Tunnel,
        "circles" | "rings" => GeneratorKind::Rings,
        _ => return None,
    })
}

// ---------------------------------------------------------------- effects

/// One effect in a Resolume render-pass chain, unwrapped from its dry/wet container.
struct RawEffect<'a, 'i> {
    node: Node<'a, 'i>,
    mix: f64,
    enabled: bool,
}

impl RawEffect<'_, '_> {
    fn ty(&self) -> &str {
        self.node.attribute("type").unwrap_or("")
    }

    fn display_name(&self) -> &str {
        self.node.attribute("name").unwrap_or_else(|| self.ty())
    }

    /// Every Resolume layer, clip and composition carries a fixed Transform. Untouched, it's
    /// a no-op and is not worth importing or reporting.
    fn is_default_transform(&self) -> bool {
        self.ty() == "TransformEffect"
            && self.mix >= 1.0
            && param_nodes(self.node).all(|p| p.attribute("value") == p.attribute("default"))
    }
}

fn chain<'a, 'i>(track: Node<'a, 'i>) -> impl Iterator<Item = RawEffect<'a, 'i>> {
    track
        .children()
        .find(|n| n.has_tag_name("RenderPass") && n.attribute("name") == Some("RenderPassChain"))
        .into_iter()
        .flat_map(|c| c.children().filter(|n| n.has_tag_name("RenderPass")))
        .map(|wrapper| {
            let enabled = !param_bool(wrapper, "Bypassed").unwrap_or(false);
            if wrapper.attribute("type") == Some("DryWetEffect") {
                let inner = wrapper
                    .children()
                    .find(|n| n.has_tag_name("RenderPass") && n.attribute("dwType").is_some())
                    .or_else(|| wrapper.children().find(|n| n.has_tag_name("RenderPass")))
                    .unwrap_or(wrapper);
                let mix = param_f64(wrapper, "Opacity")
                    .or_else(|| mixer(wrapper, "Mixer").and_then(|m| param_f64(m, "Opacity")));
                RawEffect { node: inner, mix: mix.unwrap_or(1.0), enabled }
            } else {
                RawEffect { node: wrapper, mix: 1.0, enabled }
            }
        })
}

/// Imports a track's effect chain. `imported` is called with each imported effect's path
/// name (see [`effect_path_names`]) and its index in the returned chain. When `transform`
/// is given, Resolume's Transform is imported into it instead of into the chain.
fn effect_chain(
    track: Node,
    width: u32,
    height: u32,
    label: &str,
    mut transform: Option<&mut Effect>,
    report: &mut ImportReport,
    imported: &mut dyn FnMut(String, usize),
) -> Vec<Effect> {
    let mut out = Vec::new();
    let (mut approximated, mut dropped) = (Vec::new(), Vec::new());
    let raws: Vec<RawEffect> = chain(track).collect();
    let names = effect_path_names(raws.iter().map(|r| r.display_name()));
    for (raw, path_name) in raws.into_iter().zip(names) {
        if raw.is_default_transform() {
            continue;
        }
        if raw.ty() == "TransformEffect"
            && let Some(fixed) = transform.take()
        {
            *fixed = default_transform();
            fixed.enabled = raw.enabled;
            note_unmapped(label, "Transform", import_settings(fixed, raw.node), report);
            continue;
        }
        let Some(mut effect) = map_effect(&raw, width, height, label, report) else {
            dropped.push(raw.display_name().to_owned());
            continue;
        };
        if effect.kind != "transform" {
            approximated.push(format!("{} as {}", raw.display_name(), effect.name()));
        }
        effect.enabled = raw.enabled;
        effect.mix = Param::new(raw.mix.clamp(0.0, 1.0) as f32);
        imported(path_name, out.len());
        out.push(effect);
    }
    if !dropped.is_empty() {
        report.note(format!("{label}: no openVJ equivalent for {}, dropped", dropped.join(", ")));
    }
    if !approximated.is_empty() {
        report.note(format!("{label}: imported {}, check their settings", approximated.join(", ")));
    }
    out
}

fn map_effect(raw: &RawEffect, width: u32, height: u32, label: &str, report: &mut ImportReport) -> Option<Effect> {
    let node = raw.node;
    if raw.ty() == "TransformEffect" {
        return Some(transform(node, width, height, label, report));
    }
    let lib = library();
    let ty = lib.for_resolume(raw.ty()).or_else(|| lib.for_resolume(raw.display_name()))?;
    let mut effect = Effect::new(ty.id());
    let name = effect.name();
    note_unmapped(label, &name, import_settings(&mut effect, node), report);
    Some(effect)
}

/// Copies the parameter values Resolume saved for an effect into the inputs that name them
/// (see `RESOLUME` in [`openvj_core::isf`]). Returns the changed parameters nothing maps.
fn import_settings(effect: &mut Effect, node: Node) -> Vec<String> {
    let Some(ty) = effect.ty() else { return Vec::new() };
    let mut unmapped = Vec::new();
    for p in param_nodes(node) {
        let Some(name) = p.attribute("name") else { continue };
        let Some(value) = p.attribute("value") else { continue };
        let channels: Vec<_> =
            ty.channels.iter().filter(|ch| ty.input(ch).resolume.as_ref().is_some_and(|r| r.name == name)).collect();
        if channels.is_empty() {
            if p.attribute("default") != Some(value) {
                unmapped.push(name.to_owned());
            }
            continue;
        }
        // Colours are saved as one ARGB integer; everything else as a number.
        let components: Vec<f32> = match (p.has_tag_name("ParamColor"), value.trim().parse::<f64>()) {
            (true, Ok(argb)) => {
                let c = argb as u32;
                [16, 8, 0, 24].iter().map(|shift| ((c >> shift) & 0xff) as f32 / 255.0).collect()
            }
            (false, Ok(v)) => vec![v as f32],
            _ => continue,
        };
        for ch in channels {
            let input = ty.input(ch);
            let v = components.get(ch.component).or(components.first()).copied().unwrap_or(0.0);
            if let Some(norm) = input.from_resolume(ch.component, v)
                && let Some(param) = effect.param_mut(&ch.key)
            {
                param.value = norm;
            }
        }
    }
    unmapped
}

fn note_unmapped(label: &str, effect: &str, unmapped: Vec<String>, report: &mut ImportReport) {
    if !unmapped.is_empty() {
        report.note(format!("{label}: {effect} settings openVJ doesn't have were left out ({})", unmapped.join(", ")));
    }
}

/// Resolume's transform maps to openVJ's: scale (percent) to zoom, Z rotation (degrees) to
/// rotate, and position (pixels) to the X/Y offset. Anchor and 3D rotation are dropped.
fn transform(node: Node, width: u32, height: u32, label: &str, report: &mut ImportReport) -> Effect {
    let mut e = Effect::new("transform");
    let get = |name| param_f64(node, name);
    let mut set = |key: &str, v: f64| {
        if let Some(p) = e.param_mut(key) {
            p.value = v as f32;
        }
    };
    let scale = get("Scale").unwrap_or(100.0).max(0.01) / 100.0;
    // openVJ zooms by 2^((zoom - 0.5) * 4) in normalised terms.
    set("zoom", (0.5 + scale.log2() / 4.0).clamp(0.0, 1.0));
    let rotation = get("Rotation Z").or_else(|| get("Rotation")).unwrap_or(0.0);
    set("rotate", (0.5 + rotation / 360.0).rem_euclid(1.0));
    set("x", (0.5 + get("Position X").unwrap_or(0.0) / width as f64).clamp(0.0, 1.0));
    // Resolume's Y axis points up, openVJ's texture coordinates point down.
    set("y", (0.5 - get("Position Y").unwrap_or(0.0) / height as f64).clamp(0.0, 1.0));
    let dropped: Vec<&str> = param_nodes(node)
        .filter(|p| p.attribute("value") != p.attribute("default"))
        .filter_map(|p| p.attribute("name"))
        .filter(|n| {
            n.starts_with("Anchor") || matches!(*n, "Rotation X" | "Rotation Y" | "Position Z" | "Scale W" | "Scale H")
        })
        .collect();
    if !dropped.is_empty() {
        report.note(format!("{label}: transform settings openVJ doesn't have were dropped ({})", dropped.join(", ")));
    }
    e
}

/// The names Resolume uses for effects in parameter paths: the display name in lower case,
/// with a number from the second of a kind on ("blur", "blur2", "blur3").
fn effect_path_names<'a>(display_names: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    display_names
        .map(|name| {
            let base = normalise(name);
            let n = seen.entry(base.clone()).or_default();
            *n += 1;
            if *n == 1 { base } else { format!("{base}{n}") }
        })
        .collect()
}

// ---------------------------------------------------------------- XML helpers

fn normalise(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_lowercase()).collect()
}

fn child<'a, 'i>(node: Node<'a, 'i>, tag: &str) -> Option<Node<'a, 'i>> {
    node.children().find(|n| n.has_tag_name(tag))
}

fn children<'a, 'i>(node: Node<'a, 'i>, tag: &'static str) -> impl Iterator<Item = Node<'a, 'i>> {
    node.children().filter(move |n| n.has_tag_name(tag))
}

fn attr_usize(node: Node, attr: &str) -> Option<usize> {
    node.attribute(attr)?.trim().parse().ok()
}

/// The direct parameters of an element: the children of its `<Params>`.
fn param_nodes<'a, 'i>(node: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    child(node, "Params").into_iter().flat_map(|p| p.children().filter(|n| n.is_element()))
}

fn param<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    param_nodes(node).find(|p| p.attribute("name") == Some(name))
}

/// A parameter's value, or its default when only the default was saved.
fn param_str<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    let p = param(node, name)?;
    p.attribute("value").or_else(|| p.attribute("default"))
}

fn param_f64(node: Node, name: &str) -> Option<f64> {
    param_str(node, name)?.trim().parse().ok()
}

fn param_bool(node: Node, name: &str) -> Option<bool> {
    param_str(node, name).map(|v| v.trim() == "1" || v.eq_ignore_ascii_case("true"))
}

/// A numeric parameter anywhere below `node`, for values Resolume nests (such as clip speed).
fn deep_param_f64(node: Node, name: &str) -> Option<f64> {
    node.descendants()
        .filter(|n| n.tag_name().name().starts_with("Param") && n.attribute("name") == Some(name))
        .find_map(|p| p.attribute("value").or_else(|| p.attribute("default"))?.trim().parse().ok())
}

#[cfg(test)]
mod tests;
