//! The library: every ISF generator and effect, MilkDrop preset, Shadertoy import and saved
//! preset in one list, with what the VJ thinks of each.
//!
//! What openVJ finds on disk ([`Item`]) is listed fresh on every scan; what the VJ decides
//! ([`Entry`]: status, favorite, intensity, tags, crates, usage) lives in one per-user
//! `library.json` ([`LibraryFile`]) keyed by item id, so bundled shader files are never
//! written to. An entry outlives its file, so tags come back if the file returns.
//!
//! Items start [`Status::NotReviewed`] and are sorted in the Discover layout; My library shows
//! only [`Status::Kept`] items. [`Status::Unwanted`] is a crate the VJ can restore from:
//! nothing here ever deletes a file.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::effect::{EffectLibrary, EffectType};
use crate::isf::EffectDef;
use crate::milkdrop::{MilkLibrary, MilkPreset};
use crate::{ClipSource, Composition, GeneratorKind, Preset};

/// File name of the library in openVJ's settings folder.
pub const LIBRARY_FILE: &str = "library.json";

/// Version written to [`LibraryFile::version`].
pub const LIBRARY_VERSION: u32 = 1;

/// Highest intensity: 0 is calm, 5 is peak.
pub const MAX_INTENSITY: u8 = 5;

/// Pack name of the sources and effects compiled into openVJ.
pub const BUILTIN_PACK: &str = "Built-in";

/// Pack name of Shadertoy imports.
pub const SHADERTOY_PACK: &str = "Shadertoy";

/// Pack name of MilkDrop presets at the top of a MilkDrop folder.
pub const MILKDROP_PACK: &str = "MilkDrop";

/// Pack name of saved presets.
pub const PRESETS_PACK: &str = "Presets";

/// File extension of a saved preset.
pub const PRESET_EXTENSION: &str = "ovjpreset";

/// Version written to [`PresetFile::version`].
pub const PRESET_VERSION: u32 = 1;

/// Largest number of bundled generators the first-launch starter set keeps.
pub const STARTER_GENERATORS: usize = 30;

/// What an item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Generator,
    Effect,
    MilkDrop,
    Preset,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Generator, Kind::Effect, Kind::MilkDrop, Kind::Preset];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Generator => "Generator",
            Kind::Effect => "Effect",
            Kind::MilkDrop => "MilkDrop",
            Kind::Preset => "Preset",
        }
    }

    /// Whether it plays in a clip slot (everything but effects).
    pub fn is_source(self) -> bool {
        self != Kind::Effect
    }
}

/// Whether the VJ has looked at an item yet, and what they decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// New: waits in Discover.
    #[default]
    NotReviewed,
    /// In My library.
    Kept,
    /// Hidden in the Unwanted crate; restorable, never deleted.
    Unwanted,
}

/// One thing the library lists. Rebuilt from disk on every scan; never saved.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// Stable id, the key in [`LibraryFile::items`]. See the `*_id` functions.
    pub id: String,
    pub kind: Kind,
    pub name: String,
    /// Where it comes from: the bundled pack, a folder, Shadertoy or [`BUILTIN_PACK`].
    pub pack: String,
    pub description: String,
    pub credit: String,
    pub categories: Vec<String>,
    /// Reads audio, spectrum, beat or tempo inputs (see [`isf_audio_reactive`]).
    pub audio_reactive: bool,
    /// Shipped with openVJ (built-ins and the bundled shader folder).
    pub bundled: bool,
    /// The file it was loaded from; `None` for built-ins.
    pub path: Option<PathBuf>,
}

/// Id of a built-in generator (`gen:plasma`).
pub fn builtin_generator_id(kind: GeneratorKind) -> String {
    format!("gen:{}", kind.name().to_lowercase())
}

/// Id of an ISF generator or effect, by its effect id (`isf:blur`).
pub fn isf_id(effect_id: &str) -> String {
    format!("isf:{effect_id}")
}

/// Id of a MilkDrop preset, by its id in the MilkDrop library (`milk:Geiss/Starfish.milk`).
pub fn milk_id(preset_id: &str) -> String {
    format!("milk:{preset_id}")
}

/// Id of a saved preset, by its path inside the presets folder (`preset:Tunnel peak.ovjpreset`).
pub fn preset_id(file_name: &str) -> String {
    format!("preset:{file_name}")
}

/// What the VJ decided about one item. Everything defaults to "nothing yet".
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Entry {
    #[serde(default, skip_serializing_if = "is_default")]
    pub status: Status,
    #[serde(default, skip_serializing_if = "is_default")]
    pub favorite: bool,
    /// 0 (calm) to [`MAX_INTENSITY`] (peak); `None` until the VJ sets it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intensity: Option<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Names of the crates it is in.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub crates: Vec<String>,
    /// How many times it was put in a slot or played.
    #[serde(default, skip_serializing_if = "is_default")]
    pub uses: u32,
    /// When it was last used, in seconds since the Unix epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used: Option<u64>,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

impl Entry {
    fn is_empty(&self) -> bool {
        *self == Entry::default()
    }
}

/// The per-user `library.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryFile {
    pub version: u32,
    /// The first-launch starter set was applied (see [`Library::apply_starter_set`]).
    #[serde(default)]
    pub starter_set: bool,
    /// Crate names in the order the VJ made them, including empty crates.
    #[serde(default)]
    pub crates: Vec<String>,
    /// By item id. Entries whose file is gone stay, so their tags come back with the file.
    #[serde(default)]
    pub items: BTreeMap<String, Entry>,
}

impl Default for LibraryFile {
    fn default() -> Self {
        Self { version: LIBRARY_VERSION, starter_set: false, crates: Vec::new(), items: BTreeMap::new() }
    }
}

