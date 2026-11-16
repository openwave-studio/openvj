//! MilkDrop presets (`.milk` files) loaded from folders.
//!
//! openVJ looks in `OPENVJ_MILKDROP_DIR` and `~/Documents/openVJ/MilkDrop`, subfolders
//! included. A preset's id is its path inside its folder with `/` separators
//! (`Geiss/Starfish.milk`), so compositions open on another machine with the same pack.
//! A preset outside every folder (dropped from the Files tab) is saved by its full path.
//! [`watch`] rescans about once a second and swaps the [library](milk_library) when a file
//! was added, changed or removed.

use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

/// Extension of MilkDrop presets (matched case-insensitively).
pub const MILK_EXTENSION: &str = "milk";

/// One `.milk` file.
#[derive(Debug, Clone, PartialEq)]
pub struct MilkPreset {
    /// Path inside its folder with `/` separators, or the full path for a loose file.
    pub id: String,
    /// The file name without `.milk`.
    pub name: String,
    /// The subfolder it is in (`Geiss`, `Dancer/Glowsticks`), empty at the top.
    pub category: String,
    pub path: PathBuf,
    /// Changes when the file does (size and modification time), so renderers reload it.
    pub version: u64,
}

impl MilkPreset {
    /// A preset for a file, read from disk now. `None` if it isn't a readable file.
    fn from_file(id: String, category: String, path: &Path) -> Option<Self> {
        let meta = std::fs::metadata(path).ok().filter(|m| m.is_file())?;
        let name = path.file_stem()?.to_string_lossy().into_owned();
        Some(Self { id, name, category, path: path.to_owned(), version: stamp(meta.len(), meta.modified().ok()) })
    }
}

fn stamp(len: u64, modified: Option<SystemTime>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (len, modified).hash(&mut h);
    h.finish()
}

/// Every preset found in the MilkDrop folders.
#[derive(Debug, Default)]
pub struct MilkLibrary {
    /// Sorted by category, then name (case-insensitive).
    presets: Vec<Arc<MilkPreset>>,
    by_id: HashMap<String, usize>,
    /// The folders scanned, in priority order.
    pub dirs: Vec<PathBuf>,
    /// Goes up by one each time the library is replaced.
    pub generation: u64,
}

impl MilkLibrary {
    pub fn new(dirs: Vec<PathBuf>, mut presets: Vec<MilkPreset>) -> Self {
        presets.sort_by_cached_key(|p| (p.category.to_lowercase(), p.name.to_lowercase(), p.id.clone()));
        let presets: Vec<_> = presets.into_iter().map(Arc::new).collect();
        let by_id = presets.iter().enumerate().map(|(i, p)| (p.id.clone(), i)).collect();
        Self { presets, by_id, dirs, generation: 0 }
    }

    pub fn presets(&self) -> &[Arc<MilkPreset>] {
        &self.presets
    }

    pub fn get(&self, id: &str) -> Option<&Arc<MilkPreset>> {
        self.by_id.get(id).map(|&i| &self.presets[i])
    }

    /// The preset an id names: one in the folders, or a loose file by its full path.
    pub fn resolve(&self, id: &str) -> Option<Arc<MilkPreset>> {
        if let Some(p) = self.get(id) {
            return Some(p.clone());
        }
        let path = Path::new(id);
        if path.is_absolute() {
            return MilkPreset::from_file(id.to_owned(), String::new(), path).map(Arc::new);
        }
        None
    }

    /// The id to save a `.milk` file under: its path inside a MilkDrop folder, else its full path.
    pub fn id_for_path(&self, path: &Path) -> String {
        self.dirs
            .iter()
            .find_map(|d| path.strip_prefix(d).ok())
            .map(slash_path)
            .unwrap_or_else(|| path.to_string_lossy().into_owned())
    }
}

fn slash_path(rel: &Path) -> String {
    rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}

/// True for a path ending in `.milk` (any case).
pub fn is_milk(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case(MILK_EXTENSION))
}

static LIBRARY: RwLock<Option<Arc<MilkLibrary>>> = RwLock::new(None);

/// The current MilkDrop library (empty until [`watch`] or [`set_milk_library`] fills it).
/// Cheap: take one snapshot per frame or action.
pub fn milk_library() -> Arc<MilkLibrary> {
    if let Some(lib) = LIBRARY.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
        return lib.clone();
    }
    let mut slot = LIBRARY.write().unwrap_or_else(|e| e.into_inner());
    slot.get_or_insert_with(|| Arc::new(MilkLibrary { dirs: default_dirs(), ..Default::default() })).clone()
}

