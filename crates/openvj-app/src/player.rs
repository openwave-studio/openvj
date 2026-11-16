//! The Player layout's engine side: a playlist of presets played by [`Automix`] on a
//! small composition of its own (two layers), so the Mixer's composition is never touched.

use std::path::{Path, PathBuf};

use openvj_core::{
    Automix, AutomixFrame, BlendMode, Clip, ClipId, ClipSource, Composition, ModInputs, Playing, Playlist, Preset, Span,
};

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
        }
    }
}

impl PlayerState {
    pub fn is_live(&self) -> bool {
        self.automix.is_some()
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
        self.dirty = true;
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
            self.dirty = true;
        }
    }

    pub fn move_preset(&mut self, from: usize, to: usize) {
        let n = self.playlist.presets.len();
        if from < n && to < n && from != to {
            let p = self.playlist.presets.remove(from);
            self.playlist.presets.insert(to, p);
            self.showing = [None; 2];
            self.dirty = true;
        }
    }

    pub fn set_play_for(&mut self, index: usize, span: Span) {
        if let Some(p) = self.playlist.presets.get_mut(index) {
            p.play_for = span;
            self.dirty = true;
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
        l.active = Some(Playing { clip_id: clip.id, started_at: now, started_beat: beat });
        l.fade = None;
        l.clips[0] = Some(clip);
        self.showing[layer] = Some(preset);
    }

    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut list = Playlist::from_json(&json)?;
        if list.name.is_empty() {
            list.name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        }
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
        std::fs::write(path, json).map_err(|e| e.to_string())?;
        self.file = Some(path.to_owned());
        self.dirty = true;
        Ok(())
    }
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
}