impl LibraryFile {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let file: Self = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if file.version > LIBRARY_VERSION {
            return Err(format!(
                "the library was saved by a newer openVJ (version {}, this one reads {LIBRARY_VERSION})",
                file.version
            ));
        }
        Ok(file)
    }

    pub fn to_json(&self) -> String {
        let mut out = self.clone();
        out.version = LIBRARY_VERSION;
        out.items.retain(|_, e| !e.is_empty());
        serde_json::to_string_pretty(&out).expect("the library always serializes")
    }

    /// Reads `path`. A missing file is an empty library; an unreadable or newer one is an
    /// error, so the caller can leave it alone instead of overwriting the VJ's work.
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(s) => Self::from_json(&s).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// Writes `path` (through a temporary file, so a crash never leaves half a library).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, self.to_json())?;
        std::fs::rename(&tmp, path)
    }
}

/// Which part of the library to show.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum View {
    /// Kept items (and saved presets, and whatever the composition uses).
    #[default]
    MyLibrary,
    /// Not reviewed yet.
    Discover,
    Unwanted,
    /// Starred and not unwanted.
    Favorites,
    /// Used at least once, most recent first.
    Recent,
    /// In the named crate and not unwanted.
    Crate(String),
    /// Everything, unwanted included.
    All,
}

/// What [`Library::query`] shows. The default is My library, everything in it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Filter {
    pub view: View,
    /// Only these kinds; empty means all.
    pub kinds: Vec<Kind>,
    /// Only items whose intensity is in this range (unrated items are left out).
    pub intensity: Option<(u8, u8)>,
    pub audio_reactive: bool,
    /// Only this pack; empty means all.
    pub pack: String,
    /// Only items with this category; empty means all.
    pub category: String,
    /// Words that must all appear (any case) in the name, description, credit, pack,
    /// categories or tags.
    pub search: String,
}

/// The items found on disk together with the VJ's library file.
#[derive(Debug, Clone, Default)]
pub struct Library {
    items: Vec<Item>,
    /// Index into `items` by id.
    by_id: BTreeMap<String, usize>,
    pub file: LibraryFile,
    /// Ids the open composition uses; they count as kept.
    in_use: HashSet<String>,
}

impl Library {
    pub fn new(mut items: Vec<Item>, file: LibraryFile) -> Self {
        items.sort_by_cached_key(|i| (i.name.to_lowercase(), i.id.clone()));
        items.dedup_by(|a, b| a.id == b.id);
        let by_id = items.iter().enumerate().map(|(i, it)| (it.id.clone(), i)).collect();
        Self { items, by_id, file, in_use: HashSet::new() }
    }

    /// Replaces the items after a rescan, keeping the file and what is in use.
    pub fn set_items(&mut self, items: Vec<Item>) {
        let file = std::mem::take(&mut self.file);
        let in_use = std::mem::take(&mut self.in_use);
        *self = Self::new(items, file);
        self.in_use = in_use;
    }

    /// Every item, sorted by name.
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn get(&self, id: &str) -> Option<&Item> {
        self.by_id.get(id).map(|&i| &self.items[i])
    }

    /// Ids with an entry in the file but no file on disk now.
    pub fn missing(&self) -> impl Iterator<Item = &str> {
        self.file.items.keys().filter(|id| !self.by_id.contains_key(*id)).map(String::as_str)
    }

    /// Tells the library what the open composition uses (see [`composition_ids`]).
    pub fn set_in_use(&mut self, ids: HashSet<String>) {
        self.in_use = ids;
    }

    /// The VJ's entry for an item (empty if they never touched it).
    pub fn entry(&self, id: &str) -> Entry {
        self.file.items.get(id).cloned().unwrap_or_default()
    }

    fn entry_mut(&mut self, id: &str) -> &mut Entry {
        self.file.items.entry(id.to_owned()).or_default()
    }

    /// The status the VJ sees: anything the composition uses counts as kept, and a saved
    /// preset is kept unless the VJ marked it unwanted.
    pub fn status(&self, id: &str) -> Status {
        if self.in_use.contains(id) {
            return Status::Kept;
        }
        match self.file.items.get(id).map(|e| e.status).unwrap_or_default() {
            Status::NotReviewed if self.get(id).is_some_and(|i| i.kind == Kind::Preset) => Status::Kept,
            s => s,
        }
    }

    pub fn set_status(&mut self, id: &str, status: Status) {
        self.entry_mut(id).status = status;
    }

    pub fn set_favorite(&mut self, id: &str, favorite: bool) {
        self.entry_mut(id).favorite = favorite;
    }

    pub fn toggle_favorite(&mut self, id: &str) {
        let e = self.entry_mut(id);
        e.favorite = !e.favorite;
    }

    /// Sets the intensity (clamped to [`MAX_INTENSITY`]), or clears it with `None`.
    pub fn set_intensity(&mut self, id: &str, intensity: Option<u8>) {
        self.entry_mut(id).intensity = intensity.map(|v| v.min(MAX_INTENSITY));
    }

