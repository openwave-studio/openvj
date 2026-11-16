//! Macros: knobs that move several of a clip's parameters at once, each over its own
//! range, like Ableton's rack macros and Synesthesia's controls. Shown in the Synthesis
//! layout; a macro's value is a [`Param`], so it can be played by hand or take one driver
//! (audio or tempo) like any parameter.

use serde::{Deserialize, Serialize};

use crate::{Clip, ClipSource, ModInputs, PARAM_COUNT, Param};

/// Macros per clip: one row of knobs, and the size of most controllers' knob banks.
pub const MACRO_COUNT: usize = 8;

/// One of a clip's parameters that a macro can move.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamRef {
    /// A built-in generator parameter (or MilkDrop's Sensitivity), by index.
    Source(usize),
    /// An ISF generator's input, by key.
    Isf(String),
    /// A parameter of the clip's effect at `effect`, by key; `"mix"` is its dry/wet mix.
    Effect { effect: usize, key: String },
    /// A parameter of the clip's Transform, by key.
    Transform(String),
}

/// The key [`ParamRef::Effect`] uses for an effect's dry/wet mix.
pub const MIX_KEY: &str = "mix";

/// A parameter a macro moves: at macro 0 it is `from`, at 1 it is `to`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacroTarget {
    pub param: ParamRef,
    pub from: f32,
    pub to: f32,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Macro {
    /// Empty shows "Macro N".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default)]
    pub value: Param,
    #[serde(default)]
    pub targets: Vec<MacroTarget>,
}

impl Macro {
    pub fn is_used(&self) -> bool {
        !self.targets.is_empty() || self.value.modulation.is_active()
    }
}

impl Clip {
    /// The value of one of the clip's parameters, if it has it.
    pub fn param_ref_mut(&mut self, r: &ParamRef) -> Option<&mut f32> {
        match r {
            ParamRef::Source(i) if *i < PARAM_COUNT => self.params.get_mut(*i),
            ParamRef::Source(_) => None,
            ParamRef::Isf(key) => match &mut self.source {
                ClipSource::Isf { generator } => generator.param_mut(key).map(|p| &mut p.value),
                _ => None,
            },
            ParamRef::Effect { effect, key } => {
                let fx = self.effects.get_mut(*effect)?;
                if key == MIX_KEY { Some(&mut fx.mix.value) } else { fx.param_mut(key).map(|p| &mut p.value) }
            }
            ParamRef::Transform(key) => self.transform.param_mut(key).map(|p| &mut p.value),
        }
    }

    /// The macro moving `r`, if any.
    pub fn macro_of(&self, r: &ParamRef) -> Option<usize> {
        self.macros.iter().position(|m| m.targets.iter().any(|t| t.param == *r))
    }

    /// Macro `index`, creating the row of macros the first time.
    pub fn macro_mut(&mut self, index: usize) -> Option<&mut Macro> {
        if index >= MACRO_COUNT {
            return None;
        }
        if self.macros.len() < MACRO_COUNT {
            self.macros.resize_with(MACRO_COUNT, Macro::default);
        }
        self.macros.get_mut(index)
    }

    /// Adds `r` to macro `index`, or takes it off if it's already there. A parameter belongs
    /// to one macro at a time. A new target runs from the parameter's current value (at the
    /// macro's current position) to the far end of its range.
    pub fn toggle_macro_target(&mut self, index: usize, r: ParamRef) -> bool {
        let Some(current) = self.param_ref_mut(&r).map(|v| *v) else { return false };
        if let Some(owner) = self.macro_of(&r) {
            self.macros[owner].targets.retain(|t| t.param != r);
            if owner == index {
                return true;
            }
        }
        let Some(m) = self.macro_mut(index) else { return false };
        let to = if current > 0.5 { 0.0 } else { 1.0 };
        // Solve current = from + (to - from) * position, so the parameter doesn't jump.
        let pos = m.value.value.clamp(0.0, 0.99);
        let from = ((current - to * pos) / (1.0 - pos)).clamp(0.0, 1.0);
        m.targets.push(MacroTarget { param: r, from, to });
        true
    }