/// Replaces the MilkDrop library.
pub fn set_milk_library(mut lib: MilkLibrary) {
    let mut slot = LIBRARY.write().unwrap_or_else(|e| e.into_inner());
    lib.generation = slot.as_ref().map_or(0, |l| l.generation) + 1;
    *slot = Some(Arc::new(lib));
}

/// The openVJ MilkDrop folder: `~/Documents/openVJ/MilkDrop`.
pub fn user_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join("Documents").join("openVJ").join("MilkDrop"))
}

/// Every folder presets load from, in priority order (an earlier folder wins an id clash).
pub fn default_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("OPENVJ_MILKDROP_DIR").map(PathBuf::from).into_iter().collect();
    dirs.extend(user_dir());
    dirs
}

/// Finds every `.milk` file under `dirs`. Hidden files and folders are skipped.
pub fn scan(dirs: &[PathBuf]) -> Vec<MilkPreset> {
    let mut found: BTreeMap<String, MilkPreset> = BTreeMap::new();
    for dir in dirs {
        let mut stack = vec![dir.clone()];
        while let Some(d) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&d) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if entry.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                let Ok(ty) = entry.file_type() else { continue };
                if ty.is_dir() {
                    stack.push(path);
                } else if is_milk(&path) {
                    let Ok(rel) = path.strip_prefix(dir) else { continue };
                    let id = slash_path(rel);
                    if found.contains_key(&id) {
                        continue;
                    }
                    let category = rel.parent().map(slash_path).unwrap_or_default();
                    if let Some(p) = MilkPreset::from_file(id.clone(), category, &path) {
                        found.insert(id, p);
                    }
                }
            }
        }
    }
    found.into_values().collect()
}

/// Scans `dirs` on a background thread right away, then about once a second, and replaces
/// the library whenever a preset was added, changed or removed. Creates the openVJ
/// MilkDrop folder if it's missing.
pub fn watch(dirs: Vec<PathBuf>) {
    if let Some(user) = user_dir()
        && dirs.contains(&user)
    {
        let _ = std::fs::create_dir_all(&user);
    }
    let spawned = std::thread::Builder::new().name("milkdrop-folders".into()).spawn(move || {
        let mut last: Option<Vec<MilkPreset>> = None;
        loop {
            let presets = scan(&dirs);
            if last.as_ref() != Some(&presets) {
                last = Some(presets.clone());
                set_milk_library(MilkLibrary::new(dirs.clone(), presets));
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    });
    if let Err(e) = spawned {
        eprintln!("MilkDrop folders won't load: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_subfolders_by_relative_id() {
        let root = std::env::temp_dir().join(format!("openvj-milk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("Geiss/Old")).unwrap();
        std::fs::create_dir_all(root.join(".hidden")).unwrap();
        std::fs::write(root.join("Geiss/Starfish.milk"), "[preset00]\n").unwrap();
        std::fs::write(root.join("Geiss/Old/Swirl.MILK"), "[preset00]\n").unwrap();
        std::fs::write(root.join("Top.milk"), "[preset00]\n").unwrap();
        std::fs::write(root.join("notes.txt"), "").unwrap();
        std::fs::write(root.join(".hidden/Secret.milk"), "").unwrap();

        let lib = MilkLibrary::new(vec![root.clone()], scan(std::slice::from_ref(&root)));
        let ids: Vec<_> = lib.presets().iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["Top.milk", "Geiss/Starfish.milk", "Geiss/Old/Swirl.MILK"]);
        let star = lib.get("Geiss/Starfish.milk").unwrap();
        assert_eq!((star.name.as_str(), star.category.as_str()), ("Starfish", "Geiss"));
        assert_eq!(lib.id_for_path(&root.join("Geiss/Starfish.milk")), "Geiss/Starfish.milk");

        // A loose file resolves by its full path; an unknown id doesn't resolve.
        let loose = std::env::temp_dir().join(format!("openvj-loose-{}.milk", std::process::id()));
        std::fs::write(&loose, "[preset00]\n").unwrap();
        let id = lib.id_for_path(&loose);
        assert_eq!(lib.resolve(&id).unwrap().name, loose.file_stem().unwrap().to_string_lossy());
        assert!(lib.resolve("Gone/Nope.milk").is_none());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_file(&loose);
    }
}
