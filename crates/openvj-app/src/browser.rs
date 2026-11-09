//! File browser listing: folders plus media files openVJ can play.

use std::path::{Path, PathBuf};

use openvj_core::ClipSource;

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    /// `None` for folders.
    pub source: Option<ClipSource>,
    pub detail: String,
}

impl Entry {
    pub fn kind(&self) -> &'static str {
        self.source.as_ref().map_or("dir", ClipSource::kind_label)
    }
}

/// Lists folders first, then playable media, both sorted case-insensitively. Hidden entries are skipped.
pub fn list(dir: &Path) -> Vec<Entry> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut entries: Vec<Entry> = read
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                return None;
            }
            let path = e.path();
            if path.is_dir() {
                return Some(Entry { name, path, source: None, detail: String::new() });
            }
            let source = ClipSource::from_path(&path)?;
            let detail = e.metadata().map(|m| human_size(m.len())).unwrap_or_default();
            Some(Entry { name, path, source: Some(source), detail })
        })
        .collect();
    entries.sort_by_key(|e| (e.source.is_some(), e.name.to_lowercase()));
    entries
}

/// Where the browser starts: `OPENVJ_MEDIA_DIR`, else the user's Movies/Videos folder, else home.
pub fn default_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("OPENVJ_MEDIA_DIR") {
        return dir.into();
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    let Some(home) = home else { return PathBuf::from(".") };
    ["Movies", "Videos"].iter().map(|d| home.join(d)).find(|p| p.is_dir()).unwrap_or(home)
}

/// A path for display: home shown as `~`, and long paths cut from the front to at most
/// `max` characters so the file and folder names stay visible.
pub fn short_path(path: &Path, max: usize) -> String {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let shown = match home.as_deref().and_then(|h| path.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    };
    let chars: Vec<char> = shown.chars().collect();
    if chars.len() <= max {
        return shown;
    }
    let tail: String = chars[chars.len() - (max - 1)..].iter().collect();
    // Prefer to cut at a separator.
    match tail.find('/') {
        Some(i) if i + 1 < tail.len() => format!("…{}", &tail[i..]),
        _ => format!("…{tail}"),
    }
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 { format!("{bytes} B") } else { format!("{size:.1} {}", UNITS[unit]) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_dirs_then_media_and_skips_the_rest() {
        let dir = std::env::temp_dir().join(format!("openvj-browser-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Loops")).unwrap();
        std::fs::create_dir_all(dir.join(".hidden")).unwrap();
        for f in ["b.mov", "A.png", "notes.txt", ".DS_Store"] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        let names: Vec<_> = list(&dir).into_iter().map(|e| (e.name.clone(), e.kind())).collect();
        assert_eq!(names, vec![("Loops".into(), "dir"), ("A.png".into(), "image"), ("b.mov".into(), "video")]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn short_paths_keep_the_end() {
        assert_eq!(short_path(Path::new("/a/b/c.mov"), 40), "/a/b/c.mov");
        assert_eq!(short_path(Path::new("/very/long/folder/name/clips/loop.mov"), 20), "…/clips/loop.mov");
    }

    #[test]
    fn sizes_are_human_readable() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(3 * 1024 * 1024 * 1024), "3.0 GB");
    }
}