    /// Adds a tag (trimmed; ignored if empty or already there, any case).
    pub fn add_tag(&mut self, id: &str, tag: &str) {
        let tag = tag.trim();
        let e = self.entry_mut(id);
        if !tag.is_empty() && !e.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
            e.tags.push(tag.to_owned());
        }
    }

    pub fn remove_tag(&mut self, id: &str, tag: &str) {
        self.entry_mut(id).tags.retain(|t| !t.eq_ignore_ascii_case(tag));
    }

    /// Every tag in use, sorted, each once.
    pub fn tags(&self) -> Vec<String> {
        let mut seen = BTreeMap::new();
        for t in self.file.items.values().flat_map(|e| &e.tags) {
            seen.entry(t.to_lowercase()).or_insert_with(|| t.clone());
        }
        seen.into_values().collect()
    }

    /// The crates, in the order the VJ made them.
    pub fn crates(&self) -> &[String] {
        &self.file.crates
    }

    /// Makes a crate (trimmed; nothing happens if it exists or the name is empty).
    pub fn add_crate(&mut self, name: &str) {
        let name = name.trim();
        if !name.is_empty() && !self.file.crates.iter().any(|c| c == name) {
            self.file.crates.push(name.to_owned());
        }
    }

    /// Renames a crate everywhere. Fails if the new name is empty or taken.
    pub fn rename_crate(&mut self, from: &str, to: &str) -> bool {
        let to = to.trim();
        if to.is_empty() || self.file.crates.iter().any(|c| c == to) {
            return false;
        }
        let Some(slot) = self.file.crates.iter_mut().find(|c| *c == from) else { return false };
        *slot = to.to_owned();
        for e in self.file.items.values_mut() {
            for c in e.crates.iter_mut().filter(|c| *c == from) {
                *c = to.to_owned();
            }
        }
        true
    }

    /// Removes a crate. The items in it stay in the library.
    pub fn remove_crate(&mut self, name: &str) {
        self.file.crates.retain(|c| c != name);
        for e in self.file.items.values_mut() {
            e.crates.retain(|c| c != name);
        }
    }

    /// Puts an item in a crate, making the crate if needed.
    pub fn add_to_crate(&mut self, id: &str, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        self.add_crate(name);
        let e = self.entry_mut(id);
        if !e.crates.iter().any(|c| c == name) {
            e.crates.push(name.to_owned());
        }
    }

    pub fn remove_from_crate(&mut self, id: &str, name: &str) {
        self.entry_mut(id).crates.retain(|c| c != name);
    }

    /// Counts a use (put in a slot or launched) at `now`, seconds since the Unix epoch.
    pub fn record_use(&mut self, id: &str, now: u64) {
        let e = self.entry_mut(id);
        e.uses = e.uses.saturating_add(1);
        e.last_used = Some(now);
    }

    /// The items `filter` shows: by name, or most recent first for [`View::Recent`].
    pub fn query(&self, filter: &Filter) -> Vec<&Item> {
        let words: Vec<String> = filter.search.split_whitespace().map(str::to_lowercase).collect();
        let mut out: Vec<&Item> = self.items.iter().filter(|item| self.shows(item, filter, &words)).collect();
        if filter.view == View::Recent {
            out.sort_by_key(|i| std::cmp::Reverse(self.file.items.get(&i.id).and_then(|e| e.last_used)));
        }
        out
    }

    fn shows(&self, item: &Item, filter: &Filter, words: &[String]) -> bool {
        let status = self.status(&item.id);
        let entry = self.file.items.get(&item.id);
        let in_view = match &filter.view {
            View::MyLibrary => status == Status::Kept,
            View::Discover => status == Status::NotReviewed,
            View::Unwanted => status == Status::Unwanted,
            View::Favorites => status != Status::Unwanted && entry.is_some_and(|e| e.favorite),
            View::Recent => status != Status::Unwanted && entry.is_some_and(|e| e.last_used.is_some()),
            View::Crate(name) => status != Status::Unwanted && entry.is_some_and(|e| e.crates.contains(name)),
            View::All => true,
        };
        in_view
            && (filter.kinds.is_empty() || filter.kinds.contains(&item.kind))
            && filter
                .intensity
                .is_none_or(|(lo, hi)| entry.and_then(|e| e.intensity).is_some_and(|v| (lo..=hi).contains(&v)))
            && (!filter.audio_reactive || item.audio_reactive)
            && (filter.pack.is_empty() || item.pack == filter.pack)
            && (filter.category.is_empty() || item.categories.iter().any(|c| c.eq_ignore_ascii_case(&filter.category)))
            && (words.is_empty() || matches_words(item, entry, words))
    }

    /// Every pack, sorted, each once.
    pub fn packs(&self) -> Vec<String> {
        self.items.iter().map(|i| i.pack.clone()).collect::<BTreeSet<_>>().into_iter().collect()
    }

    /// First launch: keeps the built-ins, the bundled effects and one bundled generator per
    /// category (at most [`STARTER_GENERATORS`]), so My library isn't empty. Runs once, and
    /// only after a scan found bundled shaders; returns whether it ran.
    pub fn apply_starter_set(&mut self) -> bool {
        if self.file.starter_set || !self.items.iter().any(|i| i.bundled && i.pack != BUILTIN_PACK) {
            return false;
        }
        let ids: Vec<String> = starter_set(&self.items).into_iter().map(|i| i.id.clone()).collect();
        for id in ids {
            let e = self.entry_mut(&id);
            if e.status == Status::NotReviewed {
                e.status = Status::Kept;
            }
        }
        self.file.starter_set = true;
        true
    }
}

fn matches_words(item: &Item, entry: Option<&Entry>, words: &[String]) -> bool {
    let mut hay = [&item.name, &item.description, &item.credit, &item.pack].map(|s| s.to_lowercase()).join("\n");
    for s in item.categories.iter().chain(entry.into_iter().flat_map(|e| &e.tags)) {
        hay.push('\n');
        hay.push_str(&s.to_lowercase());
    }
    words.iter().all(|w| hay.contains(w.as_str()))
}

/// The first-launch picks (see [`Library::apply_starter_set`]).
fn starter_set(items: &[Item]) -> Vec<&Item> {
    let mut out: Vec<&Item> =
        items.iter().filter(|i| i.bundled && (i.pack == BUILTIN_PACK || i.kind == Kind::Effect)).collect();
    let mut seen = HashSet::new();
    let mut gens: Vec<&Item> =
        items.iter().filter(|i| i.bundled && i.pack != BUILTIN_PACK && i.kind == Kind::Generator).collect();
    gens.sort_by_cached_key(|i| (i.name.to_lowercase(), i.id.clone()));
    for g in gens {
        if seen.len() == STARTER_GENERATORS {
            break;
        }
        if seen.insert(main_category(g)) {
            out.push(g);
        }
    }
    out
}

