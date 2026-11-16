//! The app's side of the library: `library.json` in the settings folder, kept in step with
//! the effect and MilkDrop folders, and a disk cache of thumbnails rendered by the engine.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use openvj_core::library::{
    Item, Kind, LIBRARY_FILE, Library, LibraryFile, Status, load_preset, presets_dir, scan_items, scan_presets,
};
use openvj_core::milkdrop::milk_library;
use openvj_core::{ClipSource, Effect, GeneratorKind, library as effect_library};
use openvj_render::{Engine, Thumbnail, ThumbnailSource, effect_test_source, thumbnail_clip};

/// How long after a change the library file is written.
const SAVE_DELAY: Duration = Duration::from_secs(2);

/// Bump when thumbnails should be rendered again (a new look for effects, a new size).
const THUMB_VERSION: u32 = 1;

/// Thumbnails the engine is given at once; the rest wait here so the order can change.
const IN_FLIGHT: usize = 2;

/// Smoothed frame time above which thumbnails pause, so a show never stutters for them.
pub const THUMB_FRAME_BUDGET: Duration = Duration::from_millis(22);

/// openVJ's cache folder: `~/Library/Caches/openVJ` on macOS, `%LOCALAPPDATA%\openVJ\Cache`
/// on Windows, `$XDG_CACHE_HOME/openvj` elsewhere.
pub fn cache_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        Some(home?.join("Library/Caches/openVJ"))
    } else if cfg!(windows) {
        Some(PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("openVJ").join("Cache"))
    } else {
        Some(
            std::env::var_os("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .or_else(|| home.map(|h| h.join(".cache")))?
                .join("openvj"),
        )
    }
}

/// The library as the app uses it.
pub struct AppLibrary {
    pub lib: Library,
    /// `None` when there is no settings folder, or the file couldn't be read (it is then
    /// left alone rather than overwritten).
    path: Option<PathBuf>,
    changed_at: Option<Instant>,
    effects_generation: Option<u64>,
    milk_generation: Option<u64>,
    /// The presets folder needs listing again (a preset was saved).
    presets_dirty: bool,
    pub thumbs: ThumbCache,
}

impl AppLibrary {
    pub fn load() -> Self {
        let path = crate::prefs::settings_dir().map(|d| d.join(LIBRARY_FILE));
        let (file, path) = match path.as_deref().map(LibraryFile::load) {
            Some(Ok(file)) => (file, path),
            Some(Err(e)) => {
                eprintln!("openVJ: library not loaded, and won't be saved this session: {e}");
                (LibraryFile::default(), None)
            }
            None => (LibraryFile::default(), None),
        };
        Self {
            lib: Library::new(Vec::new(), file),
            path,
            changed_at: None,
            effects_generation: None,
            milk_generation: None,
            presets_dirty: true,
            thumbs: ThumbCache::new(cache_dir().map(|d| d.join("Thumbnails"))),
        }
    }

    /// Notes a change; the file is written a little later by [`AppLibrary::save_if_due`].
    pub fn changed(&mut self) {
        self.changed_at.get_or_insert_with(Instant::now);
    }

    /// Lists the presets folder again on the next [`AppLibrary::refresh`].
    #[expect(dead_code, reason = "saving presets (Synthesis, Discover) calls it")]
    pub fn presets_changed(&mut self) {
        self.presets_dirty = true;
    }

    /// Rebuilds the items when an effect, MilkDrop or presets folder changed. Returns
    /// whether it did.
    pub fn refresh(&mut self) -> bool {
        let fx = effect_library();
        let milk = milk_library();
        if self.effects_generation == Some(fx.generation)
            && self.milk_generation == Some(milk.generation)
            && !self.presets_dirty
        {
            return false;
        }
        self.effects_generation = Some(fx.generation);
        self.milk_generation = Some(milk.generation);
        self.presets_dirty = false;
        let bundled = openvj_core::effect_folders::bundled_dir();
        let mut items = scan_items(&fx, &milk, bundled.as_deref());
        if let Some(dir) = presets_dir() {
            items.extend(scan_presets(&dir, &fx));
        }
        self.lib.set_items(items);
        if self.lib.apply_starter_set() {
            self.changed();
        }
        self.thumbs.want(self.lib.items(), |id| self.lib.status(id));
        true
    }

