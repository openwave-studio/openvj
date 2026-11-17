//! The Player layout's engine side: a playlist of presets played by [`Automix`] on a
//! small composition of its own (two layers), so the Mixer's composition is never touched.

use std::path::{Path, PathBuf};

use openvj_core::{
    Automix, AutomixFrame, BlendMode, Clip, ClipId, ClipSource, Composition, ModInputs, OUTSIDE, PLAYLIST_EXTENSION,
    Playing, Playlist, Preset, SongDetector, Span,
};

/// A playlist in the playlists folder, for the Player's picker.
#[derive(Debug, Clone, PartialEq)]
pub struct FolderEntry {
    pub name: String,
    pub path: PathBuf,
    pub presets: usize,
}

/// Clip ids for the Player's two layers, far above any composition's, so textures and
/// effect state the engine keeps per clip never mix with the Mixer's clips.
const PLAYER_IDS: u64 = 1 << 60;

pub struct PlayerState {
    pub playlist: Playlist,
    /// Where the playlist was opened from or saved to.
    pub file: Option<PathBuf>,
    /// Running while Automix is on.
    automix: Option<Automix>,
    /// Two layers, mixed with Add so their opacities sum to a clean crossfade.
    comp: Composition,
    /// Preset each layer shows, and the layer holding the current preset.
    showing: [Option<usize>; 2],
    current_layer: usize,
    next_id: u64,
    /// The Player's UI needs refreshing.
    pub dirty: bool,
    /// The playlist changed and needs saving (see [`PlayerState::autosave`]).
    edited: bool,
    /// Where playlists live, and what is there (listed again when `folder_dirty`).
    pub dir: Option<PathBuf>,
    pub folder: Vec<FolderEntry>,
    folder_dirty: bool,
    songs: SongDetector,
    /// The last-saved playlist was opened (or there was none), once at launch.
    reopened: bool,
    /// The row under the mouse, playing on the Preview.
    pub hover: Option<usize>,
}

impl Default for PlayerState {
    fn default() -> Self {
        let mut comp = Composition::new(1280, 720, 2, 1);
        for layer in &mut comp.layers {
            layer.blend = BlendMode::Add;
            layer.transition_secs = 0.0;
        }
        Self {
            playlist: Playlist::default(),
            file: None,
            automix: None,
            comp,
            showing: [None; 2],
            current_layer: 0,
            next_id: PLAYER_IDS,
            dirty: true,
            edited: false,
            dir: openvj_core::playlists_dir(),
            folder: Vec::new(),
            folder_dirty: true,
            songs: SongDetector::new(),
            reopened: false,
            hover: None,
        }
    }
}

impl PlayerState {
    pub fn is_live(&self) -> bool {
        self.automix.is_some()
    }

    /// How far the current preset has played: (0..1, seconds), if Automix is on.
    pub fn played(&self, now: f64, beat: f64) -> Option<(f32, f64)> {
        self.automix.as_ref().map(|a| a.played(&self.playlist, now, beat))
    }

    /// Index of the preset playing now, if Automix is on.
    pub fn current(&self) -> Option<usize> {
        self.automix.as_ref().map(Automix::current)
    }

    /// Adds a copy of a clip as a preset at the end of the playlist. Live inputs and video
    /// files aren't supported in the Player yet.
    pub fn add(&mut self, clip: &Clip) -> Result<(), String> {
        match clip.source {
            ClipSource::Camera { .. } | ClipSource::Syphon { .. } => {
                return Err("Live inputs can't go in a playlist.".into());
            }
            ClipSource::Video { .. } | ClipSource::Image { .. } => {
                return Err("Only generators can go in a playlist for now; videos and images come later.".into());
            }
            _ => {}
        }
        self.playlist.presets.push(Preset::new(clip.clone()));
        self.edited();
        Ok(())
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.playlist.presets.len() {
            self.playlist.presets.remove(index);
            if self.playlist.presets.is_empty() {
                self.stop();
            }
            // Indices moved: show things again from scratch on the next frame.
            self.showing = [None; 2];
            self.edited();
        }
    }

