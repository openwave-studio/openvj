//! Video effects that can be stacked on layers and on the composition.
//!
//! Each effect is an ISF file (see [`crate::isf`]). The built-in ones live in
//! `crates/openvj-core/effects/` and are compiled into the binary; more are loaded from
//! effect folders (see [`crate::effect_folders`]). [`library`] returns the current set, which
//! is swapped as a whole when a folder changes.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Deserializer, Serialize};

use crate::isf::{Channel, EffectDef};
use crate::{ModInputs, Param};

/// Built-in effect files, in browser order.
const BUILTIN: &[(&str, &str)] = &[
    ("color", include_str!("../effects/color.fs")),
    ("invert", include_str!("../effects/invert.fs")),
    ("transform", include_str!("../effects/transform.fs")),
    ("mirror", include_str!("../effects/mirror.fs")),
    ("kaleido", include_str!("../effects/kaleido.fs")),
    ("pixelate", include_str!("../effects/pixelate.fs")),
    ("rgb_split", include_str!("../effects/rgb_split.fs")),
    ("wave", include_str!("../effects/wave.fs")),
    ("posterize", include_str!("../effects/posterize.fs")),
    ("blur", include_str!("../effects/blur.fs")),
    ("edges", include_str!("../effects/edges.fs")),
    ("acuarela", include_str!("../effects/acuarela.fs")),
];

/// Built-in effects that aren't listed in the Effects tab: they're fixed in a card.
const HIDDEN: &[(&str, &str)] = &[(TRANSFORM, include_str!("../effects/clip_transform.fs"))];

/// Id of the fixed Transform every clip and layer has.
pub const TRANSFORM: &str = "clip_transform";

/// A parsed effect plus its channel list, shared by every instance.
#[derive(Debug)]
pub struct EffectType {
    pub def: EffectDef,
    pub channels: Vec<Channel>,
    /// The file it was loaded from; `None` for built-ins.
    pub path: Option<PathBuf>,
    /// Unique for every load, so a changed file gets recompiled and its buffers reset.
    pub version: u64,
}

impl EffectType {
    pub fn new(def: EffectDef, path: Option<PathBuf>) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let channels = def.channels();
        Self { def, channels, path, version: NEXT.fetch_add(1, Ordering::Relaxed) }
    }

    pub fn id(&self) -> &str {
        &self.def.id
    }

    pub fn name(&self) -> &str {
        &self.def.name
    }

    /// The input a channel belongs to.
    pub fn input(&self, ch: &Channel) -> &crate::isf::InputDef {
        &self.def.inputs[ch.input]
    }

    /// An ISF generator: it has no `inputImage` and draws on its own, so it plays as a clip.
    pub fn is_generator(&self) -> bool {
        !self.def.inputs.iter().any(|i| i.name == "inputImage" && i.ty == crate::isf::InputType::Image)
    }
}

/// An effect file that couldn't be loaded.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectError {
    pub path: PathBuf,
    /// Display name (from the header if it parsed, else the file name).
    pub name: String,
    pub message: String,
}

/// Every effect openVJ knows: the built-ins first, then folder effects by name. ISF
/// generators from the same folders are kept apart, since they play as clips.
#[derive(Debug, Default)]
pub struct EffectLibrary {
    types: Vec<Arc<EffectType>>,
    generators: Vec<Arc<EffectType>>,
    /// Folder files that failed to load.
    pub errors: Vec<EffectError>,
    /// Changes every time the library is replaced.
    pub generation: u64,
}

impl EffectLibrary {
    /// The built-in effects only.
    pub fn builtin() -> Self {
        Self { types: builtin_types().to_vec(), generators: Vec::new(), errors: Vec::new(), generation: 0 }
    }

    /// Built-ins plus the given folder effects and errors.
    pub fn with(types: Vec<Arc<EffectType>>, errors: Vec<EffectError>) -> Self {
        let mut lib = Self::builtin();
        lib.types.extend(types);
        lib.errors = errors;
        lib
    }

