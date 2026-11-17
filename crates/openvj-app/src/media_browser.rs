//! The Synthesis layout's media browser: chips for All, Live sources and each media folder
//! the VJ added, and a grid of thumbnails to pick a layer's media overlay from. Folder
//! listings come from the file browser's; thumbnails are made one at a time on a
//! background thread and kept in memory.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use openvj_core::ClipSource;

use crate::browser;

/// Chips before the folders.
pub const CHIP_ALL: usize = 0;
pub const CHIP_LIVE: usize = 1;
const FIRST_FOLDER: usize = 2;
/// Thumbnail size: small, so a folder of large images costs little.
pub const THUMB: (u32, u32) = (128, 72);

/// One tile.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub name: String,
    pub source: ClipSource,
}

impl Item {
    pub fn path(&self) -> Option<&Path> {
        self.source.media_path()
    }
}

/// A finished thumbnail: the file and its RGBA pixels, or `None` when it couldn't be read.
pub type Thumb = (PathBuf, Option<(u32, u32, Vec<u8>)>);

pub struct MediaBrowser {
    pub folders: Vec<PathBuf>,
    pub chip: usize,
    /// While on, clicks don't change the overlay.
    pub locked: bool,
    pub items: Vec<Item>,
    jobs: Option<Sender<PathBuf>>,
    pub done: Option<Receiver<Thumb>>,
    asked: HashSet<PathBuf>,
}

impl Default for MediaBrowser {
    fn default() -> Self {
        Self {
            folders: Vec::new(),
            chip: CHIP_ALL,
            locked: false,
            items: Vec::new(),
            jobs: None,
            done: None,
            asked: HashSet::new(),
        }
    }
}

/// A folder's chip label: its own name.
pub fn folder_label(dir: &Path) -> String {
    dir.file_name().map_or_else(|| dir.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// Images and videos in `dir`, not its subfolders, sorted by name.
fn media_in(dir: &Path) -> Vec<Item> {
    browser::list(dir)
        .into_iter()
        .filter_map(|e| match e.source {
            Some(s @ (ClipSource::Image { .. } | ClipSource::Video { .. })) => Some(Item { name: e.name, source: s }),
            _ => None,
        })
        .collect()
}

impl MediaBrowser {
    pub fn load() -> Self {
        Self { folders: crate::prefs::media_folders(), ..Self::default() }
    }

    pub fn chip_labels(&self) -> Vec<String> {
        let mut v = vec!["all".to_owned(), "live sources".to_owned()];
        v.extend(self.folders.iter().map(|f| folder_label(f)));
        v
    }

    /// Lists the open chip again: `live` are the cameras and Syphon servers there are now.
    pub fn refresh(&mut self, live: &[ClipSource]) {
        self.chip = self.chip.min(FIRST_FOLDER + self.folders.len()).min(self.chip_labels().len() - 1);
        self.items = match self.chip {
            CHIP_ALL => self.folders.iter().flat_map(|f| media_in(f)).collect(),
            CHIP_LIVE => live
                .iter()
                .filter(|s| matches!(s, ClipSource::Camera { .. } | ClipSource::Syphon { .. }))
                .map(|s| Item { name: s.live_name().unwrap_or_else(|| s.default_name()), source: s.clone() })
                .collect(),
            n => self.folders.get(n - FIRST_FOLDER).map(|f| media_in(f)).unwrap_or_default(),
        };
    }

    /// Adds a folder (once) and opens its chip.
    pub fn add_folder(&mut self, dir: PathBuf) {
        let at = match self.folders.iter().position(|f| *f == dir) {
            Some(i) => i,
            None => {
                self.folders.push(dir);
                crate::prefs::set_media_folders(&self.folders);
                self.folders.len() - 1
            }
        };
        self.chip = FIRST_FOLDER + at;
    }

    /// Takes the open folder off the list (its files stay where they are).
    pub fn remove_folder(&mut self, chip: usize) {
        if chip >= FIRST_FOLDER && chip - FIRST_FOLDER < self.folders.len() {
            self.folders.remove(chip - FIRST_FOLDER);
            crate::prefs::set_media_folders(&self.folders);
            self.chip = CHIP_ALL;
        }
    }

    /// The item before or after the one at `path` in the open chip (wrapping), for the
    /// overlay's previous and next buttons.
    pub fn step(&self, path: Option<&Path>, by: isize) -> Option<&Item> {
        let files: Vec<&Item> = self.items.iter().filter(|i| i.path().is_some()).collect();
        if files.is_empty() {
            return None;
        }
        let n = files.len() as isize;
        let at = path.and_then(|p| files.iter().position(|i| i.path() == Some(p)));
        let next = match at {
            Some(i) => (i as isize + by).rem_euclid(n),
            None if by < 0 => n - 1,
            None => 0,
        };
        files.get(next as usize).copied()
    }

    /// Asks for thumbnails of the files shown that have none yet.
    pub fn want_thumbs(&mut self, have: &HashMap<PathBuf, slint::Image>) {
        let wanted: Vec<PathBuf> = self
            .items
            .iter()
            .filter_map(|i| i.path())
            .filter(|p| !have.contains_key(*p) && !self.asked.contains(*p))
            .map(Path::to_owned)
            .collect();
        if wanted.is_empty() {
            return;
        }
        let jobs = self.jobs.get_or_insert_with(|| {
            let (job_tx, job_rx) = mpsc::channel::<PathBuf>();
            let (done_tx, done_rx) = mpsc::channel::<Thumb>();
            std::thread::Builder::new()
                .name("media thumbnails".into())
                .spawn(move || {
                    for path in job_rx {
                        let is_video = matches!(ClipSource::from_path(&path), Some(ClipSource::Video { .. }));
                        let thumb = openvj_media::load_media(&path, is_video, (THUMB.0 * 2, THUMB.1 * 2), THUMB)
                            .ok()
                            .map(|m| (m.thumb.width, m.thumb.height, m.thumb.rgba));
                        if done_tx.send((path, thumb)).is_err() {
                            break;
                        }
                    }
                })
                .expect("can start the thumbnail thread");
            self.done = Some(done_rx);
            job_tx
        });
        for p in wanted {
            self.asked.insert(p.clone());
            let _ = jobs.send(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str) -> Item {
        Item { name: name.into(), source: ClipSource::Image { path: PathBuf::from(name) } }
    }

    #[test]
    fn step_wraps_and_starts_at_the_ends() {
        let b = MediaBrowser { items: vec![item("a.png"), item("b.png"), item("c.png")], ..Default::default() };
        assert_eq!(b.step(Some(Path::new("c.png")), 1).unwrap().name, "a.png");
        assert_eq!(b.step(Some(Path::new("a.png")), -1).unwrap().name, "c.png");
        assert_eq!(b.step(None, 1).unwrap().name, "a.png");
        assert_eq!(b.step(None, -1).unwrap().name, "c.png");
    }

    #[test]
    fn lists_only_images_and_videos() {
        let dir = std::env::temp_dir().join(format!("openvj-media-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        for f in ["b.png", "a.mp4", "notes.txt"] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        let names: Vec<String> = media_in(&dir).into_iter().map(|i| i.name).collect();
        assert_eq!(names, ["a.mp4", "b.png"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
