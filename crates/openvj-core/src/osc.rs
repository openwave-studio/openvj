//! OSC control: Resolume's OSC addresses, and OSC addresses learned in map mode.
//!
//! Like [`crate::midi`], this is pure data and logic. The app owns the UDP sockets: it
//! turns each incoming message into [`MidiAction`]s here and applies them with
//! [`crate::apply_midi`], so OSC drives exactly what MIDI can.
//!
//! Resolume numbers layers, columns, clips and decks from 1, with layer 1 at the bottom,
//! and sends faders as 0–1 floats and buttons as 1 (press) and 0 (release). Addresses
//! covered (all under `/composition`):
//!
//! * `master`, `crossfader/phase`, `disconnectall`
//! * `tempocontroller/tempo` (0–1 over 20–500 BPM), `…/tempotap`, `…/resync`
//! * `columns/C/connect`, `decks/D/select`
//! * `layers/L/clips/C/connect`, `layers/L/clear`, `layers/L/select`,
//!   `layers/L/video/opacity`, `layers/L/bypassed`, `layers/L/solo`
//! * `layers/L/video/effects/<effect>/effect/<param>`, `…/<effect>/bypassed`,
//!   `…/<effect>/opacity` (the mix); `video/effects/…` for the composition's effects
//! * the same layer addresses with `selectedlayer` in place of `layers/L`

use serde::{Deserialize, Serialize};

use crate::{
    BPM_RANGE, Composition, Effect, EffectRef, LayerRef, MidiAction, MidiBehaviour, MidiInput, MidiMap, MidiTarget,
};

/// An OSC address learned onto a control in map mode, such as TouchOSC's `/1/fader1`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OscMapping {
    pub address: String,
    pub target: MidiTarget,
    #[serde(default)]
    pub behaviour: MidiBehaviour,
    #[serde(default)]
    pub min: f32,
    #[serde(default = "one")]
    pub max: f32,
}

fn one() -> f32 {
    1.0
}

impl OscMapping {
    pub fn new(address: &str, target: MidiTarget) -> Self {
        let behaviour = if target.is_ranged() {
            MidiBehaviour::Fader
        } else if target.is_switch() {
            MidiBehaviour::Toggle
        } else {
            MidiBehaviour::Press
        };
        Self { address: address.to_owned(), target, behaviour, min: 0.0, max: 1.0 }
    }

    /// The input one message with `value` (None: no number, which counts as a press) asks for.
    fn input(&self, value: Option<f32>) -> Option<MidiInput> {
        let v = value.unwrap_or(1.0).clamp(0.0, 1.0);
        let press = v > 0.0;
        if self.target.is_ranged() || self.target.is_switch() {
            Some(match self.behaviour {
                MidiBehaviour::Fader => MidiInput::Set(self.min + (self.max - self.min) * v),
                MidiBehaviour::Toggle if press => MidiInput::Toggle { min: self.min, max: self.max },
                MidiBehaviour::Press if press => MidiInput::Set(self.max),
                MidiBehaviour::Hold => MidiInput::Set(if press { self.max } else { self.min }),
                _ => return None,
            })
        } else {
            button(self.target, value)
        }
    }
}

/// The input a button target gets from a message: launches on press, and a clip launch
/// also hears the release (for Piano clips).
fn button(target: MidiTarget, value: Option<f32>) -> Option<MidiInput> {
    let press = value.is_none_or(|v| v > 0.0);
    match target {
        MidiTarget::ClipLaunch { .. } if press => Some(MidiInput::Set(1.0)),
        MidiTarget::ClipLaunch { .. } => Some(MidiInput::Release),
        _ if press => Some(MidiInput::Trigger),
        _ => None,
    }
}

impl MidiMap {
    /// Maps an OSC address to `target`, replacing other mappings of either.
    pub fn learn_osc(&mut self, address: &str, target: MidiTarget) -> &OscMapping {
        self.osc.retain(|m| m.target != target && m.address != address);
        self.osc.push(OscMapping::new(address, target));
        self.osc.last().unwrap()
    }

