//! Following an external MIDI clock: 24 pulses per quarter note, plus Start, Continue and Stop.
//!
//! Pure timing logic. The app timestamps each message as it arrives and feeds it to
//! [`MidiClock::message`]; each frame it reads [`MidiClock::beat_at`] and [`MidiClock::bpm`].

use std::collections::VecDeque;

/// A MIDI real-time message that drives the clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockMessage {
    /// 0xF8: one of 24 pulses per quarter note.
    Pulse,
    /// 0xFA: start from the top.
    Start,
    /// 0xFB: carry on from where Stop left off.
    Continue,
    /// 0xFC: stop.
    Stop,
}

impl ClockMessage {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        Some(match bytes.first()? {
            0xf8 => ClockMessage::Pulse,
            0xfa => ClockMessage::Start,
            0xfb => ClockMessage::Continue,
            0xfc => ClockMessage::Stop,
            _ => return None,
        })
    }
}

/// Pulses per quarter note (one beat).
pub const PPQN: f64 = 24.0;

/// The tempo follows the average pulse interval over this many pulses (2 beats).
const WINDOW: usize = 48;

/// No pulse for this long means the clock has gone away.
pub const CLOCK_TIMEOUT: f64 = 1.0;

#[derive(Debug, Clone, Default)]
pub struct MidiClock {
    /// Arrival times of the most recent pulses, in seconds.
    pulses: VecDeque<f64>,
    /// Pulses counted since Start; −1 between Start and its first pulse.
    count: i64,
    /// False after Stop: the beat holds until Continue or Start.
    running: bool,
    /// Smoothed tempo, once at least two pulses have arrived.
    bpm: Option<f32>,
}

impl MidiClock {
    pub fn new() -> Self {
        Self { running: true, ..Default::default() }
    }

    /// Feeds one message that arrived at `t` seconds.
    pub fn message(&mut self, msg: ClockMessage, t: f64) {
        match msg {
            ClockMessage::Pulse => self.pulse(t),
            ClockMessage::Start => {
                // The pulse after Start is the downbeat.
                self.count = -1;
                self.running = true;
            }
            ClockMessage::Continue => self.running = true,
            ClockMessage::Stop => self.running = false,
        }
    }

    fn pulse(&mut self, t: f64) {
        if self.is_waiting(t) {
            // The clock came back after a gap: pick up from where the free-running beat got
            // to rather than jumping back, and start measuring the tempo afresh.
            if let Some(beat) = self.beat_at(t) {
                self.count = (beat * PPQN).round() as i64 - 1;
            }
            self.pulses.clear();
        }
        self.pulses.push_back(t);
        if self.pulses.len() > WINDOW + 1 {
            self.pulses.pop_front();
        }
        if self.running {
            self.count += 1;
        }
        if self.pulses.len() >= 2 {
            let span = self.pulses.back().unwrap() - self.pulses.front().unwrap();
            let interval = span / (self.pulses.len() - 1) as f64;
            if interval > 0.0 {
                let bpm = (60.0 / (interval * PPQN)).clamp(20.0, 300.0) as f32;
                self.bpm = Some(bpm);
            }
        }
    }

    /// The measured tempo, once known. It holds its last value when the clock stops.
    pub fn bpm(&self) -> Option<f32> {
        self.bpm
    }

    /// True after Stop, until Start or Continue.
    pub fn is_stopped(&self) -> bool {
        !self.running
    }

    /// True when no pulse has arrived for [`CLOCK_TIMEOUT`] (or none ever has).
    pub fn is_waiting(&self, now: f64) -> bool {
        self.pulses.back().is_none_or(|last| now - last > CLOCK_TIMEOUT)
    }