/// The first category that says more than "Generator".
fn main_category(item: &Item) -> String {
    item.categories
        .iter()
        .map(|c| c.trim().to_lowercase())
        .find(|c| !c.is_empty() && c != "generator" && c != "generators")
        .unwrap_or_default()
}

/// Lists everything in the effect and MilkDrop libraries plus the built-in generators.
/// `bundled_dir` is the folder of shaders shipped with openVJ (see
/// [`effect_folders::bundled_dir`](crate::effect_folders::bundled_dir)). Saved presets are
/// added by the caller.
pub fn scan_items(effects: &EffectLibrary, milk: &MilkLibrary, bundled_dir: Option<&Path>) -> Vec<Item> {
    let bundled_dir = bundled_dir.and_then(|d| d.canonicalize().ok());
    let mut items: Vec<Item> = GeneratorKind::ALL.iter().map(|&k| builtin_generator_item(k)).collect();
    for t in effects.all().iter().chain(effects.generators()) {
        if !t.def.hidden {
            items.push(isf_item(t, bundled_dir.as_deref()));
        }
    }
    items.extend(milk.presets().iter().map(|p| milk_item(p)));
    items
}

fn builtin_generator_item(kind: GeneratorKind) -> Item {
    Item {
        id: builtin_generator_id(kind),
        kind: Kind::Generator,
        name: kind.name().to_owned(),
        pack: BUILTIN_PACK.to_owned(),
        description: kind.description().to_owned(),
        credit: String::new(),
        categories: Vec::new(),
        // They all follow the tempo.
        audio_reactive: true,
        bundled: true,
        path: None,
    }
}

/// An ISF generator or effect as a library item.
pub fn isf_item(t: &EffectType, bundled_dir: Option<&Path>) -> Item {
    let bundled = match (&t.path, bundled_dir) {
        (None, _) => true,
        (Some(p), Some(dir)) => p.canonicalize().is_ok_and(|p| p.starts_with(dir)),
        (Some(_), None) => false,
    };
    Item {
        id: isf_id(t.id()),
        kind: if t.is_generator() { Kind::Generator } else { Kind::Effect },
        name: t.name().to_owned(),
        pack: isf_pack(&t.def, t.path.as_deref()),
        description: t.def.description.clone(),
        credit: t.def.credit.clone(),
        categories: t.def.categories.clone(),
        audio_reactive: isf_audio_reactive(&t.def),
        bundled,
        path: t.path.clone(),
    }
}

/// Built-ins, Shadertoy imports, or the name of the folder the file is in
/// (`ghost-arcade` → "Ghost Arcade").
fn isf_pack(def: &EffectDef, path: Option<&Path>) -> String {
    let Some(path) = path else { return BUILTIN_PACK.to_owned() };
    if def.source.contains("shadertoy.com") {
        return SHADERTOY_PACK.to_owned();
    }
    let folder = path.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
    folder.map(|f| title_case(&f)).unwrap_or_default()
}

/// `ghost-arcade` / `ghost_arcade` → "Ghost Arcade"; names that already have capitals stay.
fn title_case(folder: &str) -> String {
    if folder.chars().any(|c| c.is_uppercase()) {
        return folder.to_owned();
    }
    folder
        .split(['-', '_', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A MilkDrop preset as a library item. Its pack is its top subfolder.
pub fn milk_item(p: &MilkPreset) -> Item {
    let pack = p.category.split('/').next().filter(|s| !s.is_empty()).unwrap_or(MILKDROP_PACK);
    Item {
        id: milk_id(&p.id),
        kind: Kind::MilkDrop,
        name: p.name.clone(),
        pack: pack.to_owned(),
        description: String::new(),
        credit: String::new(),
        categories: if p.category.is_empty() { Vec::new() } else { vec![p.category.clone()] },
        audio_reactive: true,
        bundled: false,
        path: Some(p.path.clone()),
    }
}

/// The presets folder: `~/Documents/openVJ/Presets`, next to the Effects folder.
pub fn presets_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join("Documents").join("openVJ").join("Presets"))
}

/// A saved preset on disk: the Player's [`Preset`] (a source with its parameter values,
/// mappings, transform and effects) with a format version, like composition files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresetFile {
    pub version: u32,
    #[serde(flatten)]
    pub preset: Preset,
}

/// Reads a `.ovjpreset` file.
pub fn load_preset(path: &Path) -> Result<Preset, String> {
    let json = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file: PresetFile = serde_json::from_str(&json).map_err(|e| format!("{}: {e}", path.display()))?;
    if file.version > PRESET_VERSION {
        return Err(format!("{}: saved by a newer openVJ (version {})", path.display(), file.version));
    }
    Ok(file.preset)
}

/// Saves a preset as a new file in `dir`, named after the preset (`Tunnel.ovjpreset`,
/// then `Tunnel 2.ovjpreset`…), and returns its path. Never overwrites a file: saving a
/// preset always makes a new one, and the item it came from stays as it was.
pub fn save_new_preset(dir: &Path, preset: &Preset) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let base = file_safe(preset.name());
    let json = serde_json::to_string_pretty(&PresetFile { version: PRESET_VERSION, preset: preset.clone() })
        .map_err(std::io::Error::other)?;
    for n in 1u32.. {
        let name = if n == 1 { format!("{base}.{PRESET_EXTENSION}") } else { format!("{base} {n}.{PRESET_EXTENSION}") };
        let path = dir.join(name);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                use std::io::Write;
                f.write_all(json.as_bytes())?;
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    unreachable!("some file name is free")
}