    pub fn osc_mapping_for(&self, target: MidiTarget) -> Option<&OscMapping> {
        self.osc.iter().find(|m| m.target == target)
    }

    pub fn osc_mapping_for_mut(&mut self, target: MidiTarget) -> Option<&mut OscMapping> {
        self.osc.iter_mut().find(|m| m.target == target)
    }

    /// The actions a message to a learned address triggers.
    pub fn handle_osc(&self, address: &str, value: Option<f32>) -> Vec<MidiAction> {
        self.osc
            .iter()
            .filter(|m| m.address == address)
            .filter_map(|m| Some(MidiAction { target: m.target, input: m.input(value)? }))
            .collect()
    }
}

/// The action a message to one of Resolume's addresses asks for, if openVJ has the control.
pub fn resolume_action(address: &str, value: Option<f32>, comp: &Composition) -> Option<MidiAction> {
    let target = resolume_target(address, comp)?;
    let input = if target.is_ranged() || target.is_switch() {
        MidiInput::Set(value?.clamp(0.0, 1.0))
    } else {
        button(target, value)?
    };
    Some(MidiAction { target, input })
}

/// The control a Resolume OSC address names. Effects are found by name in the chain.
pub fn resolume_target(address: &str, comp: &Composition) -> Option<MidiTarget> {
    let parts: Vec<&str> = address.trim_matches('/').split('/').collect();
    let ["composition", rest @ ..] = parts.as_slice() else { return None };
    let index = |s: &str| s.parse::<usize>().ok().filter(|&n| n >= 1).map(|n| n - 1);
    Some(match rest {
        ["master"] => MidiTarget::Master,
        ["crossfader", "phase"] => MidiTarget::Crossfader,
        ["disconnectall"] => MidiTarget::ClearAll,
        ["tempocontroller", "tempo"] => MidiTarget::Tempo,
        ["tempocontroller", "tempotap"] => MidiTarget::TempoTap,
        ["tempocontroller", "resync"] => MidiTarget::TempoResync,
        ["columns", c, "connect"] => MidiTarget::ColumnLaunch { column: index(c)? },
        ["decks", d, "select"] => MidiTarget::DeckSelect { deck: index(d)? },
        ["layers", l, "clips", c, "connect"] => MidiTarget::ClipLaunch { layer: index(l)?, column: index(c)? },
        ["video", "effects", fx @ ..] => effect_target(fx, None, comp)?,
        ["layers", l, layer @ ..] => layer_target(LayerRef::Index(index(l)?), layer, comp)?,
        ["selectedlayer", layer @ ..] => layer_target(LayerRef::Selected, layer, comp)?,
        _ => return None,
    })
}

fn layer_target(layer: LayerRef, rest: &[&str], comp: &Composition) -> Option<MidiTarget> {
    Some(match rest {
        ["clear"] => MidiTarget::LayerClear { layer },
        ["select"] => MidiTarget::LayerSelect { layer },
        ["bypassed"] => MidiTarget::LayerBypass { layer },
        ["solo"] => MidiTarget::LayerSolo { layer },
        ["video", "opacity"] => MidiTarget::LayerOpacity { layer },
        ["video", "effects", fx @ ..] => effect_target(fx, Some(layer), comp)?,
        _ => return None,
    })
}

/// `<effect>/effect/<param>`, `<effect>/<param>`, `<effect>/bypassed` or `<effect>/opacity`.
fn effect_target(rest: &[&str], layer: Option<LayerRef>, comp: &Composition) -> Option<MidiTarget> {
    let (name, what) = rest.split_first()?;
    let chain = match layer {
        None => &comp.effects,
        // A selected-layer address resolves its effect when the message arrives; look it
        // up in every layer's chain here and keep the position.
        Some(LayerRef::Index(l)) => &comp.layers.get(l)?.effects,
        Some(LayerRef::Selected) => {
            let (index, fx) =
                comp.layers.iter().find_map(|l| l.effects.iter().enumerate().find(|(_, fx)| effect_named(fx, name)))?;
            return effect_part(EffectRef { layer, index }, fx, what);
        }
    };
    let (index, fx) = chain.iter().enumerate().find(|(_, fx)| effect_named(fx, name))?;
    effect_part(EffectRef { layer, index }, fx, what)
}

