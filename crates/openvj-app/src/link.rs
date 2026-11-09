//! Ableton Link tempo and beat sync. Without the `link` feature this is an inert stub.

#[cfg(feature = "link")]
/// Beats per bar used for phase alignment with other Link peers.
const QUANTUM: f64 = 4.0;

#[cfg(feature = "link")]
pub struct Link {
    link: rusty_link::AblLink,
    state: rusty_link::SessionState,
}

#[cfg(feature = "link")]
impl Link {
    pub const AVAILABLE: bool = true;

    pub fn new(bpm: f32) -> Self {
        Self { link: rusty_link::AblLink::new(bpm as f64), state: rusty_link::SessionState::new() }
    }

    pub fn is_enabled(&self) -> bool {
        self.link.is_enabled()
    }

    /// Joins or leaves the Link session. On joining, offers `bpm` as the tempo
    /// (peers already in a session keep theirs).
    pub fn set_enabled(&mut self, on: bool, bpm: f32) {
        if on && !self.is_enabled() {
            self.set_tempo(bpm);
        }
        self.link.enable(on);
    }

    pub fn peers(&self) -> u64 {
        self.link.num_peers()
    }

    /// The session's beat position and tempo right now.
    pub fn capture(&mut self) -> (f64, f32) {
        self.link.capture_app_session_state(&mut self.state);
        let beat = self.state.beat_at_time(self.link.clock_micros(), QUANTUM);
        (beat, self.state.tempo() as f32)
    }

    /// Proposes a new tempo to the session.
    pub fn set_tempo(&mut self, bpm: f32) {
        self.link.capture_app_session_state(&mut self.state);
        self.state.set_tempo(bpm as f64, self.link.clock_micros());
        self.link.commit_app_session_state(&self.state);
    }
}

#[cfg(not(feature = "link"))]
pub struct Link;

#[cfg(not(feature = "link"))]
impl Link {
    pub const AVAILABLE: bool = false;

    pub fn new(_bpm: f32) -> Self {
        Self
    }

    pub fn is_enabled(&self) -> bool {
        false
    }

    pub fn set_enabled(&mut self, _on: bool, _bpm: f32) {}

    pub fn peers(&self) -> u64 {
        0
    }

    pub fn capture(&mut self) -> (f64, f32) {
        (0.0, 120.0)
    }

    pub fn set_tempo(&mut self, _bpm: f32) {}
}