    /// Adds ISF generators (files without an `inputImage`).
    pub fn with_generators(mut self, generators: Vec<Arc<EffectType>>) -> Self {
        self.generators = generators;
        self
    }

    /// The effects, in browser order. Generators aren't included.
    pub fn all(&self) -> &[Arc<EffectType>] {
        &self.types
    }

    /// The ISF generators, in browser order.
    pub fn generators(&self) -> &[Arc<EffectType>] {
        &self.generators
    }

    pub fn generator(&self, id: &str) -> Option<&Arc<EffectType>> {
        self.generators.iter().find(|t| t.def.id == id)
    }

    pub fn get(&self, id: &str) -> Option<&Arc<EffectType>> {
        self.types.iter().chain(hidden_types()).find(|t| t.def.id == id)
    }

    /// Looks an effect up by id or display name, ignoring case.
    pub fn find(&self, name: &str) -> Option<&Arc<EffectType>> {
        self.get(name).or_else(|| self.types.iter().find(|t| t.def.name.eq_ignore_ascii_case(name)))
    }

    /// The effect standing in for a Resolume effect type (or display name), if any.
    pub fn for_resolume(&self, resolume: &str) -> Option<&Arc<EffectType>> {
        let key = normalise(resolume);
        self.types.iter().find(|t| t.def.resolume.iter().any(|r| normalise(r) == key))
    }
}

fn normalise(s: &str) -> String {
    let n: String = s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
    n.strip_suffix("effect").map(str::to_owned).unwrap_or(n)
}

/// Ids of the built-in effects; folder files can't use them.
pub fn is_builtin(id: &str) -> bool {
    BUILTIN.iter().chain(HIDDEN).any(|(b, _)| *b == id)
}

/// The parsed built-ins. Panics at first use if a built-in file doesn't parse (a test checks them).
fn builtin_types() -> &'static [Arc<EffectType>] {
    static TYPES: std::sync::OnceLock<Vec<Arc<EffectType>>> = std::sync::OnceLock::new();
    TYPES.get_or_init(|| parse_builtins(BUILTIN))
}

fn hidden_types() -> &'static [Arc<EffectType>] {
    static TYPES: std::sync::OnceLock<Vec<Arc<EffectType>>> = std::sync::OnceLock::new();
    TYPES.get_or_init(|| parse_builtins(HIDDEN))
}

fn parse_builtins(files: &[(&str, &str)]) -> Vec<Arc<EffectType>> {
    files
        .iter()
        .map(|(id, src)| {
            let def = EffectDef::parse(src, id).unwrap_or_else(|e| panic!("built-in effect {id}: {e}"));
            Arc::new(EffectType::new(def, None))
        })
        .collect()
}

static LIBRARY: RwLock<Option<Arc<EffectLibrary>>> = RwLock::new(None);

/// The current effect library: the built-ins until [`set_library`] adds folder effects.
/// Cheap (a lock and a reference count); take one snapshot per frame or action.
pub fn library() -> Arc<EffectLibrary> {
    if let Some(lib) = LIBRARY.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
        return lib.clone();
    }
    let mut slot = LIBRARY.write().unwrap_or_else(|e| e.into_inner());
    slot.get_or_insert_with(|| Arc::new(EffectLibrary::builtin())).clone()
}

/// Replaces the effect library, e.g. after an effect folder changed.
pub fn set_library(mut lib: EffectLibrary) {
    let mut slot = LIBRARY.write().unwrap_or_else(|e| e.into_inner());
    lib.generation = slot.as_ref().map_or(0, |l| l.generation) + 1;
    *slot = Some(Arc::new(lib));
}

