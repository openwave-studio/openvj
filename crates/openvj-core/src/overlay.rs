//! A layer's media overlay (Synthesis layout, like Synesthesia's media section): an image,
//! video or camera drawn over whatever the layer plays, with a blend mode and amount, its
//! own transport, placement (scale, fit or fill, flips) and colour. It belongs to the
//! layer, so it stays when the layer switches clips.

use serde::{Deserialize, Serialize};

use crate::{BlendMode, Clip, ClipId, ClipSource, ClipTime, Effect, Param, default_look};

/// Scale knob range: 0..1 is 0..400%.
pub const OVERLAY_MAX_SCALE: f32 = 4.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaOverlay {
    /// The media, as a clip: its source, speed and play mode. Never in the clip grid.
    pub clip: Clip,
    pub blend: BlendMode,
    /// How much of the overlay shows, 0..1.
    pub amount: Param,
    /// 0..1 is 0..[`OVERLAY_MAX_SCALE`]×.
    pub scale: Param,
    /// Fill the frame (crop) rather than fit inside it (letterbox).
    pub fill: bool,
    pub flip_v: bool,
    pub flip_h: bool,
    /// Colour: the META look, of which the overlay shows invert, contrast, gamma, hue and saturation.
    #[serde(default = "default_look")]
    pub look: Effect,
    #[serde(skip)]
    pub transport: Transport,
}

/// Where the overlay is in its media. Not saved: a loaded show starts its overlays playing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transport {
    pub playing: bool,
    /// Seconds and beats played so far (scrubbing moves them), fed to the clip's play mode.
    pub secs: f64,
    pub beats: f64,
}

impl Default for Transport {
    fn default() -> Self {
        Self { playing: true, secs: 0.0, beats: 0.0 }
    }
}

impl MediaOverlay {
    pub fn new(id: ClipId, source: ClipSource) -> Self {
        Self {
            clip: Clip::new(id, source),
            blend: BlendMode::Add,
            amount: Param::new(1.0),
            scale: Param::new(1.0 / OVERLAY_MAX_SCALE),
            fill: true,
            flip_v: false,
            flip_h: false,
            look: default_look(),
            transport: Transport::default(),
        }
    }

    /// Moves the transport on by `dt` seconds and `dbeats` beats, if playing.
    pub fn advance(&mut self, dt: f64, dbeats: f64) {
        if self.transport.playing {
            self.transport.secs += dt.max(0.0);
            self.transport.beats += dbeats.max(0.0);
        }
    }

    /// Where the media is now, after its play mode.
    pub fn time(&self) -> ClipTime {
        self.clip.time_at(self.transport.secs, self.transport.beats)
    }

    /// Position 0..1 through the media, for the scrub bar; 0 for media without a length.
    pub fn progress(&self) -> f32 {
        match self.clip.length() {
            Some(len) if len > 0.0 => (self.time().secs / len).clamp(0.0, 1.0) as f32,
            _ => 0.0,
        }
    }

    /// Jumps to `at` (0..1) through the media.
    pub fn scrub(&mut self, at: f32) {
        let Some(len) = self.clip.length() else { return };
        let speed = (self.clip.speed as f64).max(1e-3);
        self.transport.secs = at.clamp(0.0, 1.0) as f64 * len / speed;
        self.transport.beats = match self.clip.sync_beats {
            Some(b) => at as f64 * b as f64,
            None => 0.0,
        };
    }

    /// Stop: back to the start, paused.
    pub fn stop(&mut self) {
        self.transport = Transport { playing: false, ..Transport::default() };
    }

    /// Whether the overlay draws: it has some amount, and a play-once overlay that ended
    /// with "clear" is gone.
    pub fn visible(&self) -> bool {
        let cleared = self.clip.play_mode == crate::PlayMode::OnceClear && self.time().ended;
        !cleared && (self.amount.value > 0.0 || self.amount.modulation.is_active())
    }

    /// Scale, flips and fit as a UV scale for the media shader: `cover` is the fill scale
    /// (the media's own aspect against the frame's), as the renderer works it out.
    pub fn uv_scale(&self, cover: [f32; 2], scale: f32) -> [f32; 2] {
        let base = if self.fill {
            cover
        } else {
            // Fit: the long side touches the frame, so the short side shows more than 1.
            let m = cover[0].min(cover[1]).max(1e-6);
            [cover[0] / m, cover[1] / m]
        };
        let s = (scale * OVERLAY_MAX_SCALE).max(1e-3);
        [base[0] / s * if self.flip_h { -1.0 } else { 1.0 }, base[1] / s * if self.flip_v { -1.0 } else { 1.0 }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video() -> MediaOverlay {
        let mut o = MediaOverlay::new(ClipId(9), ClipSource::Video { path: "a.mov".into() });
        o.clip.media_secs = Some(10.0);
        o
    }

    #[test]
    fn plays_pauses_and_scrubs() {
        let mut o = video();
        o.advance(2.0, 4.0);
        assert!((o.time().secs - 2.0).abs() < 1e-9);
        o.transport.playing = false;
        o.advance(2.0, 4.0);
        assert!((o.time().secs - 2.0).abs() < 1e-9);
        o.scrub(0.5);
        assert!((o.progress() - 0.5).abs() < 1e-6);
        o.stop();
        assert_eq!(o.progress(), 0.0);
        assert!(!o.transport.playing);
    }

    #[test]
    fn loops_at_the_end() {
        let mut o = video();
        o.advance(12.0, 0.0);
        assert!((o.time().secs - 2.0).abs() < 1e-6);
    }

    #[test]
    fn placement() {
        let mut o = video();
        // A wide picture in a square frame: fill crops the sides, fit shows bars above and below.
        let cover = [0.5, 1.0];
        assert_eq!(o.uv_scale(cover, 0.25), [0.5, 1.0]);
        o.fill = false;
        assert_eq!(o.uv_scale(cover, 0.25), [1.0, 2.0]);
        o.flip_h = true;
        o.fill = true;
        assert_eq!(o.uv_scale(cover, 0.5), [-0.25, 0.5]);
    }

    #[test]
    fn round_trips_without_transport() {
        let o = video();
        let json = serde_json::to_string(&o).unwrap();
        let back: MediaOverlay = serde_json::from_str(&json).unwrap();
        assert_eq!(back, o);
    }
}
