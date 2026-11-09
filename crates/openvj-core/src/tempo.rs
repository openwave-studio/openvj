/// Tap tempo: averages the intervals of recent taps.
#[derive(Debug, Default, Clone)]
pub struct TapTempo {
    taps: Vec<f64>,
}

impl TapTempo {
    /// Taps further apart than this start a new measurement.
    const RESET_SECS: f64 = 2.0;
    const MAX_TAPS: usize = 8;

    /// Registers a tap at `now` (seconds) and returns the new BPM once there are two taps.
    pub fn tap(&mut self, now: f64) -> Option<f32> {
        if self.taps.last().is_some_and(|last| now - last > Self::RESET_SECS) {
            self.taps.clear();
        }
        self.taps.push(now);
        if self.taps.len() > Self::MAX_TAPS {
            self.taps.remove(0);
        }
        if self.taps.len() < 2 {
            return None;
        }
        let span = self.taps.last().unwrap() - self.taps.first().unwrap();
        let interval = span / (self.taps.len() - 1) as f64;
        Some((60.0 / interval).clamp(20.0, 300.0) as f32)
    }
}

/// Converts seconds to beats at `bpm`.
pub fn beats(seconds: f64, bpm: f32) -> f64 {
    seconds * bpm as f64 / 60.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tap_tempo_averages_and_resets() {
        let mut t = TapTempo::default();
        assert_eq!(t.tap(0.0), None);
        assert_eq!(t.tap(0.5), Some(120.0));
        assert_eq!(t.tap(1.0), Some(120.0));
        // Long pause starts over.
        assert_eq!(t.tap(10.0), None);
        let bpm = t.tap(10.4).unwrap();
        assert!((bpm - 150.0).abs() < 0.01);
    }

    #[test]
    fn beats_from_seconds() {
        assert_eq!(beats(1.0, 120.0), 2.0);
    }
}
