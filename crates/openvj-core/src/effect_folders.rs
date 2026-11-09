//! Effects loaded from folders, like Resolume's Extra Effects folder.
//!
//! openVJ looks in `~/Documents/openVJ/Effects`, the standard ISF folders on macOS
//! (`~/Library/Graphics/ISF`, `/Library/Graphics/ISF`) and `OPENVJ_EFFECTS_DIR`, subfolders
//! included. Every `.fs` file with an `inputImage` input is an effect; the others are ISF
//! generators, which play as clips. [`Scanner`] remembers each file's size and modification time so a rescan only
//! re-reads what changed, and [`watch`] rescans about once a second and swaps the
//! [library](crate::library) when something changed.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::effect::{EffectError, EffectLibrary, EffectType, is_builtin, set_library};
use crate::isf::EffectDef;

/// Checks that an effect's GLSL compiles (openvj-render provides one using naga).
pub type Check = fn(&EffectDef) -> Result<(), String>;

/// The openVJ effects folder: `~/Documents/openVJ/Effects`.
pub fn user_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join("Documents").join("openVJ").join("Effects"))
}

/// The shaders bundled with openVJ (the Ghost Arcade pack): `Contents/Resources/shaders` in
/// the macOS app, `shaders` next to the executable elsewhere, or the repo's `shaders` folder
/// when running from a checkout.
pub fn bundled_dir() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf));
    let candidates = exe_dir
        .iter()
        .flat_map(|d| [d.join("../Resources/shaders"), d.join("shaders")])
        .chain([Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shaders")]);
    candidates.into_iter().find(|d| d.is_dir())
}

/// Every folder effects load from, in priority order (an earlier folder wins an id clash).
/// The bundled shaders come last, so a user's own file can take over one of their ids.
pub fn default_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(d) = std::env::var_os("OPENVJ_EFFECTS_DIR") {
        dirs.push(PathBuf::from(d));
    }
    dirs.extend(user_dir());
    if cfg!(target_os = "macos") {
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(home).join("Library/Graphics/ISF"));
        }
        dirs.push(PathBuf::from("/Library/Graphics/ISF"));
    }
    dirs.extend(bundled_dir());
    dirs
}

/// What a file looked like when it was last read.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Stamp {
    modified: Option<SystemTime>,
    len: u64,
    /// Whether a `.vs` file sits next to it.
    has_vs: bool,
}

/// The result of loading one file.
#[derive(Debug, Clone)]
enum Loaded {
    /// An effect or an ISF generator (no `inputImage`).
    Effect(Arc<EffectType>),
    Error(EffectError),
}

/// Loads effect folders, re-reading only files that changed since the last scan.
#[derive(Debug)]
pub struct Scanner {
    dirs: Vec<PathBuf>,
    check: Check,
    files: HashMap<PathBuf, (Stamp, Loaded)>,
    scanned: bool,
}

/// What a scan found.
#[derive(Debug, Default)]
pub struct ScanResult {
    pub effects: Vec<Arc<EffectType>>,
    /// ISF generators (files without an `inputImage`), which play as clips.
    pub generators: Vec<Arc<EffectType>>,
    pub errors: Vec<EffectError>,
}

impl ScanResult {
    /// The built-ins plus everything found.
    pub fn into_library(self) -> EffectLibrary {
        EffectLibrary::with(self.effects, self.errors).with_generators(self.generators)
    }
}

impl Scanner {
    pub fn new(dirs: Vec<PathBuf>, check: Check) -> Self {
        Self { dirs, check, files: HashMap::new(), scanned: false }
    }

    pub fn dirs(&self) -> &[PathBuf] {
        &self.dirs
    }