    pub fn move_preset(&mut self, from: usize, to: usize) {
        let n = self.playlist.presets.len();
        if from < n && to < n && from != to {
            let p = self.playlist.presets.remove(from);
            self.playlist.presets.insert(to, p);
            self.showing = [None; 2];
            self.edited();
        }
    }

    pub fn set_play_for(&mut self, index: usize, span: Span) {
        if let Some(p) = self.playlist.presets.get_mut(index) {
            p.play_for = span;
            self.edited();
        }
    }

    /// Starts Automix. Returns false for an empty playlist.
    pub fn start(&mut self, now: f64, beat: f64, bpm: f32) -> bool {
        let seed = (now * 1000.0) as u64;
        self.automix = Automix::start(&self.playlist, now, beat, bpm, seed);
        self.showing = [None; 2];
        self.dirty = true;
        self.automix.is_some()
    }

    pub fn stop(&mut self) {
        if self.automix.take().is_some() {
            self.dirty = true;
        }
        for layer in &mut self.comp.layers {
            layer.active = None;
            layer.clips[0] = None;
        }
        self.showing = [None; 2];
    }

    pub fn next(&mut self, now: f64, beat: f64) {
        if let Some(a) = &mut self.automix {
            a.next(&self.playlist, now, beat);
        }
    }

    /// Brings back the preset that played before. False when there is none.
    pub fn back(&mut self, now: f64, beat: f64) -> bool {
        self.automix.as_mut().is_some_and(|a| a.back(&self.playlist, now, beat))
    }

    /// Plays preset `index`: with the transition while playing, else starts Automix there.
    pub fn jump(&mut self, index: usize, now: f64, beat: f64, bpm: f32) {
        if index >= self.playlist.presets.len() {
            return;
        }
        match &mut self.automix {
            Some(a) => a.jump(&self.playlist, index, now, beat),
            None => {
                let seed = (now * 1000.0) as u64;
                self.automix = Automix::start_at(&self.playlist, index, now, beat, bpm, seed);
                self.showing = [None; 2];
            }
        }
        self.dirty = true;
    }

    /// Listens for a new song while Automix plays; a preset set to "Until the song changes"
    /// then moves on.
    pub fn listen(&mut self, inputs: &ModInputs, bpm: f32, dt: f32, now: f64, beat: f64) {
        let Some(a) = &mut self.automix else { return };
        if self.songs.update(inputs, bpm, dt).is_some() && a.song_changed(&self.playlist, now, beat) {
            self.dirty = true;
        }
    }

    /// Notes a change to the playlist, saved by [`PlayerState::autosave`].
    pub fn edited(&mut self) {
        self.edited = true;
        self.dirty = true;
    }

    /// Saves an edited playlist: to its file, or a new one in the playlists folder.
    pub fn autosave(&mut self) -> Result<(), String> {
        if !std::mem::take(&mut self.edited) || self.playlist.presets.is_empty() && self.file.is_none() {
            return Ok(());
        }
        let path = match (&self.file, &self.dir) {
            (Some(f), _) => f.clone(),
            (None, Some(dir)) => {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
                let name =
                    if self.playlist.name.is_empty() { "Playlist".to_string() } else { self.playlist.name.clone() };
                free_path(dir, &name)
            }
            (None, None) => return Ok(()),
        };
        self.save(&path)
    }

    /// Starts an empty playlist; it gets a file in the playlists folder once it has a preset.
    pub fn new_playlist(&mut self) {
        self.stop();
        let mut name = "New playlist".to_string();
        let mut n = 2;
        while self.folder.iter().any(|e| e.name == name) {
            name = format!("New playlist {n}");
            n += 1;
        }
        self.playlist = Playlist { name, ..Playlist::default() };
        self.file = None;
        self.dirty = true;
    }