    /// Sets every macro target from its macro's value this frame (after the macro's driver).
    pub fn apply_macros(&mut self, inputs: &ModInputs) {
        if self.macros.iter().all(|m| m.targets.is_empty()) {
            return;
        }
        let moves: Vec<(ParamRef, f32)> = self
            .macros
            .iter()
            .flat_map(|m| {
                let v = m.value.eval(inputs);
                m.targets.iter().map(move |t| (t.param.clone(), t.from + (t.to - t.from) * v))
            })
            .collect();
        for (r, v) in moves {
            if let Some(p) = self.param_ref_mut(&r) {
                *p = v.clamp(0.0, 1.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Composition, Effect, GeneratorKind, ModSource, Modulation};

    fn plasma() -> Clip {
        let mut comp = Composition::new(64, 64, 1, 1);
        comp.set_clip(0, 0, ClipSource::Generator { kind: GeneratorKind::Plasma }).unwrap();
        let mut clip = comp.clip(0, 0).unwrap().clone();
        clip.effects.push(Effect::new("blur"));
        clip
    }

    #[test]
    fn one_knob_moves_several_parameters() {
        let mut clip = plasma();
        clip.macro_mut(0).unwrap().targets = vec![
            MacroTarget { param: ParamRef::Source(0), from: 0.25, to: 1.0 },
            MacroTarget { param: ParamRef::Effect { effect: 0, key: MIX_KEY.into() }, from: 0.0, to: 0.6 },
        ];
        clip.macros[0].value.value = 0.5;
        clip.apply_macros(&ModInputs::default());
        assert!((clip.params[0] - 0.625).abs() < 1e-6);
        assert!((clip.effects[0].mix.value - 0.3).abs() < 1e-6);
    }

    #[test]
    fn a_driver_animates_the_macro() {
        let mut clip = plasma();
        clip.macro_mut(1).unwrap().targets = vec![MacroTarget { param: ParamRef::Source(1), from: 0.0, to: 1.0 }];
        clip.macros[1].value = Param {
            value: 0.0,
            modulation: Modulation { source: ModSource::Sine, amount: 1.0, beats: 4.0, ..Default::default() },
        };
        let mut inputs = ModInputs::default();
        let mut seen = Vec::new();
        for beat in [0.0, 1.0, 2.0, 3.0] {
            inputs.beat = beat;
            clip.apply_macros(&inputs);
            seen.push(clip.params[1]);
        }
        assert!(seen.windows(2).any(|w| w[0] != w[1]), "the target follows the beat: {seen:?}");
    }

    #[test]
    fn toggling_adds_without_a_jump_and_removes() {
        let mut clip = plasma();
        clip.params[2] = 0.3;
        assert!(clip.toggle_macro_target(0, ParamRef::Source(2)));
        assert_eq!(clip.macro_of(&ParamRef::Source(2)), Some(0));
        clip.apply_macros(&ModInputs::default());
        assert!((clip.params[2] - 0.3).abs() < 1e-6, "adding a target doesn't move it");
        // Moving it to another macro takes it off the first.
        clip.toggle_macro_target(3, ParamRef::Source(2));
        assert_eq!(clip.macro_of(&ParamRef::Source(2)), Some(3));
        clip.toggle_macro_target(3, ParamRef::Source(2));
        assert_eq!(clip.macro_of(&ParamRef::Source(2)), None);
        assert!(!clip.toggle_macro_target(0, ParamRef::Source(99)), "unknown parameters are refused");
    }

    #[test]
    fn macros_save_with_the_clip() {
        let mut comp = Composition::new(64, 64, 1, 1);
        comp.set_clip(0, 0, ClipSource::Generator { kind: GeneratorKind::Plasma }).unwrap();
        let clip = comp.clip_mut(0, 0).unwrap();
        clip.toggle_macro_target(0, ParamRef::Source(0));
        clip.macros[0].name = "Warp".into();
        let back = Composition::from_json(&comp.to_json().unwrap()).unwrap();
        assert_eq!(back.clip(0, 0).unwrap().macros, comp.clip(0, 0).unwrap().macros);
        // A clip without macros saves no macros key.
        let plain = Composition::new(64, 64, 1, 1);
        assert!(!plain.to_json().unwrap().contains("macros"));
    }
}