/// Rewrites an existing preset file (the VJ edited the preset itself, not its source).
pub fn overwrite_preset(path: &Path, preset: &Preset) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(&PresetFile { version: PRESET_VERSION, preset: preset.clone() })
        .map_err(std::io::Error::other)?;
    let tmp = path.with_extension("ovjpreset.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)
}

/// A name usable as a file name on every system.
fn file_safe(name: &str) -> String {
    let s: String =
        name.chars()
            .map(|c| {
                if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() {
                    '-'
                } else {
                    c
                }
            })
            .collect();
    let s = s.trim().trim_start_matches('.').trim();
    if s.is_empty() { "Preset".into() } else { s.chars().take(100).collect() }
}

/// Every `.ovjpreset` file in `dir` (subfolders included) as library items. Files that
/// don't read are reported and skipped.
pub fn scan_presets(dir: &Path, effects: &EffectLibrary) -> Vec<Item> {
    let mut out = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), 0)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                if depth < 8 {
                    stack.push((path, depth + 1));
                }
            } else if path.extension().is_some_and(|x| x.eq_ignore_ascii_case(PRESET_EXTENSION)) {
                match load_preset(&path) {
                    Ok(p) => out.push(preset_item(dir, &path, &p, effects)),
                    Err(e) => eprintln!("openVJ: preset skipped: {e}"),
                }
            }
        }
    }
    out
}

/// A saved preset as a library item. Its id is its path inside the presets folder.
pub fn preset_item(dir: &Path, path: &Path, preset: &Preset, effects: &EffectLibrary) -> Item {
    let rel = path.strip_prefix(dir).unwrap_or(path);
    let rel = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
    let clip = &preset.clip;
    let source_reactive = match &clip.source {
        ClipSource::Isf { generator } => effects.generator(&generator.kind).is_some_and(|t| isf_audio_reactive(&t.def)),
        ClipSource::Generator { .. } | ClipSource::Milkdrop { .. } => true,
        _ => false,
    };
    let mapped = clip.mods.iter().any(|m| m.is_active())
        || clip
            .effects
            .iter()
            .chain(match &clip.source {
                ClipSource::Isf { generator } => Some(generator),
                _ => None,
            })
            .flat_map(|fx| fx.params.values())
            .any(|p| p.modulation.is_active());
    let source_name = match &clip.source {
        ClipSource::Isf { generator } => effects.generator(&generator.kind).map(|t| t.name().to_owned()),
        other => Some(other.default_name()),
    };
    Item {
        id: preset_id(&rel),
        kind: Kind::Preset,
        name: path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| preset.name().to_owned()),
        pack: PRESETS_PACK.to_owned(),
        description: source_name.map(|n| format!("Preset of {n}")).unwrap_or_default(),
        credit: String::new(),
        categories: Vec::new(),
        audio_reactive: source_reactive || mapped,
        bundled: false,
        path: Some(path.to_path_buf()),
    }
}

/// openVJ's own audio and tempo uniforms.
const AUDIO_UNIFORMS: &[&str] =
    &["AUDIO", "AUDIO_HITS", "AUDIO_PRESENCE", "AUDIO_TIME", "AUDIO_ALL", "BEAT_EVENTS", "BEAT", "BPM"];

/// Ghost Arcade, Synesthesia and ISF-host names that openvj-render maps onto openVJ's audio
/// (unless the file declares an input of the same name, which is then a plain parameter).
const AUDIO_SHIM_NAMES: &[&str] = &[
    "audioBass",
    "audioMid",
    "audioHigh",
    "audioLevel",
    "audioBeatPhase",
    "audioBeat",
    "audioBPM",
    "audioSpectralCentroid",
    "sampleFFT",
    "sampleWaveform",
    "syn_Spectrum",
    "syn_Level",
    "syn_BassLevel",
    "syn_MidLevel",
    "syn_MidHighLevel",
    "syn_HighLevel",
    "syn_Hits",
    "syn_BassHits",
    "syn_MidHits",
    "syn_MidHighHits",
    "syn_HighHits",
    "syn_Presence",
    "syn_BassPresence",
    "syn_MidPresence",
    "syn_MidHighPresence",
    "syn_HighPresence",
    "syn_Time",
    "syn_CurvedTime",
    "syn_BassTime",
    "syn_MidTime",
    "syn_MidHighTime",
    "syn_HighTime",
    "syn_OnBeat",
    "syn_ToggleOnBeat",
    "syn_RandomOnBeat",
    "syn_BeatTime",
    "syn_BPMTwitcher",
    "syn_BPM",
    "syn_BPMConfidence",
    "syn_BPMSin",
    "syn_BPMSin2",
    "syn_BPMSin4",
    "syn_BPMSin8",
    "syn_BPMTri",
    "syn_BPMTri2",
    "syn_BPMTri4",
    "syn_BPMTri8",
];

/// Whether an ISF shader reads audio, spectrum, beat or tempo: openVJ's uniforms, the names
/// the ISF shim maps (Ghost Arcade, Synesthesia) or a Shadertoy sound channel.
pub fn isf_audio_reactive(def: &EffectDef) -> bool {
    let glsl = &def.glsl;
    let uses = |w: &str| crate::shadertoy::uses_word(glsl, w);
    AUDIO_UNIFORMS.iter().any(|w| uses(w))
        || uses(crate::shadertoy::AUDIO_TEXTURE)
        || AUDIO_SHIM_NAMES.iter().any(|w| uses(w) && !def.inputs.iter().any(|i| i.name == *w))
}