    /// Rescans the folders. Returns `None` when nothing changed since the last call.
    pub fn scan(&mut self) -> Option<ScanResult> {
        let mut listed: Vec<(PathBuf, Stamp)> = Vec::new();
        for dir in &self.dirs {
            let mut found = Vec::new();
            find_fs(dir, &mut found, 0);
            found.sort_by(|a, b| a.0.cmp(&b.0));
            listed.extend(found);
        }
        let changed = listed.len() != self.files.len()
            || listed.iter().any(|(p, s)| self.files.get(p).is_none_or(|(old, _)| old != s));
        if !changed && self.scanned {
            return None;
        }
        self.scanned = true;
        let mut files = HashMap::new();
        for (path, stamp) in &listed {
            let loaded = match self.files.remove(path) {
                Some((old, loaded)) if old == *stamp => loaded,
                _ => load(path, *stamp, self.check),
            };
            files.insert(path.clone(), (*stamp, loaded));
        }
        self.files = files;

        // Resolve ids in folder priority order: `listed` is already in that order.
        let mut result = ScanResult::default();
        let mut taken: BTreeMap<String, PathBuf> = BTreeMap::new();
        for (path, _) in &listed {
            match &self.files[path].1 {
                Loaded::Error(e) => result.errors.push(e.clone()),
                Loaded::Effect(t) => {
                    let id = t.id();
                    if is_builtin(id) {
                        result.errors.push(clash(path, t, "a built-in effect"));
                    } else if let Some(first) = taken.get(id) {
                        result.errors.push(clash(path, t, &first.display().to_string()));
                    } else {
                        taken.insert(id.to_owned(), path.clone());
                        match t.is_generator() {
                            true => result.generators.push(t.clone()),
                            false => result.effects.push(t.clone()),
                        }
                    }
                }
            }
        }
        result.effects.sort_by_key(|t| t.name().to_lowercase());
        result.generators.sort_by_key(|t| t.name().to_lowercase());
        result.errors.sort_by_key(|e| e.name.to_lowercase());
        Some(result)
    }
}

fn clash(path: &Path, t: &EffectType, other: &str) -> EffectError {
    EffectError {
        path: path.to_owned(),
        name: t.name().to_owned(),
        message: format!("the id \"{}\" is already used by {other}; set another OPENVJ.ID in the header", t.id()),
    }
}

/// Collects `.fs` files under `dir`, skipping hidden entries.
fn find_fs(dir: &Path, out: &mut Vec<(PathBuf, Stamp)>, depth: usize) {
    if depth > 8 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let Ok(meta) = std::fs::metadata(&path) else { continue };
        if meta.is_dir() {
            find_fs(&path, out, depth + 1);
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("fs")) {
            let has_vs = path.with_extension("vs").is_file();
            out.push((path, Stamp { modified: meta.modified().ok(), len: meta.len(), has_vs }));
        }
    }
}

fn load(path: &Path, stamp: Stamp, check: Check) -> Loaded {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let error = |name: String, message: String| {
        eprintln!("effect {}: {message}", path.display());
        Loaded::Error(EffectError { path: path.to_owned(), name, message })
    };
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => return error(stem, format!("can't read the file: {e}")),
    };
    let def = match EffectDef::parse(&src, &stem) {
        Ok(d) => d,
        Err(e) => return error(stem, e),
    };
    let name = def.name.clone();
    if stamp.has_vs {
        return error(name, "custom vertex shaders (.vs files) aren't supported yet".into());
    }
    if let Some(img) = def.extra_images().next() {
        return error(name, format!("image input \"{}\" isn't supported yet", img.name));
    }
    if let Err(e) = check(&def) {
        return error(name, e);
    }
    Loaded::Effect(Arc::new(EffectType::new(def, Some(path.to_owned()))))
}

/// Watches `dirs` on a background thread: scans them right away, then about once a second,
/// and replaces the [library](crate::library) whenever an effect file was added, changed
/// or removed. Creates the openVJ effects folder if it's missing.
pub fn watch(dirs: Vec<PathBuf>, check: Check) {
    if let Some(user) = user_dir()
        && dirs.contains(&user)
    {
        let _ = std::fs::create_dir_all(&user);
    }
    let mut scanner = Scanner::new(dirs, check);
    let spawned = std::thread::Builder::new().name("effect-folders".into()).spawn(move || {
        loop {
            install(&mut scanner);
            std::thread::sleep(Duration::from_secs(1));
        }
    });
    if let Err(e) = spawned {
        eprintln!("effect folders won't load: {e}");
    }
}