fn next_uid() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// One effect in a chain.
#[derive(Debug, Clone, Serialize)]
pub struct Effect {
    /// Id of its [`EffectType`].
    pub kind: String,
    pub enabled: bool,
    /// Normalised channel values by channel key. Missing keys use the default.
    pub params: BTreeMap<String, Param>,
    /// Dry/wet mix.
    pub mix: Param,
    /// Identifies this instance's GPU state (feedback buffers). Not saved.
    #[serde(skip)]
    pub uid: u64,
}

impl PartialEq for Effect {
    fn eq(&self, o: &Self) -> bool {
        self.kind == o.kind && self.enabled == o.enabled && self.params == o.params && self.mix == o.mix
    }
}

impl Effect {
    /// A new effect with default values. Unknown ids give an effect that passes its input through.
    pub fn new(kind: &str) -> Self {
        let mut fx = Self {
            kind: kind.to_owned(),
            enabled: true,
            params: BTreeMap::new(),
            mix: Param::new(1.0),
            uid: next_uid(),
        };
        if let Some(t) = fx.ty() {
            for ch in &t.channels {
                fx.params.insert(ch.key.clone(), Param::new(ch.default));
            }
        }
        fx
    }

    /// Its type, or `None` when the effect file isn't available.
    pub fn ty(&self) -> Option<Arc<EffectType>> {
        let lib = library();
        lib.get(&self.kind).or_else(|| lib.generator(&self.kind)).cloned()
    }

    /// Display name, falling back to the id for a missing effect.
    pub fn name(&self) -> String {
        self.ty().map(|t| t.name().to_owned()).unwrap_or_else(|| self.kind.clone())
    }

    pub fn param(&self, key: &str) -> Option<&Param> {
        self.params.get(key)
    }

    /// Mutable access to a channel, creating it from the default if needed.
    pub fn param_mut(&mut self, key: &str) -> Option<&mut Param> {
        let default = self.ty()?.channels.iter().find(|c| c.key == key)?.default;
        Some(self.params.entry(key.to_owned()).or_insert_with(|| Param::new(default)))
    }

    /// True when the effect can't change its input: bypassed, fully dry, or every channel at
    /// its default with nothing modulating it. Only meaningful for effects whose defaults
    /// are a no-op, like the Transform.
    pub fn is_noop(&self) -> bool {
        if !self.enabled || (self.mix.value <= 0.0 && self.mix.modulation.source == crate::ModSource::Off) {
            return true;
        }
        let Some(t) = self.ty() else { return true };
        t.channels.iter().all(|ch| {
            self.params
                .get(&ch.key)
                .is_none_or(|p| p.modulation.source == crate::ModSource::Off && (p.value - ch.default).abs() < 1e-5)
        })
    }

    /// Turns event inputs (momentary triggers) back off. Call once per frame after rendering.
    pub fn clear_events(&mut self) {
        let Some(t) = self.ty() else { return };
        for ch in &t.channels {
            if t.input(ch).ty == crate::isf::InputType::Event
                && let Some(p) = self.params.get_mut(&ch.key)
            {
                p.value = 0.0;
            }
        }
    }

    /// Modulated normalised value of every channel of its type, in channel order.
    pub fn values(&self, inputs: &ModInputs) -> Vec<f32> {
        let Some(t) = self.ty() else { return Vec::new() };
        t.channels
            .iter()
            .map(|ch| match self.params.get(&ch.key) {
                Some(p) if t.input(ch).wrap => p.eval_wrapped(inputs),
                Some(p) => p.eval(inputs),
                None => ch.default,
            })
            .collect()
    }
}

/// Moves an effect one step up (`-1`) or down (`+1`) in a chain.
pub fn move_effect(chain: &mut [Effect], index: usize, delta: i32) {
    let to = index as i64 + delta as i64;
    if index < chain.len() && (0..chain.len() as i64).contains(&to) {
        chain.swap(index, to as usize);
    }
}

// ---------------------------------------------------------------- loading