fn effect_part(effect: EffectRef, fx: &Effect, what: &[&str]) -> Option<MidiTarget> {
    Some(match what {
        ["bypassed"] => MidiTarget::EffectBypass { effect },
        ["opacity"] | ["mix"] => MidiTarget::EffectMix { effect },
        ["effect", param] | [param] => MidiTarget::EffectParam { effect, param: param_index(fx, param)? },
        _ => return None,
    })
}

/// The name an effect has in Resolume-style addresses: its name, lowercase, without spaces.
pub fn address_name(name: &str) -> String {
    name.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_lowercase()).collect()
}

fn effect_named(fx: &Effect, name: &str) -> bool {
    let want = address_name(name);
    if address_name(&fx.name()) == want || address_name(&fx.kind) == want {
        return true;
    }
    fx.ty().is_some_and(|t| t.def.resolume.iter().any(|r| address_name(r) == want))
}

fn param_index(fx: &Effect, name: &str) -> Option<usize> {
    let t = fx.ty()?;
    let want = address_name(name);
    t.channels.iter().position(|c| {
        let input = &t.def.inputs[c.input];
        address_name(&c.label) == want
            || address_name(&c.key) == want
            || input.resolume.as_ref().is_some_and(|r| c.component == 0 && address_name(&r.name) == want)
    })
}

/// The Resolume address openVJ sends for a target, for OSC output. Targets that only
/// make sense as input (stepping, the selected layer) have none.
pub fn resolume_address(target: MidiTarget, comp: &Composition) -> Option<String> {
    let layer = |r: LayerRef| match r {
        LayerRef::Index(l) => Some(format!("/composition/layers/{}", l + 1)),
        LayerRef::Selected => None,
    };
    Some(match target {
        MidiTarget::Master => "/composition/master".into(),
        MidiTarget::Crossfader => "/composition/crossfader/phase".into(),
        MidiTarget::Tempo => "/composition/tempocontroller/tempo".into(),
        MidiTarget::LayerOpacity { layer: l } => format!("{}/video/opacity", layer(l)?),
        MidiTarget::LayerBypass { layer: l } => format!("{}/bypassed", layer(l)?),
        MidiTarget::LayerSolo { layer: l } => format!("{}/solo", layer(l)?),
        MidiTarget::ClipLaunch { layer, column } => {
            format!("/composition/layers/{}/clips/{}/connect", layer + 1, column + 1)
        }
        MidiTarget::EffectMix { effect } => format!("{}/opacity", effect_base(effect, comp)?.0),
        MidiTarget::EffectParam { effect, param } => {
            let (base, fx) = effect_base(effect, comp)?;
            format!("{base}/effect/{}", address_name(&fx.ty()?.channels.get(param)?.label))
        }
        MidiTarget::EffectBypass { effect } => format!("{}/bypassed", effect_base(effect, comp)?.0),
        _ => return None,
    })
}

fn effect_base(effect: EffectRef, comp: &Composition) -> Option<(String, &Effect)> {
    let (prefix, chain) = match effect.layer {
        None => ("/composition".to_owned(), &comp.effects),
        Some(LayerRef::Index(l)) => (format!("/composition/layers/{}", l + 1), &comp.layers.get(l)?.effects),
        Some(LayerRef::Selected) => return None,
    };
    let fx = chain.get(effect.index)?;
    Some((format!("{prefix}/video/effects/{}", address_name(&fx.name())), fx))
}