    pub fn save_if_due(&mut self) {
        if self.changed_at.is_some_and(|t| t.elapsed() >= SAVE_DELAY) {
            self.save_now();
        }
    }

    pub fn save_now(&mut self) {
        if self.changed_at.take().is_none() {
            return;
        }
        if let Some(path) = &self.path
            && let Err(e) = self.lib.file.save(path)
        {
            eprintln!("openVJ: could not save the library to {}: {e}", path.display());
        }
    }
}

/// What the engine renders for an item's thumbnail.
pub fn thumbnail_source(item: &Item) -> Option<ThumbnailSource> {
    let (prefix, rest) = item.id.split_once(':')?;
    let clip = |source| Some(thumbnail_clip(source, Vec::new()));
    match (prefix, item.kind) {
        ("gen", _) => clip(ClipSource::Generator {
            kind: GeneratorKind::ALL.into_iter().find(|k| k.name().eq_ignore_ascii_case(rest))?,
        }),
        ("isf", Kind::Generator) => clip(ClipSource::isf(rest)),
        ("isf", Kind::Effect) => Some(thumbnail_clip(effect_test_source(), vec![Effect::new(rest)])),
        ("milk", _) => clip(ClipSource::Milkdrop { preset: rest.to_owned() }),
        ("preset", _) => Some(ThumbnailSource::Clip(Box::new(load_preset(item.path.as_deref()?).ok()?.clip))),
        _ => None,
    }
}

