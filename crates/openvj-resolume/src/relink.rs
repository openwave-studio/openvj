//! Finds media whose saved path no longer exists, by file name, under folders the user chose.
//!
//! Resolume stores absolute paths, and compositions usually move between machines (or from
//! macOS to Windows), so most imported clips point at files that aren't there any more.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use openvj_core::{ClipSource, Composition};

/// How deep to look below each search folder.
const MAX_DEPTH: usize = 6;
/// Stop scanning after this many directory entries, so pointing at a huge drive stays quick.
const MAX_ENTRIES: usize = 50_000;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RelinkResult {
    /// Clips whose media was found under one of the search folders.
    pub relinked: usize,
    /// Saved paths that are still missing, one entry per file.
    pub missing: Vec<PathBuf>,
}

/// The last component of a path saved on any OS. `Path::file_name` doesn't split on `\` on
/// macOS and Linux, and Windows paths are common in shared compositions.
pub fn file_name(path: &str) -> Option<&str> {
    path.rsplit(['/', '\\']).next().filter(|s| !s.is_empty())
}

/// Points every clip, in every deck, whose media file doesn't exist at a file with the same name found under
/// `roots`. Matching ignores case. When several files share a name, the shallowest one wins.
pub fn relink_missing(comp: &mut Composition, roots: &[PathBuf]) -> RelinkResult {
    let missing_names: Vec<String> = media_paths(comp)
        .filter(|p| !p.exists())
        .filter_map(|p| file_name(&p.to_string_lossy()).map(str::to_lowercase))
        .collect();
    let mut result = RelinkResult::default();
    if missing_names.is_empty() {
        return result;
    }
    let found = index_files(roots, &missing_names);

    for clip in comp.every_clip_mut() {
        let (ClipSource::Image { path } | ClipSource::Video { path }) = &mut clip.source else { continue };
        if path.exists() {
            continue;
        }
        let name = file_name(&path.to_string_lossy()).map(str::to_lowercase);
        match name.and_then(|n| found.get(&n)) {
            Some(new) => {
                *path = new.clone();
                result.relinked += 1;
            }
            None if !result.missing.contains(path) => result.missing.push(path.clone()),
            None => {}
        }
    }
    result
}

fn media_paths(comp: &Composition) -> impl Iterator<Item = &Path> {
    comp.every_clip().filter_map(|c| c.source.media_path())
}

/// Breadth-first walk of `roots`, keeping the first (shallowest) file for each wanted name.
fn index_files(roots: &[PathBuf], wanted: &[String]) -> HashMap<String, PathBuf> {
    let mut found = HashMap::new();
    let mut queue: Vec<(PathBuf, usize)> = roots.iter().map(|r| (r.clone(), 0)).collect();
    let mut seen = 0;
    let mut i = 0;
    while i < queue.len() && found.len() < wanted.len() {
        let (dir, depth) = queue[i].clone();
        i += 1;
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            seen += 1;
            if seen > MAX_ENTRIES {
                return found;
            }
            let path = entry.path();
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_dir() {
                let hidden = entry.file_name().to_string_lossy().starts_with('.');
                if depth < MAX_DEPTH && !hidden {
                    queue.push((path, depth + 1));
                }
            } else {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if wanted.contains(&name) {
                    found.entry(name).or_insert(path);
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_name_splits_on_both_separators() {
        assert_eq!(file_name("/Users/a/clip.mov"), Some("clip.mov"));
        assert_eq!(file_name(r"C:\VJ\Loops\clip.mov"), Some("clip.mov"));
        assert_eq!(file_name("clip.mov"), Some("clip.mov"));
        assert_eq!(file_name("/trailing/"), None);
    }

    #[test]
    fn relinks_by_name_and_reports_the_rest() {
        let dir = std::env::temp_dir().join(format!("openvj-relink-{}", std::process::id()));
        let nested = dir.join("loops").join("hd");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("Found.MOV"), b"").unwrap();

        let mut comp = Composition::new(640, 360, 1, 3);
        comp.set_clip(0, 0, ClipSource::Video { path: r"D:\shows\found.mov".into() });
        comp.set_clip(0, 1, ClipSource::Video { path: "/old/machine/gone.mov".into() });
        comp.set_clip(0, 2, ClipSource::Video { path: "/old/machine/gone.mov".into() });

        let result = relink_missing(&mut comp, std::slice::from_ref(&dir));
        assert_eq!(result.relinked, 1);
        assert_eq!(result.missing, vec![PathBuf::from("/old/machine/gone.mov")]);
        assert_eq!(comp.clip(0, 0).unwrap().source.media_path(), Some(nested.join("Found.MOV").as_path()));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