/// A 0.1 parameter slot: the channel key it became and how its value converts.
type LegacySlot = (&'static str, fn(f32) -> f32);

/// Channel keys of the 4 fixed parameter slots of openVJ 0.1 effects, and how an old
/// normalised value maps onto the new channel (most are unchanged).
fn legacy_slots(kind: &str) -> [Option<LegacySlot>; 4] {
    fn same(v: f32) -> f32 {
        v
    }
    fn flip(v: f32) -> f32 {
        1.0 - v
    }
    match kind {
        "color" => {
            [Some(("hue", same)), Some(("saturation", same)), Some(("brightness", same)), Some(("contrast", same))]
        }
        "transform" => [Some(("zoom", same)), Some(("rotate", same)), Some(("x", same)), Some(("y", same))],
        "mirror" => [Some(("mode", legacy_mirror_mode)), Some(("offset", same)), None, None],
        "kaleido" => [Some(("segments", same)), Some(("rotate", same)), Some(("zoom", same)), None],
        "pixelate" => [Some(("size", same)), None, None, None],
        "rgb_split" => [Some(("amount", same)), Some(("angle", same)), None, None],
        "wave" => [Some(("amount", same)), Some(("frequency", same)), Some(("speed", same)), Some(("direction", same))],
        "posterize" => [Some(("levels", flip)), None, None, None],
        "blur" => [Some(("radius", same)), None, None, None],
        "edges" => [Some(("strength", same)), Some(("keep_color", same)), None, None],
        _ => [None; 4],
    }
}

/// 0.1 split the mirror mode at thirds; the choice channel splits at quarters.
fn legacy_mirror_mode(v: f32) -> f32 {
    if v < 0.334 {
        0.0
    } else if v < 0.667 {
        0.5
    } else {
        1.0
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ParamsRepr {
    Map(BTreeMap<String, Param>),
    /// openVJ 0.1 saved four positional slots.
    List(Vec<Param>),
}

#[derive(Deserialize)]
struct EffectRepr {
    kind: String,
    #[serde(default = "yes")]
    enabled: bool,
    #[serde(default)]
    params: Option<ParamsRepr>,
    #[serde(default = "full")]
    mix: Param,
}

fn yes() -> bool {
    true
}

fn full() -> Param {
    Param::new(1.0)
}

impl<'de> Deserialize<'de> for Effect {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let r = EffectRepr::deserialize(d)?;
        let mut fx = Effect::new(&r.kind);
        fx.enabled = r.enabled;
        fx.mix = r.mix;
        match r.params {
            Some(ParamsRepr::Map(map)) => fx.params.extend(map),
            Some(ParamsRepr::List(list)) => {
                for (slot, p) in legacy_slots(&r.kind).into_iter().zip(list) {
                    if let Some((key, convert)) = slot {
                        fx.params.insert(key.to_owned(), Param { value: convert(p.value), ..p });
                    }
                }
            }
            None => {}
        }
        Ok(fx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isf::InputType;
    use crate::{ModSource, Modulation};

    #[test]
    fn builtins_parse_with_unique_ids() {
        let lib = EffectLibrary::builtin();
        assert_eq!(lib.all().len(), BUILTIN.len());
        for (i, t) in lib.all().iter().enumerate() {
            assert_eq!(t.id(), BUILTIN[i].0, "the header ID must match the file name");
            assert!(!t.def.description.is_empty(), "{} has no description", t.id());
            assert_eq!(lib.get(t.id()).unwrap().id(), t.id());
            assert!(t.def.extra_images().next().is_none());
        }
    }

    #[test]
    fn new_effects_get_defaults() {
        let fx = Effect::new("color");
        assert_eq!(fx.params.len(), 4);
        assert_eq!(fx.params["saturation"].value, 0.5);
        assert_eq!(fx.name(), "Color");
        let missing = Effect::new("nope");
        assert!(missing.ty().is_none() && missing.params.is_empty());
        assert_eq!(missing.name(), "nope");
    }

    #[test]
    fn values_apply_modulation_and_wrap() {
        let mut fx = Effect::new("pixelate");
        fx.params.get_mut("size").unwrap().modulation = Modulation { source: ModSource::Bass, amount: 0.5, beats: 1.0 };
        fx.params.get_mut("size").unwrap().value = 0.3;
        let v = fx.values(&ModInputs { beat: 0.0, bands: [1.0, 0.0, 0.0, 0.0], ..Default::default() });
        assert!((v[0] - 0.8).abs() < 1e-6);

        // Hue wraps instead of stopping at 360°.
        let mut c = Effect::new("color");
        let hue = c.params.get_mut("hue").unwrap();
        hue.value = 0.75;
        hue.modulation = Modulation { source: ModSource::Bass, amount: 0.5, beats: 1.0 };
        let v = c.values(&ModInputs { beat: 0.0, bands: [1.0, 0.0, 0.0, 0.0], ..Default::default() });
        assert!((v[0] - 0.25).abs() < 1e-6, "{v:?}");
    }

    #[test]
    fn json_round_trip_and_unknown_effects_survive() {
        let mut fx = Effect::new("wave");
        fx.params.get_mut("speed").unwrap().value = 0.9;
        fx.enabled = false;
        let back: Effect = serde_json::from_str(&serde_json::to_string(&fx).unwrap()).unwrap();
        assert_eq!(back, fx);
        assert_ne!(back.uid, fx.uid, "every loaded effect gets its own GPU state");

        let alien: Effect = serde_json::from_str(r#"{"kind":"future_fx","params":{"k":{"value":0.2}}}"#).unwrap();
        assert_eq!(alien.params["k"].value, 0.2);
        let again: Effect = serde_json::from_str(&serde_json::to_string(&alien).unwrap()).unwrap();
        assert_eq!(again, alien);
    }

    #[test]
    fn loads_openvj_0_1_effects() {
        let json = r#"{"kind":"posterize","enabled":true,"params":[{"value":0.25},{"value":0},{"value":0},{"value":0}],"mix":{"value":0.5}}"#;
        let fx: Effect = serde_json::from_str(json).unwrap();
        assert_eq!(fx.params["levels"].value, 0.75);
        assert_eq!(fx.mix.value, 0.5);
        let json =
            r#"{"kind":"mirror","params":[{"value":0.5},{"value":0.2},{"value":0},{"value":0}],"mix":{"value":1}}"#;
        let fx: Effect = serde_json::from_str(json).unwrap();
        let mode = &fx.ty().unwrap().def.inputs[1];
        assert!(matches!(mode.ty, InputType::Long { .. }));
        assert_eq!(mode.display(0, fx.params["mode"].value), "Vertical");
        assert_eq!(fx.params["offset"].value, 0.2);
    }

    #[test]
    fn every_legacy_slot_names_a_real_channel() {
        for t in EffectLibrary::builtin().all() {
            for (key, _) in legacy_slots(t.id()).into_iter().flatten() {
                assert!(t.channels.iter().any(|c| c.key == key), "{}: no channel {key}", t.id());
            }
        }
    }

    #[test]
    fn resolume_lookup() {
        let lib = library();
        assert_eq!(lib.for_resolume("FastBlur").unwrap().id(), "blur");
        assert_eq!(lib.for_resolume("EdgeDetectionEffect").unwrap().id(), "edges");
        assert_eq!(lib.for_resolume("Acuarela").unwrap().id(), "acuarela");
        assert!(lib.for_resolume("Bendoscope").is_none());
    }

    #[test]
    fn move_effect_stays_in_bounds() {
        let mut chain = vec![Effect::new("blur"), Effect::new("invert")];
        move_effect(&mut chain, 0, 1);
        assert_eq!(chain[0].kind, "invert");
        move_effect(&mut chain, 0, -1);
        assert_eq!(chain[0].kind, "invert");
        move_effect(&mut chain, 5, 1);
    }
}