    /// Opens another playlist. While Automix plays, its first preset comes in over the
    /// current look with the new playlist's transition.
    pub fn switch(&mut self, path: &Path, now: f64, beat: f64, bpm: f32) -> Result<(), String> {
        let list = read(path)?;
        if !self.is_live() || list.presets.is_empty() {
            self.stop();
            self.playlist = list;
            self.file = Some(path.to_owned());
            self.dirty = true;
            return Ok(());
        }
        self.playlist = list;
        self.file = Some(path.to_owned());
        // What's on screen belongs to the old playlist now.
        self.showing = self.showing.map(|s| s.map(|_| OUTSIDE));
        let seed = (now * 1000.0) as u64;
        self.automix = Automix::start(&self.playlist, now, beat, bpm, seed);
        if let Some(a) = &mut self.automix {
            a.handover(now, beat);
        }
        self.dirty = true;
        Ok(())
    }

    /// Lists the playlists folder again if something there changed.
    pub fn refresh_folder(&mut self) -> bool {
        if !std::mem::take(&mut self.folder_dirty) {
            return false;
        }
        self.folder = self.dir.as_deref().map(list_folder).unwrap_or_default();
        // At launch the Player opens the playlist saved last, ready to play.
        if !std::mem::replace(&mut self.reopened, true) && self.file.is_none() && self.playlist.presets.is_empty() {
            let last = self.dir.as_deref().and_then(last_saved);
            if let Some(path) = last {
                let _ = self.open(&path);
            }
        }
        true
    }

    /// Advances Automix and returns the composition to render, or `None` when the Player
    /// isn't live. `main` lends the master level and size.
    pub fn frame(
        &mut self,
        main: &Composition,
        now: f64,
        beat: f64,
        bpm: f32,
        inputs: &ModInputs,
    ) -> Option<&Composition> {
        let automix = self.automix.as_mut()?;
        let before = automix.current();
        let frame = automix.tick(&self.playlist, now, beat, bpm);
        if frame.current != before {
            self.dirty = true;
        }
        self.show(frame, now, beat);
        let (w_prev, w_cur) = frame.weights(self.playlist.style);
        self.comp.layers[self.current_layer].opacity = w_cur;
        self.comp.layers[1 - self.current_layer].opacity =
            if self.showing[1 - self.current_layer].is_some() { w_prev } else { 0.0 };
        self.comp.master = main.master;
        self.comp.master_mod = main.master_mod;
        self.comp.reactivity = main.reactivity;
        self.comp.width = main.width;
        self.comp.height = main.height;
        self.comp.bpm = main.bpm;
        self.comp.tick(now, beat);
        for clip in self.comp.layers.iter_mut().flat_map(|l| l.clips.iter_mut().flatten()) {
            clip.apply_macros(inputs);
        }
        Some(&self.comp)
    }

    /// Puts the current preset on a layer (the free one, so the outgoing preset keeps
    /// running where it was) and keeps the outgoing one only during the transition.
    fn show(&mut self, frame: AutomixFrame, now: f64, beat: f64) {
        if self.showing[self.current_layer] != Some(frame.current) {
            let target =
                if self.showing[self.current_layer].is_some() { 1 - self.current_layer } else { self.current_layer };
            self.load(target, frame.current, now, beat);
            self.current_layer = target;
        }
        let other = 1 - self.current_layer;
        if frame.previous.is_none() && self.showing[other].is_some() {
            self.comp.layers[other].active = None;
            self.comp.layers[other].clips[0] = None;
            self.showing[other] = None;
        }
    }

