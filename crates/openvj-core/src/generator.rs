use serde::{Deserialize, Serialize};

/// Number of user parameters every generator exposes. All are normalised to 0..1.
pub const PARAM_COUNT: usize = 4;

/// Built-in procedural sources. The shader side switches on [`GeneratorKind::shader_id`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneratorKind {
    Plasma,
    Tunnel,
    Bars,
    Rings,
    Noise,
    Kaleido,
    Gradient,
    Strobe,
    Checker,
}

impl GeneratorKind {
    pub const ALL: [GeneratorKind; 9] = [
        GeneratorKind::Plasma,
        GeneratorKind::Tunnel,
        GeneratorKind::Bars,
        GeneratorKind::Rings,
        GeneratorKind::Noise,
        GeneratorKind::Kaleido,
        GeneratorKind::Gradient,
        GeneratorKind::Strobe,
        GeneratorKind::Checker,
    ];

    pub fn name(self) -> &'static str {
        match self {
            GeneratorKind::Plasma => "Plasma",
            GeneratorKind::Tunnel => "Tunnel",
            GeneratorKind::Bars => "Bars",
            GeneratorKind::Rings => "Rings",
            GeneratorKind::Noise => "Noise",
            GeneratorKind::Kaleido => "Kaleido",
            GeneratorKind::Gradient => "Gradient",
            GeneratorKind::Strobe => "Strobe",
            GeneratorKind::Checker => "Checker",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            GeneratorKind::Plasma => "Classic sine plasma",
            GeneratorKind::Tunnel => "Polar tunnel flythrough",
            GeneratorKind::Bars => "Beat-synced moving bars",
            GeneratorKind::Rings => "Concentric rings pulsing on the beat",
            GeneratorKind::Noise => "Flowing fractal noise",
            GeneratorKind::Kaleido => "Kaleidoscope of a moving pattern",
            GeneratorKind::Gradient => "Sweeping colour gradient",
            GeneratorKind::Strobe => "Tempo-locked strobe",
            GeneratorKind::Checker => "Warping checkerboard",
        }
    }

    /// Index the WGSL generator shader switches on. Must match `fs_generator`.
    pub fn shader_id(self) -> u32 {
        Self::ALL.iter().position(|k| *k == self).unwrap() as u32
    }

    pub fn params(self) -> [ParamSpec; PARAM_COUNT] {
        use ParamSpec as P;
        match self {
            GeneratorKind::Plasma => {
                [P::new("Scale", 0.4), P::new("Hue", 0.6), P::new("Contrast", 0.5), P::new("Warp", 0.3)]
            }
            GeneratorKind::Tunnel => {
                [P::new("Twist", 0.3), P::new("Rings", 0.5), P::new("Hue", 0.8), P::new("Glow", 0.5)]
            }
            GeneratorKind::Bars => {
                [P::new("Count", 0.3), P::new("Angle", 0.0), P::new("Hue", 0.1), P::new("Width", 0.5)]
            }
            GeneratorKind::Rings => {
                [P::new("Count", 0.4), P::new("Thickness", 0.3), P::new("Hue", 0.5), P::new("Pulse", 0.7)]
            }
            GeneratorKind::Noise => {
                [P::new("Scale", 0.4), P::new("Detail", 0.6), P::new("Hue", 0.3), P::new("Contrast", 0.5)]
            }
            GeneratorKind::Kaleido => {
                [P::new("Segments", 0.4), P::new("Zoom", 0.5), P::new("Hue", 0.0), P::new("Spin", 0.3)]
            }
            GeneratorKind::Gradient => {
                [P::new("Angle", 0.25), P::new("Spread", 0.5), P::new("Hue", 0.0), P::new("Saturation", 0.8)]
            }
            GeneratorKind::Strobe => {
                [P::new("Rate", 0.5), P::new("Duty", 0.2), P::new("Hue", 0.0), P::new("Saturation", 0.0)]
            }
            GeneratorKind::Checker => {
                [P::new("Size", 0.4), P::new("Rotate", 0.2), P::new("Hue", 0.55), P::new("Warp", 0.4)]
            }
        }
    }

    pub fn default_params(self) -> [f32; PARAM_COUNT] {
        self.params().map(|p| p.default)
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.name().eq_ignore_ascii_case(name))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamSpec {
    pub name: &'static str,
    pub default: f32,
}

impl ParamSpec {
    const fn new(name: &'static str, default: f32) -> Self {
        Self { name, default }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_ids_are_dense_and_stable() {
        for (i, k) in GeneratorKind::ALL.iter().enumerate() {
            assert_eq!(k.shader_id(), i as u32);
            assert_eq!(GeneratorKind::from_name(k.name()), Some(*k));
        }
    }
}