fn install(scanner: &mut Scanner) {
    if let Some(r) = scanner.scan() {
        set_library(r.into_library());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(_: &EffectDef) -> Result<(), String> {
        Ok(())
    }

    fn no_nope(def: &EffectDef) -> Result<(), String> {
        if def.glsl.contains("nope") { Err("error: unknown identifier `nope`\n  more".into()) } else { Ok(()) }
    }

    const FILTER: &str = r#"/*{ "DESCRIPTION": "Test", "INPUTS": [
        {"NAME": "inputImage", "TYPE": "image"},
        {"NAME": "amount", "TYPE": "float", "DEFAULT": 0.25} ] }*/
        void main() { gl_FragColor = IMG_THIS_PIXEL(inputImage) * amount; }"#;
    const GENERATOR: &str = r#"/*{ "INPUTS": [] }*/ void main() { gl_FragColor = vec4(1.0); }"#;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("openvj-fx-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(path: &Path, src: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, src).unwrap();
    }

    #[test]
    fn scans_filters_skips_generators_and_reports_errors() {
        let a = temp_dir("scan-a");
        let b = temp_dir("scan-b");
        write(&a.join("Glow Lines.fs"), FILTER);
        write(&a.join("sub/zoom_fx.fs"), FILTER);
        write(&a.join("sub/plasma.fs"), GENERATOR);
        write(&a.join("broken.fs"), &FILTER.replace("* amount", "* nope"));
        write(&a.join("header.fs"), "/* { not json */ void main() {}");
        write(&a.join("blur.fs"), FILTER); // a built-in id
        write(&a.join("vert.fs"), FILTER);
        write(&a.join("vert.vs"), "void main() {}");
        write(&a.join("notes.txt"), "not an effect");
        write(&b.join("zoom_fx.fs"), FILTER); // loses to the one in `a`

        let mut scanner = Scanner::new(vec![a.clone(), b.clone(), a.join("missing")], no_nope);
        let r = scanner.scan().unwrap();
        let ids: Vec<_> = r.effects.iter().map(|t| t.id()).collect();
        assert_eq!(ids, ["Glow Lines", "zoom_fx"]);
        assert_eq!(r.effects[0].name(), "Glow Lines");
        assert_eq!(r.effects[1].path.as_deref(), Some(a.join("sub/zoom_fx.fs").as_path()));
        let gens: Vec<_> = r.generators.iter().map(|t| t.id()).collect();
        assert_eq!(gens, ["plasma"]);
        let errors: Vec<_> = r.errors.iter().map(|e| (e.name.as_str(), e.message.as_str())).collect();
        assert_eq!(errors.len(), 5, "{errors:?}");
        let find = |name: &str| errors.iter().find(|e| e.0 == name).unwrap_or_else(|| panic!("{name}: {errors:?}")).1;
        assert!(find("Broken").contains("nope"));
        assert!(find("header").contains("ISF header"));
        assert!(find("Blur").contains("built-in"));
        assert!(find("Vert").contains(".vs"));
        assert!(find("Zoom Fx").contains("already used"));

        // Nothing changed: no new result.
        assert!(scanner.scan().is_none());
        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b);
    }

    #[test]
    fn rescans_only_changed_files() {
        let d = temp_dir("rescan");
        write(&d.join("one.fs"), FILTER);
        write(&d.join("two.fs"), FILTER);
        let mut scanner = Scanner::new(vec![d.clone()], ok);
        let first = scanner.scan().unwrap();
        assert_eq!(first.effects.len(), 2);

        // Change one file (different length, so the stamp changes even on coarse clocks).
        write(&d.join("two.fs"), &FILTER.replace("0.25", "0.5"));
        let second = scanner.scan().unwrap();
        let v = |r: &ScanResult, id: &str| r.effects.iter().find(|t| t.id() == id).unwrap().version;
        assert_eq!(v(&first, "one"), v(&second, "one"), "unchanged files aren't reloaded");
        assert_ne!(v(&first, "two"), v(&second, "two"));

        std::fs::remove_file(d.join("one.fs")).unwrap();
        let third = scanner.scan().unwrap();
        assert_eq!(third.effects.len(), 1);

        // An empty folder still reports once, then stays quiet.
        let empty = temp_dir("empty");
        let mut s = Scanner::new(vec![empty.clone()], ok);
        assert!(s.scan().unwrap().effects.is_empty());
        assert!(s.scan().is_none());
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_dir_all(&empty);
    }

    #[test]
    fn swapping_the_library_keeps_effect_values() {
        let d = temp_dir("swap");
        write(&d.join("swap_test_fx.fs"), FILTER);
        let mut scanner = Scanner::new(vec![d.clone()], ok);
        let r = scanner.scan().unwrap();
        set_library(r.into_library());
        let mut fx = crate::Effect::new("swap_test_fx");
        assert_eq!(fx.params["amount"].value, 0.25);
        fx.params.get_mut("amount").unwrap().value = 0.9;
        let before = fx.ty().unwrap().version;
        assert!(crate::library().all().len() > 1 && crate::library().get("blur").is_some());

        // Edit the file: same effect, new version, value kept.
        write(&d.join("swap_test_fx.fs"), &FILTER.replace("Test", "Edited test"));
        let r = scanner.scan().unwrap();
        set_library(r.into_library());
        let ty = fx.ty().unwrap();
        assert_ne!(ty.version, before);
        assert_eq!(ty.def.description, "Edited test");
        assert_eq!(fx.values(&crate::ModInputs::default()), [0.9]);
        let _ = std::fs::remove_dir_all(&d);
    }
}