    fn load(&mut self, layer: usize, preset: usize, now: f64, beat: f64) {
        let Some(p) = self.playlist.presets.get(preset) else { return };
        let mut clip = p.clip.clone();
        clip.id = ClipId(self.next_id);
        self.next_id += 1;
        let l = &mut self.comp.layers[layer];
        l.active = Some(Playing::new(clip.id, now, beat));
        l.fade = None;
        l.clips[0] = Some(clip);
        self.showing[layer] = Some(preset);
    }

    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        let list = read(path)?;
        self.stop();
        self.playlist = list;
        self.file = Some(path.to_owned());
        self.dirty = true;
        Ok(())
    }

    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        if self.playlist.name.is_empty() {
            self.playlist.name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        }
        let json = self.playlist.to_json().map_err(|e| e.to_string())?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, json).and_then(|_| std::fs::rename(&tmp, path)).map_err(|e| e.to_string())?;
        self.file = Some(path.to_owned());
        self.edited = false;
        self.dirty = true;
        self.folder_dirty = true;
        Ok(())
    }
}

fn read(path: &Path) -> Result<Playlist, String> {
    let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut list = Playlist::from_json(&json)?;
    if list.name.is_empty() {
        list.name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    }
    Ok(list)
}

/// A path in `dir` for a playlist called `name` that isn't taken yet.
fn free_path(dir: &Path, name: &str) -> PathBuf {
    let clean: String = name.chars().map(|c| if matches!(c, '/' | '\\' | ':') { '-' } else { c }).collect();
    let mut path = dir.join(format!("{clean}.{PLAYLIST_EXTENSION}"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{clean} {n}.{PLAYLIST_EXTENSION}"));
        n += 1;
    }
    path
}

/// The playlists in a folder, by name.
fn list_folder(dir: &Path) -> Vec<FolderEntry> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<FolderEntry> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == PLAYLIST_EXTENSION))
        .filter_map(|p| {
            let list = read(&p).ok()?;
            Some(FolderEntry { name: list.name, presets: list.presets.len(), path: p })
        })
        .collect();
    out.sort_by_key(|e| e.name.to_lowercase());
    out
}

