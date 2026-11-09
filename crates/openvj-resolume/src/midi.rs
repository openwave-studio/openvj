//! Imports Resolume MIDI shortcut presets (`Shortcuts/MIDI/<name>.xml`) as an openVJ
//! [`MidiMap`].
//!
//! A preset is a list of `<Shortcut>`s. Each names a parameter by path, such as
//! `/composition/layers/2/clips/3/connect`, and the MIDI message that drives it, packed
//! into `RawInputMessage key`: the status byte in bits 0–7, the note or controller number in
//! bits 8–15, a hash of the device name above that, and the message type (1 note, 2 CC) in
//! the top byte. Paths are resolved against the composition the preset is applied to; with
//! [`ResolumeRefs`] from an `.avc` import, effect names and layer ids resolve exactly.
//!
//! The `behaviour` flags aren't documented. From presets saved by Arena 6 and 7: 8 is an
//! absolute fader, 4 a button that toggles, 4|1024 a button that sets or launches on press,
//! and 2048 a button that is on only while held. Real Arena 7.15–7.26 presets also
//! use 1032 (a CC button that triggers) and 131082 (a blend-mode fader), which fit that reading.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use openvj_core::{
    Composition, EffectRef, LayerRef, MidiBehaviour, MidiControl, MidiKind, MidiMap, MidiMapping, MidiTarget,
};
use roxmltree::{Document, Node};

use crate::{ResolumeRefs, blend_mode, normalise, plural};
use openvj_core::{EffectType, library};

/// A preset read from a Resolume `.xml` file, plus what could not be carried over.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MidiImport {
    pub map: MidiMap,
    /// Shortcuts in the file.
    pub total: usize,
    /// One line per kind of shortcut that was dropped or approximated.
    pub notes: Vec<String>,
}

impl MidiImport {
    pub fn summary(&self) -> String {
        let mapped = self.map.mappings.len();
        let mut s = format!("{mapped} of {} MIDI shortcut{} mapped", self.total, plural(self.total));
        if let Some(device) = &self.map.device {
            s += &format!(" for {device}");
        }
        s
    }
}

/// Folders Resolume keeps MIDI presets in, for Arena and Avenue.
pub fn midi_preset_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    let Some(docs) = home.map(|h| h.join("Documents")) else { return Vec::new() };
    ["Resolume Arena", "Resolume Avenue"].iter().map(|app| docs.join(app).join("Shortcuts").join("MIDI")).collect()
}

/// Looks for a preset called `name` in `dirs` (and their `Shortcuts/MIDI` subfolders): first
/// by file name, then by the name saved inside each file.
pub fn find_midi_preset(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let dirs: Vec<PathBuf> = dirs.iter().flat_map(|d| [d.clone(), d.join("Shortcuts").join("MIDI")]).collect();
    let by_file = dirs.iter().map(|d| d.join(format!("{name}.xml"))).find(|p| p.is_file());
    by_file.or_else(|| {
        dirs.iter()
            .filter_map(|d| std::fs::read_dir(d).ok())
            .flatten()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("xml")))
            .find(|p| preset_name(p).as_deref() == Some(name))
    })
}

fn preset_name(path: &Path) -> Option<String> {
    let xml = std::fs::read_to_string(path).ok()?;
    let doc = Document::parse(&xml).ok()?;
    let root = doc.root_element();
    (root.tag_name().name() == "MidiShortcutPreset").then(|| root.attribute("name").map(str::to_owned))?
}

pub fn import_midi_preset_file(
    path: &Path,
    comp: &Composition,
    refs: Option<&ResolumeRefs>,
) -> Result<MidiImport, String> {
    let xml = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut imported = import_midi_preset(&xml, comp, refs)?;
    if imported.map.name.is_empty()
        && let Some(stem) = path.file_stem()
    {
        imported.map.name = stem.to_string_lossy().into_owned();
    }
    Ok(imported)
}

