//! The Discover layout's state: the review queue, the current item playing on the Preview
//! (never the output), and its parameter panel. The logic lives in
//! [`openvj_core::discover`]; this maps a clip's parameters to that panel and back.

use openvj_core::discover::{Knobs, Order, Queue, Rng};
use openvj_core::isf::InputType;
use openvj_core::{Clip, ClipSource, MILK_SENSITIVITY};

/// Where a Discover parameter lives in the clip.
#[derive(Debug, Clone, PartialEq)]
pub enum Slot {
    /// A built-in generator's parameter.
    Builtin(usize),
    /// A MilkDrop preset's sensitivity.
    Milk,
    /// An ISF generator's channel, by key.
    Isf(String),
    /// A channel of the effect under review (the clip's last effect), by key.
    Effect(String),
}

/// One row of the parameter panel.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub slot: Slot,
    pub name: String,
}

#[derive(Debug, Default)]
pub struct DiscoverState {
    pub queue: Queue,
    /// Whether the queue was built since the app started.
    pub started: bool,
    /// Packs in the From picker that are on; none means all.
    pub packs: Vec<String>,
    pub order: Order,
    pub rng: Option<Rng>,
    /// The item under review as it plays on the Preview, with the panel's values applied.
    pub clip: Option<Clip>,
    pub rows: Vec<Row>,
    pub knobs: Knobs,
}

impl DiscoverState {
    pub fn rng(&mut self) -> &mut Rng {
        self.rng.get_or_insert_with(|| Rng::new(seed()))
    }
}

/// A seed from the clock, so every session shuffles differently.
pub fn seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64)
}

/// The parameter panel for a clip: its rows and their values. For an effect under review
/// (`effect`), the rows are that effect's (the clip's last); otherwise the source's.
/// Momentary (event) inputs are shown but never randomized.
pub fn panel(clip: &Clip, effect: bool) -> (Vec<Row>, Knobs) {
    let mut rows = Vec::new();
    let (mut values, mut defaults, mut fixed) = (Vec::new(), Vec::new(), Vec::new());
    let mut push = |slot: Slot, name: &str, value: f32, default: f32, event: bool| {
        rows.push(Row { slot, name: name.to_owned() });
        values.push(value);
        defaults.push(default);
        fixed.push(event);
    };
    let source = if effect { None } else { Some(&clip.source) };
    match source {
        Some(ClipSource::Generator { kind }) => {
            for (i, spec) in kind.params().iter().enumerate() {
                push(Slot::Builtin(i), spec.name, clip.params[i], spec.default, false);
            }
        }
        Some(ClipSource::Milkdrop { .. }) => {
            push(Slot::Milk, "Sensitivity", clip.params[MILK_SENSITIVITY], 0.5, false);
        }
        Some(ClipSource::Isf { generator }) => {
            if let Some(ty) = generator.ty() {
                for ch in &ty.channels {
                    let v = generator.param(&ch.key).map_or(ch.default, |p| p.value);
                    let event = ty.input(ch).ty == InputType::Event;
                    push(Slot::Isf(ch.key.clone()), &ch.label, v, ch.default, event);
                }
            }
        }
        _ => {}
    }
    if effect
        && let Some(fx) = clip.effects.last()
        && let Some(ty) = fx.ty()
    {
        for ch in &ty.channels {
            let v = fx.param(&ch.key).map_or(ch.default, |p| p.value);
            let event = ty.input(ch).ty == InputType::Event;
            push(Slot::Effect(ch.key.clone()), &ch.label, v, ch.default, event);
        }
    }
    (rows, Knobs::new(values, defaults, fixed))
}

/// Writes the panel's values into the clip.
pub fn apply(clip: &mut Clip, rows: &[Row], values: &[f32]) {
    for (row, &v) in rows.iter().zip(values) {
        match &row.slot {
            Slot::Builtin(i) => clip.params[*i] = v,
            Slot::Milk => clip.params[MILK_SENSITIVITY] = v,
            Slot::Isf(key) => {
                if let ClipSource::Isf { generator } = &mut clip.source
                    && let Some(p) = generator.param_mut(key)
                {
                    p.value = v;
                }
            }
            Slot::Effect(key) => {
                if let Some(p) = clip.effects.last_mut().and_then(|fx| fx.param_mut(key)) {
                    p.value = v;
                }
            }
        }
    }
}

/// How a parameter's value reads on its bar.
pub fn display(clip: &Clip, slot: &Slot, v: f32) -> String {
    let pct = || format!("{:.0}%", v * 100.0);
    match slot {
        Slot::Builtin(_) => pct(),
        Slot::Milk => format!("{:.0}%", v * 200.0),
        Slot::Isf(key) => match &clip.source {
            ClipSource::Isf { generator } => isf_display(generator, key, v).unwrap_or_else(pct),
            _ => pct(),
        },
        Slot::Effect(key) => clip.effects.last().and_then(|fx| isf_display(fx, key, v)).unwrap_or_else(pct),
    }
}

fn isf_display(fx: &openvj_core::Effect, key: &str, v: f32) -> Option<String> {
    let ty = fx.ty()?;
    let ch = ty.channels.iter().find(|c| c.key == key)?;
    Some(ty.input(ch).display(ch.component, v))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openvj_core::GeneratorKind;

    fn generator_clip() -> Clip {
        let mut comp = openvj_core::Composition::new(16, 9, 1, 1);
        comp.set_clip(0, 0, ClipSource::Generator { kind: GeneratorKind::ALL[0] });
        comp.clip(0, 0).cloned().unwrap()
    }

    #[test]
    fn builtin_generator_params_round_trip() {
        let mut clip = generator_clip();
        let (rows, mut knobs) = panel(&clip, false);
        assert_eq!(rows.len(), openvj_core::PARAM_COUNT);
        assert_eq!(knobs.values(), clip.params);
        knobs.set(1, 0.9);
        apply(&mut clip, &rows, knobs.values());
        assert_eq!(clip.params[1], 0.9);
        assert_eq!(display(&clip, &rows[1].slot, 0.9), "90%");
    }

    #[test]
    fn milkdrop_has_sensitivity_only() {
        let mut comp = openvj_core::Composition::new(16, 9, 1, 1);
        comp.set_clip(0, 0, ClipSource::Milkdrop { preset: "x.milk".into() });
        let clip = comp.clip(0, 0).cloned().unwrap();
        let (rows, knobs) = panel(&clip, false);
        assert_eq!(rows, [Row { slot: Slot::Milk, name: "Sensitivity".into() }]);
        assert_eq!(knobs.values(), [0.5]);
        assert_eq!(display(&clip, &Slot::Milk, 0.5), "100%");
    }
}