/// The playlist in a folder saved most recently.
fn last_saved(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == PLAYLIST_EXTENSION))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(name: &str) -> Clip {
        let mut comp = Composition::new(64, 64, 1, 1);
        comp.set_clip(0, 0, ClipSource::isf(name)).unwrap();
        comp.clip(0, 0).unwrap().clone()
    }

    #[test]
    fn crossfades_between_its_two_layers() {
        let main = Composition::default();
        let mut p = PlayerState::default();
        p.playlist.transition = Span::Bars(1.0);
        for n in ["a", "b"] {
            p.add(&clip(n)).unwrap();
            p.set_play_for(p.playlist.presets.len() - 1, Span::Bars(2.0));
        }
        assert!(p.start(0.0, 0.0, 120.0));
        let c = p.frame(&main, 0.5, 1.0, 120.0, &ModInputs::default()).unwrap();
        assert_eq!(c.layers[0].opacity, 1.0);
        assert!(c.layers[1].active.is_none());
        // Two bars in, plus half a bar: b comes in on layer 1 while a keeps playing on layer 0.
        let c = p.frame(&main, 5.0, 10.0, 120.0, &ModInputs::default()).unwrap();
        assert!((c.layers[1].opacity - 0.5).abs() < 1e-6);
        assert!((c.layers[0].opacity - 0.5).abs() < 1e-6);
        assert!(c.layers[0].active.is_some() && c.layers[1].active.is_some());
        // Transition over: a is gone.
        let c = p.frame(&main, 6.25, 12.5, 120.0, &ModInputs::default()).unwrap();
        assert!(c.layers[0].active.is_none());
        assert_eq!(c.layers[1].opacity, 1.0);
        p.stop();
        assert!(p.frame(&main, 7.0, 14.0, 120.0, &ModInputs::default()).is_none());
    }

    #[test]
    fn player_clip_ids_never_clash_with_the_composition() {
        let main = Composition::default();
        let mut p = PlayerState::default();
        p.add(&clip("a")).unwrap();
        p.start(0.0, 0.0, 120.0);
        let c = p.frame(&main, 0.0, 0.0, 120.0, &ModInputs::default()).unwrap();
        assert!(c.all_clips().all(|c| c.id.0 >= PLAYER_IDS));
    }

    #[test]
    fn saves_and_opens_playlists() {
        let dir = std::env::temp_dir().join(format!("openvj-playlist-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Friday.ovjplaylist");
        let mut p = PlayerState::default();
        p.add(&clip("a")).unwrap();
        p.save(&path).unwrap();
        let mut q = PlayerState::default();
        q.open(&path).unwrap();
        assert_eq!(q.playlist.name, "Friday");
        assert_eq!(q.playlist.presets.len(), 1);
        std::fs::remove_dir_all(dir).ok();
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("openvj-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        dir
    }

    #[test]
    fn edits_save_themselves_in_the_playlists_folder() {
        let dir = temp_dir("autosave");
        let mut p = PlayerState { dir: Some(dir.clone()), ..PlayerState::default() };
        p.new_playlist();
        p.autosave().unwrap();
        assert!(!dir.exists(), "an empty new playlist isn't saved");
        p.add(&clip("a")).unwrap();
        p.autosave().unwrap();
        let path = dir.join("New playlist.ovjplaylist");
        assert!(path.exists());
        p.set_play_for(0, Span::Song);
        p.autosave().unwrap();
        assert!(std::fs::read_to_string(&path).unwrap().contains("\"song\""));
        assert!(p.refresh_folder());
        let mut later = PlayerState { dir: Some(dir.clone()), ..PlayerState::default() };
        later.refresh_folder();
        assert_eq!(later.file.as_deref(), Some(path.as_path()), "reopens the last saved playlist");
        assert_eq!(p.folder, vec![FolderEntry { name: "New playlist".into(), path: path.clone(), presets: 1 }]);
        // The next new playlist gets its own name and file.
        p.new_playlist();
        assert_eq!(p.playlist.name, "New playlist 2");
        p.add(&clip("b")).unwrap();
        p.autosave().unwrap();
        assert!(dir.join("New playlist 2.ovjplaylist").exists());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn switching_playlists_while_playing_crossfades() {
        let dir = temp_dir("switch");
        std::fs::create_dir_all(&dir).unwrap();
        let other = dir.join("Peak.ovjplaylist");
        let mut q = PlayerState::default();
        q.playlist.transition = Span::Bars(1.0); // the new playlist's transition is used
        q.add(&clip("x")).unwrap();
        q.save(&other).unwrap();

        let main = Composition::default();
        let mut p = PlayerState::default();
        p.playlist.transition = Span::Bars(1.0);
        p.add(&clip("a")).unwrap();
        p.start(0.0, 0.0, 120.0);
        p.frame(&main, 1.0, 2.0, 120.0, &ModInputs::default());
        p.switch(&other, 2.0, 4.0, 120.0).unwrap();
        // Half a bar later both looks are on, half each.
        let c = p.frame(&main, 3.0, 6.0, 120.0, &ModInputs::default()).unwrap();
        let mut opacities: Vec<f32> = c.layers.iter().filter(|l| l.active.is_some()).map(|l| l.opacity).collect();
        opacities.sort_by(f32::total_cmp);
        assert_eq!(opacities.len(), 2);
        assert!((opacities[0] - 0.5).abs() < 1e-6 && (opacities[1] - 0.5).abs() < 1e-6);
        let c = p.frame(&main, 4.5, 9.0, 120.0, &ModInputs::default()).unwrap();
        assert_eq!(c.layers.iter().filter(|l| l.active.is_some()).count(), 1);
        assert_eq!(p.playlist.name, "Peak");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_click_on_a_row_starts_there() {
        let main = Composition::default();
        let mut p = PlayerState::default();
        for n in ["a", "b", "c"] {
            p.add(&clip(n)).unwrap();
        }
        p.jump(2, 0.0, 0.0, 120.0);
        assert_eq!(p.current(), Some(2));
        assert!(p.frame(&main, 0.1, 0.2, 120.0, &ModInputs::default()).is_some());
        p.jump(0, 1.0, 2.0, 120.0);
        assert!(p.back(2.0, 4.0));
        assert_eq!(p.current(), Some(2));
    }
}