/// Parses the text of a MIDI shortcut preset and resolves it against `comp`.
pub fn import_midi_preset(xml: &str, comp: &Composition, refs: Option<&ResolumeRefs>) -> Result<MidiImport, String> {
    let doc = Document::parse(xml).map_err(|e| format!("not valid XML: {e}"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "MidiShortcutPreset" {
        return Err("not a Resolume MIDI preset (the root element isn't <MidiShortcutPreset>)".into());
    }
    let mut out = MidiImport::default();
    out.map.name = root.attribute("name").unwrap_or_default().to_owned();
    let resolver = Resolver { comp, refs };
    let mut skipped: BTreeMap<String, usize> = BTreeMap::new();
    let mut devices: BTreeMap<&str, usize> = BTreeMap::new();
    let mut approx_params = false;

    for sc in root.descendants().filter(|n| n.has_tag_name("Shortcut")) {
        out.total += 1;
        if let Some(d) = sc.attribute("inputDeviceName").filter(|d| !d.is_empty()) {
            *devices.entry(d).or_default() += 1;
        }
        let path = sc
            .children()
            .find(|n| n.has_tag_name("ShortcutPath") && n.attribute("name") == Some("InputPath"))
            .and_then(|n| n.attribute("path"))
            .unwrap_or_default();
        let Some(control) = sc.children().find(|n| n.has_tag_name("RawInputMessage")).and_then(control) else {
            *skipped.entry("MIDI messages other than notes and CCs".into()).or_default() += 1;
            continue;
        };
        let target = match resolver.target(path, sc) {
            Ok(t) => t,
            Err(why) => {
                *skipped.entry(why).or_default() += 1;
                continue;
            }
        };
        if matches!(target, MidiTarget::EffectParam { .. }) {
            approx_params = true;
        }
        let bits: u32 = sc.attribute("behaviour").and_then(|b| b.parse().ok()).unwrap_or(0);
        let mut mapping = MidiMapping::new(control, target, behaviour(bits));
        mapping.device = sc.attribute("inputDeviceName").filter(|d| !d.is_empty()).map(str::to_owned);
        if let Some(range) = sc.children().find(|n| n.has_tag_name("ValueRange")) {
            let get = |a| range.attribute(a).and_then(|v| v.parse::<f32>().ok());
            mapping.min = get("min").unwrap_or(0.0).clamp(0.0, 1.0);
            mapping.max = get("max").unwrap_or(1.0).clamp(0.0, 1.0);
        }
        out.map.mappings.push(mapping);
    }

    out.map.device = devices.into_iter().max_by_key(|(_, n)| *n).map(|(d, _)| d.to_owned());
    for (why, n) in skipped {
        out.notes.push(format!("{n} shortcut{} skipped: {why}", plural(n)));
    }
    if approx_params {
        out.notes.push("Effect parameters map onto openVJ's closest parameter; their ranges may differ".into());
    }
    Ok(out)
}

/// Decodes `<RawInputMessage key=...>` into a note or CC.
fn control(node: Node) -> Option<MidiControl> {
    let key: u64 = node.attribute("key")?.trim().parse().ok()?;
    let status = (key & 0xff) as u8;
    let number = ((key >> 8) & 0x7f) as u8;
    let kind = match (status & 0xf0, key >> 56) {
        (0x80 | 0x90, _) => MidiKind::Note,
        (0xb0, _) => MidiKind::Cc,
        (_, 1) => MidiKind::Note,
        (_, 2) => MidiKind::Cc,
        _ => return None,
    };
    Some(MidiControl { kind, channel: status & 0x0f, number })
}

fn behaviour(bits: u32) -> MidiBehaviour {
    if bits & 8 != 0 {
        MidiBehaviour::Fader
    } else if bits & 2048 != 0 {
        MidiBehaviour::Hold
    } else if bits & 1024 != 0 {
        MidiBehaviour::Press
    } else {
        MidiBehaviour::Toggle
    }
}

struct Resolver<'a> {
    comp: &'a Composition,
    refs: Option<&'a ResolumeRefs>,
}

type Resolved = Result<MidiTarget, String>;

impl Resolver<'_> {
    fn target(&self, path: &str, shortcut: Node) -> Resolved {
        let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
        let unsupported = || Err(format!("no openVJ equivalent for {}", generic(path)));
        let Some(("composition", rest)) = parts.split_first().map(|(a, b)| (*a, b)) else {
            return unsupported();
        };
        match rest {
            ["master"] | ["video", "opacity"] => Ok(MidiTarget::Master),
            ["disconnectall"] => Ok(MidiTarget::ClearAll),
            ["tempocontroller", "tempotap"] => Ok(MidiTarget::TempoTap),
            ["tempocontroller", "resync"] => Ok(MidiTarget::TempoResync),
            ["tempocontroller", "tempo"] => Ok(MidiTarget::Tempo),
            ["columns", n, "connect"] => Ok(MidiTarget::ColumnLaunch { column: self.column(n)? }),
            ["connectnextcolumn"] => Ok(MidiTarget::ColumnStep { forward: true }),
            ["connectprevcolumn" | "connectpreviouscolumn"] => Ok(MidiTarget::ColumnStep { forward: false }),
            ["dashboard", ..] | ["layers", _, "dashboard", ..] => {
                Err("Resolume dashboard knobs aren't supported yet".into())
            }
            ["video", "effects", fx, tail @ ..] => self.effect(None, fx, tail, path),
            ["layers", n, tail @ ..] => self.layer(LayerRef::Index(index(n)?), tail, path, shortcut),
            ["selectedlayer", tail @ ..] => self.layer(LayerRef::Selected, tail, path, shortcut),
            ["objects", id, tail @ ..] => {
                let layer =
                    id.parse::<u64>().ok().and_then(|id| self.refs?.layer_ids.get(&id).copied()).ok_or_else(|| {
                        if self.refs.is_some() {
                            "they address a layer group or a layer that isn't in the .avc".to_owned()
                        } else {
                            "they address layers by id, which only resolves when the preset comes with its .avc"
                                .to_owned()
                        }
                    })?;
                self.layer(LayerRef::Index(layer), tail, path, shortcut)
            }
            ["selectnextdeck"] => Ok(MidiTarget::DeckStep { forward: true }),
            ["selectprevdeck" | "selectpreviousdeck"] => Ok(MidiTarget::DeckStep { forward: false }),
            ["decks", n, "select"] => Ok(MidiTarget::DeckSelect { deck: index(n)? }),
            _ => unsupported(),
        }
    }

    fn layer(&self, layer: LayerRef, tail: &[&str], path: &str, shortcut: Node) -> Resolved {
        Ok(match tail {
            ["master"] | ["video", "opacity"] => MidiTarget::LayerOpacity { layer },
            ["bypassed"] => MidiTarget::LayerBypass { layer },
            ["solo"] => MidiTarget::LayerSolo { layer },
            ["select"] => MidiTarget::LayerSelect { layer },
            ["clear"] => MidiTarget::LayerClear { layer },
            ["connectnextclip"] => MidiTarget::LayerNextClip { layer },
            ["connectprevclip" | "connectpreviousclip"] => MidiTarget::LayerPrevClip { layer },
            ["video", "mixer", "blendmode"] => {
                let option =
                    shortcut.children().find(|n| n.has_tag_name("Subtarget")).and_then(|s| s.attribute("name"));
                match option {
                    // A button for one mode.
                    Some(name) => {
                        let blend = blend_mode(name).ok_or_else(|| format!("blend mode \"{name}\" isn't available"))?;
                        MidiTarget::LayerBlend { layer, blend }
                    }
                    // A fader across all modes. Resolume has ~60, so positions won't line up.
                    None => MidiTarget::LayerBlendFader { layer },
                }
            }
            ["video", "effects", fx, tail @ ..] => return self.effect(Some(layer), fx, tail, path),
            ["clips", c, "connect"] => {
                let LayerRef::Index(layer) = layer else {
                    return Err(format!("no openVJ equivalent for {}", generic(path)));
                };
                MidiTarget::ClipLaunch { layer, column: self.column(c)? }
            }
            _ => return Err(format!("no openVJ equivalent for {}", generic(path))),
        })
    }

    fn effect(&self, layer: Option<LayerRef>, fx: &str, tail: &[&str], path: &str) -> Resolved {
        let index = self.effect_index(layer, fx)?;
        let effect = EffectRef { layer, index };
        Ok(match tail {
            ["opacity"] => MidiTarget::EffectMix { effect },
            ["bypassed"] => MidiTarget::EffectBypass { effect },
            ["effect", param] => {
                let kind = self.effect_kind_at(layer, index).ok_or("their effect isn't in the composition")?;
                let param = effect_param(&kind, param)
                    .ok_or_else(|| format!("{} has no parameter like \"{param}\"", kind.name()))?;
                MidiTarget::EffectParam { effect, param }
            }
            _ => return Err(format!("no openVJ equivalent for {}", generic(path))),
        })
    }

    /// Finds the effect a path names. With import refs that's exact; otherwise it is the nth
    /// effect of the matching kind in the chain ("blur2" is the second Blur).
    fn effect_index(&self, layer: Option<LayerRef>, fx: &str) -> Result<usize, String> {
        let layer_index = match layer {
            None => None,
            Some(LayerRef::Index(i)) => Some(i),
            Some(LayerRef::Selected) => {
                return Err("effects on the selected layer aren't supported yet".into());
            }
        };
        if let Some(refs) = self.refs {
            return refs
                .effects
                .get(&(layer_index, fx.to_owned()))
                .copied()
                .ok_or_else(|| format!("effect \"{fx}\" has no openVJ equivalent or isn't on that layer"));
        }
        let base = fx.trim_end_matches(|c: char| c.is_ascii_digit());
        let nth: usize = fx[base.len()..].parse().unwrap_or(1);
        let lib = library();
        let kind = lib
            .for_resolume(base)
            .or_else(|| lib.find(base))
            .ok_or_else(|| format!("no openVJ effect like \"{base}\""))?;
        let chain = match layer_index {
            None => &self.comp.effects,
            Some(l) => &self.comp.layers.get(l).ok_or("their layer isn't in the composition")?.effects,
        };
        chain
            .iter()
            .enumerate()
            .filter(|(_, e)| e.kind == kind.id())
            .nth(nth.saturating_sub(1))
            .map(|(i, _)| i)
            .ok_or_else(|| "their effect isn't in the composition".into())
    }

    fn effect_kind_at(&self, layer: Option<LayerRef>, index: usize) -> Option<Arc<EffectType>> {
        let chain = match layer {
            None => &self.comp.effects,
            Some(LayerRef::Index(l)) => &self.comp.layers.get(l)?.effects,
            Some(LayerRef::Selected) => return None,
        };
        chain.get(index).and_then(|e| e.ty())
    }

    fn column(&self, n: &str) -> Result<usize, String> {
        // Like Resolume, clip and column buttons address the open deck. Layers and columns
        // that don't exist yet are kept: they work once they're added.
        index(n)
    }
}