/// The library id of a clip's source, if the library lists that kind of source.
pub fn source_id(source: &ClipSource) -> Option<String> {
    match source {
        ClipSource::Generator { kind } => Some(builtin_generator_id(*kind)),
        ClipSource::Isf { generator } => Some(isf_id(&generator.kind)),
        ClipSource::Milkdrop { preset } => Some(milk_id(preset)),
        _ => None,
    }
}

/// Ids of every source and effect the composition uses, in every deck, so the library can
/// count them as kept.
pub fn composition_ids(comp: &Composition) -> HashSet<String> {
    let mut ids = HashSet::new();
    let add_clip = |clip: &crate::Clip, ids: &mut HashSet<String>| {
        ids.extend(source_id(&clip.source));
        ids.extend(clip.effects.iter().map(|fx| isf_id(&fx.kind)));
    };
    for layer in &comp.layers {
        ids.extend(layer.effects.iter().map(|fx| isf_id(&fx.kind)));
        for clip in layer.clips.iter().flatten() {
            add_clip(clip, &mut ids);
        }
    }
    for deck in &comp.decks {
        for clip in deck.clips.iter().flatten().flatten() {
            add_clip(clip, &mut ids);
        }
    }
    ids.extend(comp.effects.iter().map(|fx| isf_id(&fx.kind)));
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::EffectType;
    use std::sync::Arc;

    fn isf(id: &str, src: &str, path: Option<&str>) -> Arc<EffectType> {
        let def = EffectDef::parse(src, id).expect("parses");
        Arc::new(EffectType::new(def, path.map(PathBuf::from)))
    }

    fn item(id: &str, kind: Kind, name: &str) -> Item {
        Item {
            id: id.into(),
            kind,
            name: name.into(),
            pack: "Pack".into(),
            description: String::new(),
            credit: String::new(),
            categories: Vec::new(),
            audio_reactive: false,
            bundled: false,
            path: None,
        }
    }

    fn sample() -> Library {
        let mut tunnel = item("isf:tunnel", Kind::Generator, "Neon Tunnel");
        tunnel.description = "Flies through rings".into();
        tunnel.audio_reactive = true;
        let blur = item("isf:blur", Kind::Effect, "Blur");
        let star = item("milk:Geiss/Starfish.milk", Kind::MilkDrop, "Starfish");
        let preset = item("preset:Peak.ovjpreset", Kind::Preset, "Peak");
        Library::new(vec![tunnel, star, blur, preset], LibraryFile::default())
    }

    fn ids(items: Vec<&Item>) -> Vec<&str> {
        items.into_iter().map(|i| i.id.as_str()).collect()
    }

    #[test]
    fn new_items_wait_in_discover_and_presets_are_kept() {
        let lib = sample();
        assert_eq!(lib.status("isf:tunnel"), Status::NotReviewed);
        assert_eq!(ids(lib.query(&Filter::default())), ["preset:Peak.ovjpreset"]);
        let discover = Filter { view: View::Discover, ..Default::default() };
        assert_eq!(ids(lib.query(&discover)), ["isf:blur", "isf:tunnel", "milk:Geiss/Starfish.milk"]);
    }

    #[test]
    fn unwanted_hides_and_restores_with_its_tags() {
        let mut lib = sample();
        lib.set_status("isf:tunnel", Status::Kept);
        lib.set_intensity("isf:tunnel", Some(4));
        lib.add_tag("isf:tunnel", "neon");
        lib.set_status("isf:tunnel", Status::Unwanted);
        assert!(!ids(lib.query(&Filter::default())).contains(&"isf:tunnel"));
        let discover = Filter { view: View::Discover, ..Default::default() };
        assert!(!ids(lib.query(&discover)).contains(&"isf:tunnel"));
        let unwanted = Filter { view: View::Unwanted, ..Default::default() };
        assert_eq!(ids(lib.query(&unwanted)), ["isf:tunnel"]);

        lib.set_status("isf:tunnel", Status::Kept);
        assert!(ids(lib.query(&Filter::default())).contains(&"isf:tunnel"));
        assert_eq!(lib.entry("isf:tunnel").intensity, Some(4));
        assert_eq!(lib.entry("isf:tunnel").tags, ["neon"]);
    }

    #[test]
    fn composition_items_count_as_kept() {
        let mut lib = sample();
        lib.set_status("isf:blur", Status::Unwanted);
        lib.set_in_use(["isf:blur".to_owned()].into());
        assert_eq!(lib.status("isf:blur"), Status::Kept);
        assert!(ids(lib.query(&Filter::default())).contains(&"isf:blur"));
    }

    #[test]
    fn composition_ids_cover_clips_effects_and_decks() {
        let comp = Composition::demo();
        let ids = composition_ids(&comp);
        assert!(ids.contains("gen:plasma"));
        assert!(ids.contains("isf:color"));
        assert!(ids.contains("isf:rgb_split"));
    }

    #[test]
    fn search_filters_and_intensity() {
        let mut lib = sample();
        for id in ["isf:tunnel", "isf:blur", "milk:Geiss/Starfish.milk"] {
            lib.set_status(id, Status::Kept);
        }
        lib.add_tag("milk:Geiss/Starfish.milk", "Peak hour");
        let q = |lib: &Library, f: Filter| ids(lib.query(&f)).into_iter().map(str::to_owned).collect::<Vec<_>>();

        assert_eq!(q(&lib, Filter { search: "TUNNEL".into(), ..Default::default() }), ["isf:tunnel"]);
        assert_eq!(q(&lib, Filter { search: "rings flies".into(), ..Default::default() }), ["isf:tunnel"]);
        assert_eq!(q(&lib, Filter { search: "peak".into(), ..Default::default() }).len(), 2); // tag + preset name
        assert_eq!(q(&lib, Filter { kinds: vec![Kind::Effect], ..Default::default() }), ["isf:blur"]);
        assert_eq!(q(&lib, Filter { audio_reactive: true, ..Default::default() }), ["isf:tunnel"]);

        lib.set_intensity("isf:blur", Some(9)); // clamped to 5
        lib.set_intensity("isf:tunnel", Some(2));
        assert_eq!(lib.entry("isf:blur").intensity, Some(5));
        assert_eq!(q(&lib, Filter { intensity: Some((4, 5)), ..Default::default() }), ["isf:blur"]);
        assert_eq!(q(&lib, Filter { intensity: Some((0, 5)), ..Default::default() }).len(), 2);
    }

    #[test]
    fn favorites_crates_and_recent() {
        let mut lib = sample();
        lib.toggle_favorite("isf:tunnel");
        lib.add_to_crate("isf:blur", "Peak hour");
        lib.add_to_crate("isf:tunnel", "Peak hour");
        lib.add_crate("Warm up");
        lib.record_use("isf:blur", 100);
        lib.record_use("isf:tunnel", 200);
        lib.record_use("isf:blur", 50);

        let fav = Filter { view: View::Favorites, ..Default::default() };
        assert_eq!(ids(lib.query(&fav)), ["isf:tunnel"]);
        let peak = Filter { view: View::Crate("Peak hour".into()), ..Default::default() };
        assert_eq!(ids(lib.query(&peak)), ["isf:blur", "isf:tunnel"]);
        assert_eq!(lib.crates(), ["Peak hour", "Warm up"]);
        let recent = Filter { view: View::Recent, ..Default::default() };
        assert_eq!(ids(lib.query(&recent)), ["isf:tunnel", "isf:blur"]);
        assert_eq!(lib.entry("isf:blur").uses, 2);

        assert!(lib.rename_crate("Peak hour", "Peak"));
        assert!(!lib.rename_crate("Peak", "Warm up"));
        assert_eq!(lib.entry("isf:blur").crates, ["Peak"]);
        lib.remove_crate("Peak");
        assert!(lib.entry("isf:blur").crates.is_empty());
        assert!(lib.get("isf:blur").is_some());
    }

    #[test]
    fn library_file_round_trips_and_keeps_missing_entries() {
        let mut lib = sample();
        lib.set_status("isf:tunnel", Status::Kept);
        lib.add_tag("isf:gone", "old tag");
        lib.toggle_favorite("isf:blur");
        lib.toggle_favorite("isf:blur"); // back to empty: not written
        let json = lib.file.to_json();
        assert!(!json.contains("isf:blur"));
        let back = LibraryFile::from_json(&json).expect("reads");
        assert_eq!(back.items.len(), 2);

        let lib = Library::new(sample().items().to_vec(), back);
        assert_eq!(lib.missing().collect::<Vec<_>>(), ["isf:gone"]);
        assert_eq!(lib.status("isf:tunnel"), Status::Kept);
    }

    #[test]
    fn library_file_load_and_save() {
        let dir = std::env::temp_dir().join(format!("openvj-library-{}", std::process::id()));
        let path = dir.join(LIBRARY_FILE);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(LibraryFile::load(&path).expect("missing is empty"), LibraryFile::default());
        let mut file = LibraryFile::default();
        file.items.insert("gen:plasma".into(), Entry { favorite: true, ..Default::default() });
        file.save(&path).expect("saves");
        assert_eq!(LibraryFile::load(&path).expect("loads"), file);

        std::fs::write(&path, r#"{"version": 99}"#).unwrap();
        assert!(LibraryFile::load(&path).is_err());
        std::fs::write(&path, "not json").unwrap();
        assert!(LibraryFile::load(&path).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detects_audio_reactive_shaders() {
        let reactive = |glsl: &str, inputs: &str| {
            let src = format!(r#"/*{{ "INPUTS": [{inputs}] }}*/ {glsl}"#);
            isf_audio_reactive(&EffectDef::parse(&src, "t").expect("parses"))
        };
        assert!(reactive("void main() { gl_FragColor = vec4(AUDIO.x); }", ""));
        assert!(reactive("void main() { float b = BPM; }", ""));
        assert!(reactive("void main() { float b = audioBass; }", ""));
        assert!(reactive("void main() { float b = syn_BassLevel; }", ""));
        assert!(!reactive("void main() { float b = TIME; }", ""));
        assert!(!reactive("void main() { float AUDIOX = 1.0; }", ""));
        // A declared input with a shim name is a plain parameter.
        assert!(!reactive(
            "void main() { float b = audioBass; }",
            r#"{ "NAME": "audioBass", "TYPE": "float", "DEFAULT": 0.5 }"#
        ));
    }

    #[test]
    fn isf_items_get_kind_pack_and_reactivity() {
        let gen_src =
            r#"/*{ "CATEGORIES": ["Generator", "Tunnel"], "INPUTS": [] }*/ void main() { gl_FragColor = vec4(BEAT); }"#;
        let fx_src =
            r#"/*{ "INPUTS": [{"NAME":"inputImage","TYPE":"image"}] }*/ void main() { gl_FragColor = vec4(1.0); }"#;
        let g = isf_item(&isf("t", gen_src, Some("/x/ghost-arcade/t.fs")), None);
        assert_eq!(
            (g.id.as_str(), g.kind, g.pack.as_str(), g.audio_reactive),
            ("isf:t", Kind::Generator, "Ghost Arcade", true)
        );
        let f = isf_item(&isf("f", fx_src, Some("/x/My FX/f.fs")), None);
        assert_eq!((f.kind, f.pack.as_str(), f.audio_reactive), (Kind::Effect, "My FX", false));
        let st = r#"/*{ "OPENVJ": { "SOURCE": "https://www.shadertoy.com/view/abc" }, "INPUTS": [] }*/ void main() {}"#;
        assert_eq!(isf_item(&isf("s", st, Some("/x/Effects/s.fs")), None).pack, SHADERTOY_PACK);
        let b = isf_item(&isf("blur", fx_src, None), None);
        assert_eq!((b.pack.as_str(), b.bundled), (BUILTIN_PACK, true));
    }

    #[test]
    fn scans_effects_milkdrop_and_builtin_generators() {
        let milk = MilkLibrary::new(
            Vec::new(),
            vec![MilkPreset {
                id: "Geiss/Starfish.milk".into(),
                name: "Starfish".into(),
                category: "Geiss".into(),
                path: "/m/Geiss/Starfish.milk".into(),
                version: 0,
            }],
        );
        let items = scan_items(&EffectLibrary::builtin(), &milk, None);
        let lib = Library::new(items, LibraryFile::default());
        assert!(lib.get("gen:tunnel").is_some());
        assert!(lib.get("isf:blur").is_some_and(|i| i.kind == Kind::Effect));
        let star = lib.get("milk:Geiss/Starfish.milk").expect("milk item");
        assert_eq!((star.pack.as_str(), star.audio_reactive), ("Geiss", true));
    }

    #[test]
    fn starter_set_keeps_builtins_effects_and_one_generator_per_category() {
        let mut items = vec![builtin_generator_item(GeneratorKind::Plasma)];
        for (id, cat) in [("a", "Tunnel"), ("b", "Tunnel"), ("c", "Fractal"), ("d", "Generator")] {
            let mut i = item(&format!("isf:{id}"), Kind::Generator, id);
            i.bundled = true;
            i.pack = "Ghost Arcade".into();
            i.categories = vec!["Generator".into(), cat.into()];
            items.push(i);
        }
        let mut fx = item("isf:glow", Kind::Effect, "Glow");
        fx.bundled = true;
        items.push(fx);
        items.push(item("isf:mine", Kind::Generator, "Mine")); // user folder: not bundled
        let mut lib = Library::new(items, LibraryFile::default());
        lib.set_status("isf:c", Status::Unwanted); // a choice already made stays

        assert!(lib.apply_starter_set());
        let kept: Vec<_> =
            lib.items().iter().filter(|i| lib.status(&i.id) == Status::Kept).map(|i| i.id.as_str()).collect();
        assert_eq!(kept, ["isf:a", "isf:d", "isf:glow", "gen:plasma"]);
        assert_eq!(lib.status("isf:c"), Status::Unwanted);
        assert!(!lib.apply_starter_set(), "only once");
    }

    #[test]
    fn starter_set_waits_for_the_bundled_scan() {
        let mut lib = Library::new(vec![builtin_generator_item(GeneratorKind::Plasma)], LibraryFile::default());
        assert!(!lib.apply_starter_set());
        assert!(!lib.file.starter_set);
    }

    #[test]
    fn bundled_ghost_arcade_pack_is_listed() {
        let Some(dir) = crate::effect_folders::bundled_dir() else { return };
        fn ok(_: &EffectDef) -> Result<(), String> {
            Ok(())
        }
        let mut scanner = crate::effect_folders::Scanner::new(vec![dir.clone()], ok);
        let lib = scanner.scan().expect("first scan").into_library();
        let mut library = Library::new(scan_items(&lib, &MilkLibrary::default(), Some(&dir)), LibraryFile::default());
        let ga: Vec<_> = library.items().iter().filter(|i| i.pack == "Ghost Arcade").collect();
        assert!(ga.len() > 300, "found {}", ga.len());
        assert!(ga.iter().all(|i| i.bundled));
        assert!(ga.iter().any(|i| i.audio_reactive));
        assert!(library.apply_starter_set());
        let kept_gens = library
            .items()
            .iter()
            .filter(|i| i.pack == "Ghost Arcade" && i.kind == Kind::Generator && library.status(&i.id) == Status::Kept)
            .count();
        assert!((5..=STARTER_GENERATORS).contains(&kept_gens), "kept {kept_gens}");
    }
    #[test]
    fn presets_save_as_new_files_and_list_as_kept_items() {
        let dir = std::env::temp_dir().join(format!("openvj-presets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut comp = Composition::new(16, 9, 1, 1);
        comp.set_clip(0, 0, ClipSource::Generator { kind: GeneratorKind::Tunnel });
        let mut clip = comp.clip(0, 0).cloned().unwrap();
        clip.name = "Tunnel: peak/drop".into();
        clip.params[0] = 0.9;
        let preset = Preset::new(clip);

        let a = save_new_preset(&dir, &preset).expect("saves");
        let b = save_new_preset(&dir, &preset).expect("saves again");
        assert_eq!(a.file_name().unwrap(), "Tunnel- peak-drop.ovjpreset");
        assert_eq!(b.file_name().unwrap(), "Tunnel- peak-drop 2.ovjpreset");
        assert_eq!(load_preset(&a).expect("loads"), preset);
        assert!(std::fs::read_to_string(&a).unwrap().contains("\"version\": 1"));

        let mut items = scan_presets(&dir, &EffectLibrary::builtin());
        items.sort_by(|x, y| x.id.cmp(&y.id));
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "preset:Tunnel- peak-drop 2.ovjpreset");
        assert_eq!((items[0].kind, items[0].audio_reactive), (Kind::Preset, true));
        assert_eq!(items[1].description, "Preset of Tunnel");
        let lib = Library::new(items, LibraryFile::default());
        assert_eq!(lib.query(&Filter::default()).len(), 2, "saved presets are in My library");

        let mut edited = preset.clone();
        edited.clip.params[0] = 0.1;
        overwrite_preset(&a, &edited).expect("overwrites");
        assert_eq!(load_preset(&a).unwrap(), edited);
        std::fs::write(&b, r#"{"version": 7, "clip": {}}"#).unwrap();
        assert!(load_preset(&b).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