/// A target's current value as Resolume sends it: 0–1 for faders, 1 or 0 for switches and
/// for whether a clip is playing.
pub fn current_value(target: MidiTarget, comp: &Composition, selected: usize) -> Option<f32> {
    let layer = |r: LayerRef| {
        comp.layers.get(match r {
            LayerRef::Index(l) => l,
            LayerRef::Selected => selected,
        })
    };
    let flag = |b: bool| if b { 1.0 } else { 0.0 };
    let effect = |e: EffectRef| -> Option<&Effect> {
        match e.layer {
            None => comp.effects.get(e.index),
            Some(l) => layer(l)?.effects.get(e.index),
        }
    };
    Some(match target {
        MidiTarget::Master => comp.master,
        MidiTarget::Crossfader => comp.crossfader.position,
        MidiTarget::Tempo => ((comp.bpm - BPM_RANGE.0) / (BPM_RANGE.1 - BPM_RANGE.0)).clamp(0.0, 1.0),
        MidiTarget::LayerOpacity { layer: l } => layer(l)?.opacity,
        MidiTarget::LayerBypass { layer: l } => flag(layer(l)?.bypass),
        MidiTarget::LayerSolo { layer: l } => flag(layer(l)?.solo),
        MidiTarget::ClipLaunch { layer: l, column } => flag(comp.layers.get(l)?.playing_column() == Some(column)),
        MidiTarget::EffectParam { effect: e, param } => {
            let fx = effect(e)?;
            let key = fx.ty()?.channels.get(param)?.key.clone();
            fx.param(&key)?.value
        }
        MidiTarget::EffectMix { effect: e } => effect(e)?.mix.value,
        MidiTarget::EffectBypass { effect: e } => flag(!effect(e)?.enabled),
        _ => return None,
    })
}