/// 64-bit FNV-1a: stable across Rust versions and machines, unlike `DefaultHasher`.
fn fnv1a(hash: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(hash, |h, b| (h ^ *b as u64).wrapping_mul(0x0000_0100_0000_01b3))
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// The cache key of an item's thumbnail: its id and kind, the thumbnail version and the
/// file's content (or openVJ's version for built-ins), so an edited file gets a new one.
pub fn thumb_key(id: &str, kind: Kind, path: Option<&Path>) -> String {
    let mut h = fnv1a(FNV_OFFSET, &THUMB_VERSION.to_le_bytes());
    h = fnv1a(h, id.as_bytes());
    h = fnv1a(h, kind.name().as_bytes());
    h = match path.map(std::fs::read) {
        Some(Ok(bytes)) => fnv1a(h, &bytes),
        _ => fnv1a(h, env!("CARGO_PKG_VERSION").as_bytes()),
    };
    format!("{h:016x}")
}

/// The VJ's own pick ("Use Preview frame as thumbnail") is stored by item id.
#[expect(dead_code, reason = "the Sources grid (add-library-browser task group 3) uses it")]
fn custom_key(id: &str) -> String {
    format!("custom-{:016x}", fnv1a(FNV_OFFSET, id.as_bytes()))
}

/// Thumbnails on disk, and the queue of ones still to render.
pub struct ThumbCache {
    dir: Option<PathBuf>,
    /// Cache key by item id, once hashed.
    keys: HashMap<String, String>,
    /// Keys with a file in the cache folder.
    on_disk: HashSet<String>,
    /// Item ids still to render, most wanted first, and their keys while in the engine.
    todo: VecDeque<String>,
    in_flight: HashMap<String, String>,
    sources: HashMap<String, Item>,
    keys_rx: Option<mpsc::Receiver<Vec<(String, String)>>>,
    saved_tx: mpsc::Sender<(String, Option<String>)>,
    saved_rx: mpsc::Receiver<(String, Option<String>)>,
    /// Item ids whose thumbnail file appeared since [`ThumbCache::take_updated`].
    updated: Vec<String>,
}

impl ThumbCache {
    pub fn new(dir: Option<PathBuf>) -> Self {
        let on_disk = dir
            .as_deref()
            .and_then(|d| std::fs::read_dir(d).ok())
            .map(|rd| {
                rd.flatten().filter_map(|e| e.file_name().to_str()?.strip_suffix(".jpg").map(str::to_owned)).collect()
            })
            .unwrap_or_default();
        let (saved_tx, saved_rx) = mpsc::channel();
        Self {
            dir,
            keys: HashMap::new(),
            on_disk,
            todo: VecDeque::new(),
            in_flight: HashMap::new(),
            sources: HashMap::new(),
            keys_rx: None,
            saved_tx,
            saved_rx,
            updated: Vec::new(),
        }
    }

    /// The thumbnail file for an item, if there is one: the VJ's own pick first.
    #[expect(dead_code, reason = "the Sources grid (add-library-browser task group 3) uses it")]
    pub fn path(&self, id: &str) -> Option<PathBuf> {
        let dir = self.dir.as_ref()?;
        let custom = custom_key(id);
        if self.on_disk.contains(&custom) {
            return Some(dir.join(format!("{custom}.jpg")));
        }
        let key = self.keys.get(id)?;
        self.on_disk.contains(key).then(|| dir.join(format!("{key}.jpg")))
    }

    /// Item ids whose thumbnail appeared since the last call.
    #[expect(dead_code, reason = "the Sources grid (add-library-browser task group 3) uses it")]
    pub fn take_updated(&mut self) -> Vec<String> {
        std::mem::take(&mut self.updated)
    }

    /// Thumbnails waiting to render.
    #[expect(dead_code, reason = "the Sources grid (add-library-browser task group 3) uses it")]
    pub fn pending(&self) -> usize {
        self.todo.len() + self.in_flight.len()
    }

    /// Hashes every item's file on a worker thread; items without a cached thumbnail then
    /// queue up, My library first.
    fn want(&mut self, items: &[Item], status: impl Fn(&str) -> Status) {
        if self.dir.is_none() {
            return;
        }
        let mut list: Vec<Item> = items.iter().filter(|i| thumbnail_source(i).is_some()).cloned().collect();
        list.sort_by_key(|i| match status(&i.id) {
            Status::Kept => 0,
            Status::NotReviewed => 1,
            Status::Unwanted => 2,
        });
        self.sources = list.iter().map(|i| (i.id.clone(), i.clone())).collect();
        let jobs: Vec<(String, Kind, Option<PathBuf>)> = list.into_iter().map(|i| (i.id, i.kind, i.path)).collect();
        let (tx, rx) = mpsc::channel();
        self.keys_rx = Some(rx);
        std::thread::Builder::new()
            .name("thumbnail keys".into())
            .spawn(move || {
                let keys = jobs.into_iter().map(|(id, kind, path)| {
                    let key = thumb_key(&id, kind, path.as_deref());
                    (id, key)
                });
                let _ = tx.send(keys.collect());
            })
            .expect("spawn thumbnail key thread");
    }

    /// Keeps the engine fed with thumbnails to render and saves the finished ones. Call
    /// once per frame; `over_budget` pauses the work.
    pub fn pump(&mut self, engine: &mut Engine, over_budget: bool) {
        if let Some(keys) = self.keys_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.keys_rx = None;
            self.todo.clear();
            for (id, key) in keys {
                if !self.on_disk.contains(&key) {
                    self.todo.push_back(id.clone());
                }
                self.keys.insert(id, key);
            }
        }
        engine.pause_thumbnails(over_budget);
        while !over_budget && self.in_flight.len() < IN_FLIGHT {
            let Some(id) = self.todo.pop_front() else { break };
            let (Some(item), Some(key)) = (self.sources.get(&id), self.keys.get(&id)) else { continue };
            if self.on_disk.contains(key) || self.in_flight.values().any(|k| k == key) {
                continue;
            }
            let Some(source) = thumbnail_source(item) else { continue };
            engine.request_thumbnail(key.clone(), source);
            self.in_flight.insert(id, key.clone());
        }
        for thumb in engine.take_thumbnails() {
            self.save(thumb);
        }
        while let Ok((key, id)) = self.saved_rx.try_recv() {
            self.on_disk.insert(key);
            self.updated.extend(id);
        }
    }

    /// Stores the Preview's next frame as an item's thumbnail.
    #[expect(dead_code, reason = "the Sources grid (add-library-browser task group 3) uses it")]
    pub fn use_preview(&mut self, engine: &mut Engine, id: &str) {
        if self.dir.is_none() {
            return;
        }
        let key = custom_key(id);
        engine.request_thumbnail(key.clone(), ThumbnailSource::Preview);
        self.in_flight.insert(id.to_owned(), key);
    }

    /// Encodes a finished thumbnail as JPEG on a worker thread.
    fn save(&mut self, thumb: Thumbnail) {
        let id = self.in_flight.iter().find(|(_, k)| **k == thumb.key).map(|(id, _)| id.clone());
        if let Some(id) = &id {
            self.in_flight.remove(id);
        }
        let (Some(dir), Some(rgba)) = (self.dir.clone(), thumb.rgba) else { return };
        let tx = self.saved_tx.clone();
        let key = thumb.key;
        let (w, h) = (thumb.width, thumb.height);
        std::thread::spawn(move || {
            let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
            let Some(img) = image::RgbImage::from_raw(w, h, rgb) else { return };
            let path = dir.join(format!("{key}.jpg"));
            let tmp = dir.join(format!("{key}.jpg.tmp"));
            let written = std::fs::create_dir_all(&dir)
                .map_err(|e| e.to_string())
                .and_then(|_| {
                    let mut out = std::io::BufWriter::new(std::fs::File::create(&tmp).map_err(|e| e.to_string())?);
                    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85)
                        .encode_image(&img)
                        .map_err(|e| e.to_string())
                })
                .and_then(|_| std::fs::rename(&tmp, &path).map_err(|e| e.to_string()));
            match written {
                Ok(()) => {
                    let _ = tx.send((key, id));
                }
                Err(e) => eprintln!("openVJ: could not save thumbnail {}: {e}", path.display()),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumb_keys_follow_file_content() {
        let dir = std::env::temp_dir().join(format!("openvj-thumbkeys-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.fs");
        std::fs::write(&f, "one").unwrap();
        let a = thumb_key("isf:a", Kind::Generator, Some(&f));
        assert_eq!(a, thumb_key("isf:a", Kind::Generator, Some(&f)), "stable");
        assert_ne!(a, thumb_key("isf:a", Kind::Effect, Some(&f)));
        std::fs::write(&f, "two").unwrap();
        assert_ne!(a, thumb_key("isf:a", Kind::Generator, Some(&f)));
        assert_eq!(a.len(), 16);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn known_fnv_value() {
        assert_eq!(fnv1a(FNV_OFFSET, b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn sources_for_each_kind() {
        let item = |id: &str, kind| Item {
            id: id.into(),
            kind,
            name: String::new(),
            pack: String::new(),
            description: String::new(),
            credit: String::new(),
            categories: Vec::new(),
            audio_reactive: false,
            bundled: false,
            path: None,
        };
        let src = |id, kind| match thumbnail_source(&item(id, kind)) {
            Some(ThumbnailSource::Clip(clip)) => Some(*clip),
            _ => None,
        };
        let plasma = src("gen:plasma", Kind::Generator).expect("built-in");
        assert!(matches!(plasma.source, ClipSource::Generator { kind: GeneratorKind::Plasma }));
        assert_eq!(src("isf:blur", Kind::Effect).expect("effect").effects.len(), 1);
        let milk = src("milk:Geiss/A.milk", Kind::MilkDrop).expect("milk");
        assert!(matches!(milk.source, ClipSource::Milkdrop { preset } if preset == "Geiss/A.milk"));
        assert!(src("preset:x.ovjpreset", Kind::Preset).is_none(), "no file");
    }
}