/// Resolume numbers from 1.
fn index(n: &str) -> Result<usize, String> {
    n.parse::<usize>().ok().and_then(|n| n.checked_sub(1)).ok_or_else(|| format!("bad index \"{n}\" in path"))
}

/// A path with its numbers replaced, so similar shortcuts group into one note.
fn generic(path: &str) -> String {
    path.split('/')
        .map(|p| if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) { "N" } else { p })
        .collect::<Vec<_>>()
        .join("/")
}

/// The openVJ parameter (channel index) closest to a Resolume effect parameter.
fn effect_param(kind: &EffectType, resolume: &str) -> Option<usize> {
    let n = normalise(resolume);
    if let Some(i) = kind.channels.iter().position(|c| normalise(&c.label) == n || normalise(&c.key) == n) {
        return Some(i);
    }
    let key = match (kind.id(), n.as_str()) {
        ("blur", "blur" | "amount" | "radius" | "size") => "radius",
        ("color", "huerotate" | "hue" | "rotate") => "hue",
        ("color", "sat") => "saturation",
        ("color", "bright" | "exposure" | "gain") => "brightness",
        ("transform", "scale") => "zoom",
        ("transform", "rotation" | "rotationz") => "rotate",
        ("transform", "positionx") => "x",
        ("transform", "positiony") => "y",
        ("kaleido", "slices" | "segments" | "sides") => "segments",
        ("kaleido", "rotation" | "angle") => "rotate",
        ("pixelate", "pixelsize" | "amount" | "pixels" | "blocksize") => "size",
        ("rgb_split", "distance" | "offset" | "shift") => "amount",
        ("wave", "strength" | "amplitude" | "distortion") => "amount",
        ("posterize", "steps" | "colors" | "amount") => "levels",
        ("edges", "amount" | "threshold") => "strength",
        ("mirror", "mode") => "mode",
        _ => return None,
    };
    kind.channels.iter().position(|c| c.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openvj_core::{BlendMode, ClipSource, Effect, GeneratorKind, MidiMessage};

    /// Shortcuts in the shape Arena 7.22 saves them (see the module docs for the key format).
    const PRESET: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<MidiShortcutPreset name="Circuit Tracks" presetId="1">
  <versionInfo name="Resolume Arena" majorVersion="7" minorVersion="22" microVersion="9" revision="1"/>
  <ShortcutManager name="MIDIShortcutManagerShortcuts">
    <Shortcut name="Shortcut" paramNodeName="ParamEvent" behaviour="1028" inputDeviceName="Circuit Tracks">
      <ShortcutPath name="InputPath" path="/composition/layers/2/clips/3/connect" translationType="1" allowedTranslationTypes="11"/>
      <RawInputMessage key="73408041227146128" value="0" numSteps="128"/>
    </Shortcut>
    <Shortcut name="Shortcut" paramNodeName="ParamRange" behaviour="8" inputDeviceName="Circuit Tracks">
      <ShortcutPath name="InputPath" path="/composition/layers/1/video/opacity" translationType="1" allowedTranslationTypes="11"/>
      <Subtarget type="5" optionIndex="-1"/>
      <ValueRange min="0.25" max="0.75"/>
      <RawInputMessage key="145465635265057717" value="0" numSteps="128"/>
    </Shortcut>
    <Shortcut name="Shortcut" paramNodeName="RangedParam[bool]" behaviour="4" inputDeviceName="Circuit Tracks">
      <ShortcutPath name="InputPath" path="/composition/layers/1/video/effects/blur2/bypassed" translationType="1" allowedTranslationTypes="11"/>
      <RawInputMessage key="73408041227125648" value="0" numSteps="128"/>
    </Shortcut>
    <Shortcut name="Shortcut" paramNodeName="ParamRange" behaviour="8" inputDeviceName="Circuit Tracks">
      <ShortcutPath name="InputPath" path="/composition/video/effects/colorize/effect/contrast" translationType="1" allowedTranslationTypes="1"/>
      <RawInputMessage key="145465635265057968" value="0" numSteps="128"/>
    </Shortcut>
    <Shortcut name="Shortcut" paramNodeName="ParamChoice[unsigned long long]" behaviour="131076" inputDeviceName="Circuit Tracks">
      <ShortcutPath name="InputPath" path="/composition/layers/2/video/mixer/blendmode" translationType="1" allowedTranslationTypes="11"/>
      <Subtarget type="4" optionIndex="24" numChoices="60" name="Screen"/>
      <RawInputMessage key="73408041227125904" value="0" numSteps="128"/>
    </Shortcut>
    <Shortcut name="Shortcut" paramNodeName="RangedParam[bool]" behaviour="3076" inputDeviceName="Circuit Tracks">
      <ShortcutPath name="InputPath" path="/composition/selectedlayer/solo" translationType="1" allowedTranslationTypes="1"/>
      <RawInputMessage key="73408041227126160" value="0" numSteps="128"/>
    </Shortcut>
    <Shortcut name="Shortcut" paramNodeName="ParamEvent" behaviour="1028" inputDeviceName="Circuit Tracks">
      <ShortcutPath name="InputPath" path="/composition/tempocontroller/tempotap" translationType="1" allowedTranslationTypes="1"/>
      <RawInputMessage key="72057594037932176" value="0" numSteps="128"/>
    </Shortcut>
    <Shortcut name="Shortcut" paramNodeName="ParamRange" behaviour="8" inputDeviceName="Circuit Tracks">
      <ShortcutPath name="InputPath" path="/composition/layers/1/clips/1/transport/position/behaviour/speed" translationType="1" allowedTranslationTypes="11"/>
      <RawInputMessage key="145465635265058229" value="0" numSteps="128"/>
    </Shortcut>
    <Shortcut name="Shortcut" paramNodeName="ParamRange" behaviour="8" inputDeviceName="Launch Control">
      <ShortcutPath name="InputPath" path="/composition/layers/9/video/opacity" translationType="1" allowedTranslationTypes="11"/>
      <RawInputMessage key="145465635265058485" value="0" numSteps="128"/>
    </Shortcut>
  </ShortcutManager>
</MidiShortcutPreset>
"#;

    fn comp() -> Composition {
        let mut c = Composition::new(64, 64, 2, 4);
        c.set_clip(1, 2, ClipSource::Generator { kind: GeneratorKind::ALL[0] });
        c.layers[0].effects = vec![Effect::new("blur"), Effect::new("invert"), Effect::new("blur")];
        c.effects.push(Effect::new("color"));
        c
    }

    #[test]
    fn decodes_raw_input_keys() {
        let key = |k: u64| control(Document::parse(&format!(r#"<R key="{k}"/>"#)).unwrap().root_element());
        // Note 82 on channel 1 (0x52 0x90), from an APC40 mkII preset.
        assert_eq!(key(0x104cc397d005290), Some(MidiControl { kind: MidiKind::Note, channel: 0, number: 0x52 }));
        // CC 7 on channel 6 (0x07 0xb5).
        assert_eq!(key(0x204cc397d0007b5), Some(MidiControl { kind: MidiKind::Cc, channel: 5, number: 7 }));
        // Avenue 6 leaves the device hash out.
        assert_eq!(key(72057594037929872), Some(MidiControl { kind: MidiKind::Note, channel: 0, number: 7 }));
        // Pitch bend isn't mapped.
        assert_eq!(key(0x03000000000000e0), None);
    }

    #[test]
    fn imports_shortcuts_it_has_targets_for() {
        let c = comp();
        let imp = import_midi_preset(PRESET, &c, None).unwrap();
        assert_eq!(imp.map.name, "Circuit Tracks");
        assert_eq!(imp.map.device.as_deref(), Some("Circuit Tracks"));
        assert_eq!(imp.total, 9);
        let targets: Vec<MidiTarget> = imp.map.mappings.iter().map(|m| m.target).collect();
        let l0 = LayerRef::Index(0);
        assert_eq!(
            targets,
            [
                MidiTarget::ClipLaunch { layer: 1, column: 2 },
                MidiTarget::LayerOpacity { layer: l0 },
                MidiTarget::EffectBypass { effect: EffectRef { layer: Some(l0), index: 2 } },
                MidiTarget::EffectParam { effect: EffectRef { layer: None, index: 0 }, param: 3 },
                MidiTarget::LayerBlend { layer: LayerRef::Index(1), blend: BlendMode::Screen },
                MidiTarget::LayerSolo { layer: LayerRef::Selected },
                MidiTarget::TempoTap,
                MidiTarget::LayerOpacity { layer: LayerRef::Index(8) },
            ]
        );
        let m = &imp.map.mappings;
        assert_eq!(m[0].behaviour, MidiBehaviour::Press);
        assert_eq!(m[1].behaviour, MidiBehaviour::Fader);
        assert_eq!((m[1].min, m[1].max), (0.25, 0.75));
        assert_eq!(m[2].behaviour, MidiBehaviour::Toggle);
        assert_eq!(m[5].behaviour, MidiBehaviour::Hold);
        assert_eq!(m[1].control, MidiControl { kind: MidiKind::Cc, channel: 5, number: 0x13 });
        assert!(
            imp.notes.iter().any(|n| n.contains("/composition/layers/N/clips/N/transport/position/behaviour/speed")),
            "{:?}",
            imp.notes
        );
        // Layer 9 doesn't exist yet, but the shortcut is kept for when it does.
        assert_eq!(imp.map.mappings[7].target, MidiTarget::LayerOpacity { layer: LayerRef::Index(8) });
        assert_eq!(imp.summary(), "8 of 9 MIDI shortcuts mapped for Circuit Tracks");
    }

    #[test]
    fn imported_map_drives_the_composition() {
        let mut c = comp();
        let map = import_midi_preset(PRESET, &c, None).unwrap().map;
        let send = |c: &mut Composition, bytes: &[u8]| {
            for a in map.handle(MidiMessage::parse(bytes).unwrap()) {
                openvj_core::apply_midi(c, a, 1.0, 0);
            }
        };
        send(&mut c, &[0x90, 0x53, 100]);
        assert_eq!(c.layers[1].playing_column(), Some(2));
        send(&mut c, &[0xb5, 0x13, 127]);
        assert!((c.layers[0].opacity - 0.75).abs() < 1e-6);
        send(&mut c, &[0x90, 0x03, 127]);
        assert!(!c.layers[0].effects[2].enabled);
        assert!(c.layers[0].effects[0].enabled);
    }

    #[test]
    fn import_refs_resolve_effects_layers_and_decks() {
        let c = comp();
        let mut refs = ResolumeRefs::default();
        // In the .avc, "blur2" became openVJ's first effect on layer 1.
        refs.effects.insert((Some(0), "blur2".into()), 0);
        refs.layer_ids.insert(1748026448470, 1);
        let xml = PRESET
            .replace("/composition/layers/2/clips/3/connect", "/composition/objects/1748026448470/clips/3/connect");
        let imp = import_midi_preset(&xml, &c, Some(&refs)).unwrap();
        assert_eq!(imp.map.mappings[0].target, MidiTarget::ClipLaunch { layer: 1, column: 2 });
        assert_eq!(
            imp.map.mappings[2].target,
            MidiTarget::EffectBypass { effect: EffectRef { layer: Some(LayerRef::Index(0)), index: 0 } }
        );
        // Colorize wasn't imported according to the refs, so its shortcut is dropped.
        assert!(!imp.map.mappings.iter().any(|m| matches!(m.target, MidiTarget::EffectParam { .. })));
    }

    #[test]
    fn maps_column_steps_blend_faders_and_per_shortcut_devices() {
        let shortcut = |path: &str, key: u64, device: &str| {
            format!(
                r#"<Shortcut behaviour="1032" inputDeviceName="{device}"><ShortcutPath name="InputPath" path="{path}"/><RawInputMessage key="{key}"/></Shortcut>"#
            )
        };
        let xml = format!(
            "<MidiShortcutPreset name=\"Mixed\">{}{}{}{}{}</MidiShortcutPreset>",
            shortcut("/composition/connectnextcolumn", 0x2000000000001b0, "Circuit Tracks MIDI"),
            shortcut("/composition/connectprevcolumn", 0x2000000000002b0, "Circuit Tracks MIDI"),
            shortcut("/composition/layers/1/video/mixer/blendmode", 0x2000000000003b0, "XONE:K2"),
            shortcut("/composition/dashboard/link1", 0x2000000000004b0, "XONE:K2"),
            shortcut("/composition/layers/2/dashboard/link3", 0x2000000000005b0, "XONE:K2"),
        );
        let imp = import_midi_preset(&xml, &comp(), None).unwrap();
        let m = &imp.map.mappings;
        assert_eq!(m[0].target, MidiTarget::ColumnStep { forward: true });
        assert_eq!(m[1].target, MidiTarget::ColumnStep { forward: false });
        assert_eq!(m[2].target, MidiTarget::LayerBlendFader { layer: LayerRef::Index(0) });
        assert_eq!(m[0].device.as_deref(), Some("Circuit Tracks MIDI"));
        assert_eq!(m[2].device.as_deref(), Some("XONE:K2"));
        assert_eq!(imp.notes, ["2 shortcuts skipped: Resolume dashboard knobs aren't supported yet"]);
    }

    #[test]
    fn rejects_other_xml() {
        assert!(import_midi_preset("<Composition/>", &comp(), None).is_err());
    }

    #[test]
    fn finds_presets_by_file_or_saved_name() {
        let dir = std::env::temp_dir().join(format!("openvj-midi-{}", std::process::id()));
        let sub = dir.join("Shortcuts").join("MIDI");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("ct.xml"), PRESET).unwrap();
        assert_eq!(find_midi_preset("Circuit Tracks", std::slice::from_ref(&dir)), Some(sub.join("ct.xml")));
        std::fs::write(sub.join("Circuit Tracks.xml"), PRESET).unwrap();
        assert_eq!(
            find_midi_preset("Circuit Tracks", std::slice::from_ref(&dir)),
            Some(sub.join("Circuit Tracks.xml"))
        );
        assert_eq!(find_midi_preset("Nope", std::slice::from_ref(&dir)), None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