/// Every target openVJ reports over OSC output, for the composition as it is: master,
/// crossfader, tempo, each layer's opacity, bypass and solo, each clip's connect state, and
/// every effect parameter, mix and bypass on layers and the composition.
pub fn output_targets(comp: &Composition) -> Vec<MidiTarget> {
    let mut out = vec![MidiTarget::Master, MidiTarget::Crossfader, MidiTarget::Tempo];
    let effects = |out: &mut Vec<MidiTarget>, layer: Option<LayerRef>, chain: &[Effect]| {
        for (index, fx) in chain.iter().enumerate() {
            let effect = EffectRef { layer, index };
            out.push(MidiTarget::EffectMix { effect });
            out.push(MidiTarget::EffectBypass { effect });
            let n = fx.ty().map_or(0, |t| t.channels.len());
            out.extend((0..n).map(|param| MidiTarget::EffectParam { effect, param }));
        }
    };
    for (l, layer) in comp.layers.iter().enumerate() {
        let r = LayerRef::Index(l);
        out.extend([
            MidiTarget::LayerOpacity { layer: r },
            MidiTarget::LayerBypass { layer: r },
            MidiTarget::LayerSolo { layer: r },
        ]);
        out.extend(
            (0..layer.clips.len())
                .filter(|&c| layer.clips[c].is_some())
                .map(|column| MidiTarget::ClipLaunch { layer: l, column }),
        );
        effects(&mut out, Some(r), &layer.effects);
    }
    effects(&mut out, None, &comp.effects);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipSource, GeneratorKind, MidiEffect, apply_midi};

    fn comp() -> Composition {
        let mut c = Composition::new(64, 64, 2, 3);
        c.set_clip(1, 2, ClipSource::Generator { kind: GeneratorKind::ALL[0] });
        c.layers[0].effects.push(Effect::new("blur"));
        c.effects.push(Effect::new("invert"));
        c
    }

    fn apply(c: &mut Composition, address: &str, value: Option<f32>) -> MidiEffect {
        let action = resolume_action(address, value, c).unwrap_or_else(|| panic!("{address} isn't understood"));
        apply_midi(c, action, 1.0, 0)
    }

    #[test]
    fn layer_and_composition_faders() {
        let mut c = comp();
        apply(&mut c, "/composition/layers/2/video/opacity", Some(0.25));
        assert_eq!(c.layers[1].opacity, 0.25);
        apply(&mut c, "/composition/master", Some(0.5));
        assert_eq!(c.master, 0.5);
        apply(&mut c, "/composition/crossfader/phase", Some(0.75));
        assert_eq!(c.crossfader.position, 0.75);
        apply(&mut c, "/composition/layers/1/bypassed", Some(1.0));
        assert!(c.layers[0].bypass);
        apply(&mut c, "/composition/tempocontroller/tempo", Some(0.0));
        assert_eq!(c.bpm, BPM_RANGE.0);
        // Layer 3 doesn't exist; layer 0 isn't a Resolume number.
        let action = resolume_action("/composition/layers/3/video/opacity", Some(1.0), &c).unwrap();
        assert_eq!(apply_midi(&mut c, action, 1.0, 0), MidiEffect::Ignored);
        assert!(resolume_action("/composition/layers/0/video/opacity", Some(1.0), &c).is_none());
    }

    #[test]
    fn clip_connect_launches_on_press() {
        let mut c = comp();
        assert_eq!(apply(&mut c, "/composition/layers/2/clips/3/connect", Some(1.0)), MidiEffect::Changed);
        assert_eq!(c.layers[1].playing_column(), Some(2));
        assert_eq!(current_value(MidiTarget::ClipLaunch { layer: 1, column: 2 }, &c, 0), Some(1.0));
        // The release only matters for Piano clips.
        apply(&mut c, "/composition/layers/2/clips/3/connect", Some(0.0));
        assert_eq!(c.layers[1].playing_column(), Some(2));
        assert!(resolume_action("/composition/columns/1/connect", Some(0.0), &c).is_none(), "no release action");
    }

    #[test]
    fn effects_by_name() {
        let mut c = comp();
        apply(&mut c, "/composition/layers/1/video/effects/blur/effect/radius", Some(0.5));
        assert_eq!(c.layers[0].effects[0].param("radius").unwrap().value, 0.5);
        apply(&mut c, "/composition/layers/1/video/effects/blur/opacity", Some(0.3));
        assert_eq!(c.layers[0].effects[0].mix.value, 0.3);
        apply(&mut c, "/composition/video/effects/invert/bypassed", Some(1.0));
        assert!(!c.effects[0].enabled);
        // Resolume's own effect names find the openVJ effect that replaces them.
        assert!(resolume_target("/composition/layers/1/video/effects/gaussianblur/bypassed", &c).is_some());
        assert!(resolume_target("/composition/layers/1/video/effects/edges/bypassed", &c).is_none());
    }

    #[test]
    fn selected_layer() {
        let mut c = comp();
        let action = resolume_action("/composition/selectedlayer/video/opacity", Some(0.1), &c).unwrap();
        apply_midi(&mut c, action, 1.0, 1);
        assert_eq!(c.layers[1].opacity, 0.1);
    }

    #[test]
    fn addresses_round_trip() {
        let c = comp();
        let targets = output_targets(&c);
        assert!(targets.contains(&MidiTarget::ClipLaunch { layer: 1, column: 2 }));
        for t in targets {
            let address = resolume_address(t, &c).unwrap_or_else(|| panic!("{t:?} has no address"));
            assert_eq!(resolume_target(&address, &c), Some(t), "{address}");
            assert!(current_value(t, &c, 0).is_some(), "{address}");
        }
    }

    #[test]
    fn learned_addresses() {
        let mut c = comp();
        c.midi.learn_osc("/1/fader1", MidiTarget::Master);
        let actions = c.midi.handle_osc("/1/fader1", Some(0.7));
        assert_eq!(actions.len(), 1);
        apply_midi(&mut c, actions[0], 1.0, 0);
        assert!((c.master - 0.7).abs() < 1e-6);
        // Re-learning moves the address; a push button toggles a switch.
        c.midi.learn_osc("/1/fader1", MidiTarget::Crossfader);
        assert_eq!(c.midi.osc.len(), 1);
        c.midi.learn_osc("/1/push1", MidiTarget::LayerBypass { layer: LayerRef::Index(0) });
        for a in c.midi.handle_osc("/1/push1", Some(1.0)) {
            apply_midi(&mut c, a, 1.0, 0);
        }
        assert!(c.layers[0].bypass);
        assert!(c.midi.handle_osc("/1/push1", Some(0.0)).is_empty(), "release does nothing on a toggle");
        // Saved with the composition.
        let json = serde_json::to_string(&c.midi).unwrap();
        let back: MidiMap = serde_json::from_str(&json).unwrap();
        assert_eq!(back.osc, c.midi.osc);
    }
}