    /// Beat position at `now`, once a tempo is known. Between pulses the beat runs on at the
    /// measured tempo, but never past the next pulse, so it never has to jump back. After
    /// Stop it holds; when the clock has gone away it runs on freely at the last tempo.
    pub fn beat_at(&self, now: f64) -> Option<f64> {
        let bpm = self.bpm? as f64;
        let last = *self.pulses.back()?;
        let at_pulse = self.count.max(0) as f64 / PPQN;
        if !self.running {
            return Some(at_pulse);
        }
        let ahead = (now - last).max(0.0) * bpm / 60.0;
        if now - last > CLOCK_TIMEOUT {
            return Some(at_pulse + ahead);
        }
        Some(at_pulse + ahead.min(1.0 / PPQN))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sends `n` pulses at `bpm` starting at `t`, returning the time of the next pulse.
    fn pulses(clock: &mut MidiClock, t: f64, n: usize, bpm: f64) -> f64 {
        let dt = 60.0 / bpm / PPQN;
        for i in 0..n {
            clock.message(ClockMessage::Pulse, t + i as f64 * dt);
        }
        t + n as f64 * dt
    }

    #[test]
    fn parses_real_time_messages() {
        assert_eq!(ClockMessage::parse(&[0xf8]), Some(ClockMessage::Pulse));
        assert_eq!(ClockMessage::parse(&[0xfa]), Some(ClockMessage::Start));
        assert_eq!(ClockMessage::parse(&[0xfb]), Some(ClockMessage::Continue));
        assert_eq!(ClockMessage::parse(&[0xfc]), Some(ClockMessage::Stop));
        assert_eq!(ClockMessage::parse(&[0x90, 60, 100]), None);
    }

    #[test]
    fn measures_a_steady_tempo() {
        let mut c = MidiClock::new();
        assert_eq!(c.bpm(), None);
        pulses(&mut c, 0.0, 49, 124.0);
        assert!((c.bpm().unwrap() - 124.0).abs() < 0.01);
    }

    #[test]
    fn start_puts_the_next_pulse_on_beat_zero() {
        let mut c = MidiClock::new();
        let t = pulses(&mut c, 0.0, 30, 120.0);
        c.message(ClockMessage::Start, t);
        c.message(ClockMessage::Pulse, t + 0.001);
        assert_eq!(c.beat_at(t + 0.001), Some(0.0));
        // 24 more pulses make one beat.
        pulses(&mut c, t + 0.001 + 60.0 / 120.0 / PPQN, 24, 120.0);
        let beat = c.beat_at(t + 0.001 + 0.5).unwrap();
        assert!((beat - 1.0).abs() < 1e-6, "{beat}");
    }

    #[test]
    fn beat_runs_between_pulses_but_never_past_the_next() {
        let mut c = MidiClock::new();
        c.message(ClockMessage::Start, 0.0);
        let t = pulses(&mut c, 0.0, 25, 120.0);
        let last = t - 60.0 / 120.0 / PPQN;
        let at = c.beat_at(last).unwrap();
        let half = c.beat_at(last + 0.5 / 120.0 / PPQN * 60.0).unwrap();
        assert!(half > at);
        // A late pulse: the beat waits at the next pulse's position.
        let late = c.beat_at(last + 0.2).unwrap();
        assert!((late - (at + 1.0 / PPQN)).abs() < 1e-9);
    }

    #[test]
    fn follows_a_tempo_change() {
        let mut c = MidiClock::new();
        let t = pulses(&mut c, 0.0, 48, 120.0);
        pulses(&mut c, t, 49, 140.0);
        assert!((c.bpm().unwrap() - 140.0).abs() < 0.01);
    }

    #[test]
    fn stop_holds_the_beat_and_continue_resumes() {
        let mut c = MidiClock::new();
        c.message(ClockMessage::Start, 0.0);
        let t = pulses(&mut c, 0.0, 49, 120.0);
        c.message(ClockMessage::Stop, t);
        let held = c.beat_at(t).unwrap();
        // Clock keeps ticking while stopped (as most devices do), but the beat holds.
        let t2 = pulses(&mut c, t, 24, 120.0);
        assert_eq!(c.beat_at(t2).unwrap(), held);
        assert!(c.is_stopped());
        c.message(ClockMessage::Continue, t2);
        pulses(&mut c, t2, 1, 120.0);
        assert!(c.beat_at(t2).unwrap() > held);
    }

    #[test]
    fn holds_the_tempo_and_runs_on_when_the_clock_goes_away() {
        let mut c = MidiClock::new();
        c.message(ClockMessage::Start, 0.0);
        let t = pulses(&mut c, 0.0, 49, 120.0);
        assert!(!c.is_waiting(t));
        let later = t + 3.0;
        assert!(c.is_waiting(later));
        assert!((c.bpm().unwrap() - 120.0).abs() < 0.01);
        let beat = c.beat_at(later).unwrap();
        assert!(beat > 2.0 + 5.0, "{beat}");
        // When it comes back, the beat carries on rather than jumping back.
        c.message(ClockMessage::Pulse, later);
        assert!((c.beat_at(later).unwrap() - beat).abs() < 1.0 / PPQN);
    }
}
