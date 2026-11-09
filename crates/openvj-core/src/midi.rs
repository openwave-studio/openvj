//! MIDI mappings: which controller button, pad or knob drives which part of a composition.
//!
//! This module is pure data and logic. Opening MIDI ports is the app's job; it feeds raw
//! messages to [`MidiMap::handle`] and applies the resulting [`MidiAction`]s with
//! [`apply_midi`].

use serde::{Deserialize, Serialize};

use crate::{BlendMode, Composition};

/// A message from a MIDI controller, as far as mappings care.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiMessage {
    /// Note on with a velocity above zero.
    NoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    /// Note off, or note on with velocity zero.
    NoteOff {
        channel: u8,
        note: u8,
    },
    ControlChange {
        channel: u8,
        controller: u8,
        value: u8,
    },
}

impl MidiMessage {
    /// Parses a raw channel message. Clock, sysex and other messages return `None`.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let [status, data1, rest @ ..] = bytes else { return None };
        let data2 = rest.first().copied().unwrap_or(0) & 0x7f;
        let (channel, data1) = (status & 0x0f, data1 & 0x7f);
        Some(match status & 0xf0 {
            0x80 => MidiMessage::NoteOff { channel, note: data1 },
            0x90 if data2 == 0 => MidiMessage::NoteOff { channel, note: data1 },
            0x90 => MidiMessage::NoteOn { channel, note: data1, velocity: data2 },
            0xb0 => MidiMessage::ControlChange { channel, controller: data1, value: data2 },
            _ => return None,
        })
    }

    /// The control this message came from.
    pub fn control(self) -> MidiControl {
        match self {
            MidiMessage::NoteOn { channel, note, .. } | MidiMessage::NoteOff { channel, note } => {
                MidiControl { kind: MidiKind::Note, channel, number: note }
            }
            MidiMessage::ControlChange { channel, controller, .. } => {
                MidiControl { kind: MidiKind::Cc, channel, number: controller }
            }
        }
    }

    /// The message's value, normalised to 0..1 (velocity for notes, 0 for note off).
    pub fn value(self) -> f32 {
        match self {
            MidiMessage::NoteOn { velocity, .. } => velocity as f32 / 127.0,
            MidiMessage::NoteOff { .. } => 0.0,
            MidiMessage::ControlChange { value, .. } => value as f32 / 127.0,
        }
    }

    /// True when a button goes down: a note on, or a CC above zero.
    pub fn is_press(self) -> bool {
        match self {
            MidiMessage::NoteOn { .. } => true,
            MidiMessage::NoteOff { .. } => false,
            MidiMessage::ControlChange { value, .. } => value > 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MidiKind {
    Note,
    Cc,
}

/// One physical control: a note or controller number on a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MidiControl {
    pub kind: MidiKind,
    /// 0..15 (shown to people as 1..16).
    pub channel: u8,
    pub number: u8,
}

impl MidiControl {
    pub fn label(&self) -> String {
        let kind = match self.kind {
            MidiKind::Note => "Note",
            MidiKind::Cc => "CC",
        };
        format!("{kind} {} ch {}", self.number, self.channel + 1)
    }
}

/// A layer by position, or whichever layer is selected when the message arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerRef {
    Index(usize),
    Selected,
}

impl LayerRef {
    fn resolve(self, selected: usize) -> usize {
        match self {
            LayerRef::Index(i) => i,
            LayerRef::Selected => selected,
        }
    }
}

/// An effect in a layer's chain, or in the composition's chain when `layer` is `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectRef {
    pub layer: Option<LayerRef>,
    pub index: usize,
}

/// What a mapping controls.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MidiTarget {
    Master,
    LayerOpacity {
        layer: LayerRef,
    },
    LayerBypass {
        layer: LayerRef,
    },
    LayerSolo {
        layer: LayerRef,
    },
    LayerSelect {
        layer: LayerRef,
    },
    LayerClear {
        layer: LayerRef,
    },
    LayerBlend {
        layer: LayerRef,
        blend: BlendMode,
    },
    /// A fader that steps through the blend modes.
    LayerBlendFader {
        layer: LayerRef,
    },
    LayerNextClip {
        layer: LayerRef,
    },
    LayerPrevClip {
        layer: LayerRef,
    },
    ClipLaunch {
        layer: usize,
        column: usize,
    },
    ColumnLaunch {
        column: usize,
    },
    /// Launches the column after (or before) the last one launched.
    ColumnStep {
        forward: bool,
    },
    /// Opens the next (or previous) deck.
    DeckStep {
        forward: bool,
    },
    DeckSelect {
        deck: usize,
    },
    ClearAll,
    EffectParam {
        effect: EffectRef,
        param: usize,
    },
    EffectMix {
        effect: EffectRef,
    },
    EffectBypass {
        effect: EffectRef,
    },
    TempoTap,
    TempoResync,
    /// Sets the tempo across [`BPM_RANGE`].
    Tempo,
}

/// The tempo a full-range fader on [`MidiTarget::Tempo`] covers (Resolume's range).
pub const BPM_RANGE: (f32, f32) = (20.0, 500.0);

impl MidiTarget {
    /// True for targets that hold a continuous value (a fader makes sense).
    pub fn is_ranged(&self) -> bool {
        matches!(
            self,
            MidiTarget::Master
                | MidiTarget::LayerOpacity { .. }
                | MidiTarget::LayerBlendFader { .. }
                | MidiTarget::EffectParam { .. }
                | MidiTarget::EffectMix { .. }
                | MidiTarget::Tempo
        )
    }

    /// True for on/off targets.
    pub fn is_switch(&self) -> bool {
        matches!(self, MidiTarget::LayerBypass { .. } | MidiTarget::LayerSolo { .. } | MidiTarget::EffectBypass { .. })
    }
}

/// How a control's messages turn into changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MidiBehaviour {
    /// The control's value sets the target (knobs and faders).
    #[default]
    Fader,
    /// Each press flips the target between off and on (or min and max).
    Toggle,
    /// Pressing turns the target on (or launches it); releasing does nothing.
    Press,
    /// On while held, off when released.
    Hold,
}

/// One control driving one target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MidiMapping {
    pub control: MidiControl,
    pub target: MidiTarget,
    #[serde(default)]
    pub behaviour: MidiBehaviour,
    /// The part of the target's 0..1 range the control covers.
    #[serde(default)]
    pub min: f32,
    #[serde(default = "one")]
    pub max: f32,
    /// The controller this mapping listens to. While a controller with this name is
    /// connected, the same note or CC from other controllers is ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
}

fn one() -> f32 {
    1.0
}

impl MidiMapping {
    pub fn new(control: MidiControl, target: MidiTarget, behaviour: MidiBehaviour) -> Self {
        Self { control, target, behaviour, min: 0.0, max: 1.0, device: None }
    }
}

/// A set of mappings, usually for one controller (like a Resolume MIDI shortcut preset).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MidiMap {
    /// The preset's name, such as "Circuit Tracks".
    pub name: String,
    /// The controller most of the mappings are for, for display. Each mapping keeps its own.
    pub device: Option<String>,
    pub mappings: Vec<MidiMapping>,
}

/// What a single message asks for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MidiInput {
    /// Set a ranged target to this 0..1 value, or a switch on when it is ≥ 0.5.
    Set(f32),
    /// Flip a switch, or move a ranged target between its min and max.
    Toggle { min: f32, max: f32 },
    /// Launch or trigger the target.
    Trigger,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MidiAction {
    pub target: MidiTarget,
    pub input: MidiInput,
}

impl MidiMap {
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    /// The actions one message triggers, in mapping order.
    pub fn handle(&self, msg: MidiMessage) -> Vec<MidiAction> {
        self.handle_from(msg, "", &[])
    }

    /// Like [`handle`](Self::handle), for a message from port `port` while `connected`
    /// ports are open: a mapping made for a device that is connected only answers that
    /// device.
    pub fn handle_from(&self, msg: MidiMessage, port: &str, connected: &[String]) -> Vec<MidiAction> {
        let control = msg.control();
        let press = msg.is_press();
        let listens = |m: &MidiMapping| match &m.device {
            Some(d) if connected.iter().any(|p| same_device(p, d)) => same_device(port, d),
            _ => true,
        };
        self.mappings
            .iter()
            .filter(|m| m.control == control && listens(m))
            .filter_map(|m| {
                let input = if m.target.is_ranged() || m.target.is_switch() {
                    match m.behaviour {
                        MidiBehaviour::Fader => MidiInput::Set(m.min + (m.max - m.min) * msg.value()),
                        MidiBehaviour::Toggle if press => MidiInput::Toggle { min: m.min, max: m.max },
                        MidiBehaviour::Press if press => MidiInput::Set(m.max),
                        MidiBehaviour::Hold => MidiInput::Set(if press { m.max } else { m.min }),
                        _ => return None,
                    }
                } else if press {
                    MidiInput::Trigger
                } else {
                    return None;
                };
                Some(MidiAction { target: m.target, input })
            })
            .collect()
    }
}

/// True if a port name and a preset's device name refer to the same controller. Port names
/// differ between systems ("Circuit Tracks MIDI" on macOS, "Circuit Tracks:Circuit Tracks
/// MIDI 1 24:0" on Linux), so this compares letters and digits only and accepts either
/// containing the other.
pub fn same_device(port: &str, device: &str) -> bool {
    let norm = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect::<String>();
    let (p, d) = (norm(port), norm(device));
    !p.is_empty() && !d.is_empty() && (p.contains(&d) || d.contains(&p))
}

/// Things an action asks of the app, beyond changing the composition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MidiEffect {
    /// The composition changed.
    Changed,
    /// Nothing to do (the target doesn't exist in this composition).
    Ignored,
    SelectLayer(usize),
    TapTempo,
    ResyncTempo,
    /// Launch the next (or previous) column; the app knows which column was launched last.
    StepColumn {
        forward: bool,
    },
    /// Open this deck.
    OpenDeck(usize),
}

/// Applies one action to a composition at time `now`. `selected` is the selected layer.
pub fn apply_midi(comp: &mut Composition, action: MidiAction, now: f64, selected: usize) -> MidiEffect {
    use MidiEffect::*;

    fn ranged(p: &mut f32, input: MidiInput) {
        *p = match input {
            MidiInput::Set(v) => v,
            MidiInput::Toggle { min, max } => {
                if *p < (min + max) / 2.0 {
                    max
                } else {
                    min
                }
            }
            MidiInput::Trigger => *p,
        }
        .clamp(0.0, 1.0);
    }
    fn switch(b: &mut bool, input: MidiInput) {
        *b = match input {
            MidiInput::Set(v) => v >= 0.5,
            MidiInput::Toggle { .. } => !*b,
            MidiInput::Trigger => true,
        };
    }

    let input = action.input;
    let n_layers = comp.layers.len();
    let layer_of = |r: LayerRef| Some(r.resolve(selected)).filter(|&l| l < n_layers);
    match action.target {
        MidiTarget::Master => ranged(&mut comp.master, input),
        MidiTarget::LayerOpacity { layer } => {
            let Some(l) = layer_of(layer) else { return Ignored };
            ranged(&mut comp.layers[l].opacity, input);
        }
        MidiTarget::LayerBypass { layer } => {
            let Some(l) = layer_of(layer) else { return Ignored };
            switch(&mut comp.layers[l].bypass, input);
        }
        MidiTarget::LayerSolo { layer } => {
            let Some(l) = layer_of(layer) else { return Ignored };
            switch(&mut comp.layers[l].solo, input);
        }
        MidiTarget::LayerSelect { layer } => return layer_of(layer).map_or(Ignored, SelectLayer),
        MidiTarget::LayerClear { layer } => {
            let Some(l) = layer_of(layer) else { return Ignored };
            comp.clear_layer(l, now);
        }
        MidiTarget::LayerBlend { layer, blend } => {
            let Some(l) = layer_of(layer) else { return Ignored };
            comp.layers[l].blend = blend;
        }
        MidiTarget::LayerNextClip { layer } | MidiTarget::LayerPrevClip { layer } => {
            let Some(l) = layer_of(layer) else { return Ignored };
            let forward = matches!(action.target, MidiTarget::LayerNextClip { .. });
            let Some(col) = step_clip(comp, l, forward) else { return Ignored };
            comp.trigger(l, col, now);
        }
        MidiTarget::ClipLaunch { layer, column } => {
            if comp.clip(layer, column).is_none() {
                return Ignored;
            }
            comp.trigger(layer, column, now);
        }
        MidiTarget::ColumnLaunch { column } => {
            if column >= comp.columns.len() {
                return Ignored;
            }
            comp.trigger_column(column, now);
        }
        MidiTarget::LayerBlendFader { layer } => {
            let Some(l) = layer_of(layer) else { return Ignored };
            let MidiInput::Set(v) = input else { return Ignored };
            let n = BlendMode::ALL.len();
            comp.layers[l].blend = BlendMode::ALL[((v.clamp(0.0, 1.0) * n as f32) as usize).min(n - 1)];
        }
        MidiTarget::ColumnStep { forward } => return StepColumn { forward },
        MidiTarget::DeckStep { forward } => {
            let n = comp.decks.len().max(1);
            let c = comp.current_deck;
            return OpenDeck(if forward { (c + 1) % n } else { (c + n - 1) % n });
        }
        MidiTarget::DeckSelect { deck } => {
            return if deck < comp.decks.len() { OpenDeck(deck) } else { Ignored };
        }
        MidiTarget::ClearAll => comp.clear_all(now),
        MidiTarget::EffectParam { effect, param } => {
            let Some(fx) = effect_mut(comp, effect, selected) else { return Ignored };
            // `param` is a channel index of the effect type.
            let Some(key) = fx.ty().and_then(|t| t.channels.get(param).map(|c| c.key.clone())) else {
                return Ignored;
            };
            let Some(p) = fx.param_mut(&key) else { return Ignored };
            ranged(&mut p.value, input);
        }
        MidiTarget::EffectMix { effect } => {
            let Some(fx) = effect_mut(comp, effect, selected) else { return Ignored };
            ranged(&mut fx.mix.value, input);
        }
        MidiTarget::EffectBypass { effect } => {
            let Some(fx) = effect_mut(comp, effect, selected) else { return Ignored };
            let mut bypassed = !fx.enabled;
            switch(&mut bypassed, input);
            fx.enabled = !bypassed;
        }
        MidiTarget::TempoTap => return TapTempo,
        MidiTarget::TempoResync => return ResyncTempo,
        MidiTarget::Tempo => {
            let (lo, hi) = BPM_RANGE;
            let mut v = (comp.bpm - lo) / (hi - lo);
            ranged(&mut v, input);
            comp.bpm = ((lo + v * (hi - lo)) * 10.0).round() / 10.0;
        }
    }
    Changed
}

fn effect_mut(comp: &mut Composition, effect: EffectRef, selected: usize) -> Option<&mut crate::Effect> {
    let chain = match effect.layer {
        None => &mut comp.effects,
        Some(layer) => &mut comp.layers.get_mut(layer.resolve(selected))?.effects,
    };
    chain.get_mut(effect.index)
}

/// The next (or previous) filled slot on a layer after the one playing, wrapping around.
fn step_clip(comp: &Composition, layer: usize, forward: bool) -> Option<usize> {
    let l = comp.layers.get(layer)?;
    let n = l.clips.len();
    let start = l.playing_column();
    (1..=n)
        .map(|step| match start {
            Some(s) if forward => (s + step) % n,
            Some(s) => (s + n * 2 - step) % n,
            None if forward => step - 1,
            None => n - step,
        })
        .find(|&c| l.clips[c].is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipSource, Effect, GeneratorKind};

    fn note(n: u8) -> MidiControl {
        MidiControl { kind: MidiKind::Note, channel: 0, number: n }
    }
    fn cc(n: u8) -> MidiControl {
        MidiControl { kind: MidiKind::Cc, channel: 0, number: n }
    }

    fn comp() -> Composition {
        let mut c = Composition::new(64, 64, 2, 3);
        for col in [0, 2] {
            c.set_clip(0, col, ClipSource::Generator { kind: GeneratorKind::ALL[0] });
        }
        c.layers[1].effects.push(Effect::new("blur"));
        c
    }

    #[test]
    fn parses_channel_messages() {
        assert_eq!(
            MidiMessage::parse(&[0x91, 60, 100]),
            Some(MidiMessage::NoteOn { channel: 1, note: 60, velocity: 100 })
        );
        assert_eq!(MidiMessage::parse(&[0x90, 60, 0]), Some(MidiMessage::NoteOff { channel: 0, note: 60 }));
        assert_eq!(MidiMessage::parse(&[0x8f, 60, 10]), Some(MidiMessage::NoteOff { channel: 15, note: 60 }));
        assert_eq!(
            MidiMessage::parse(&[0xb5, 7, 127]),
            Some(MidiMessage::ControlChange { channel: 5, controller: 7, value: 127 })
        );
        assert_eq!(MidiMessage::parse(&[0xf8]), None);
        assert_eq!(MidiMessage::parse(&[0xe0, 0, 64]), None);
    }

    #[test]
    fn fader_scales_into_the_mapped_range() {
        let mut m = MidiMapping::new(cc(7), MidiTarget::Master, MidiBehaviour::Fader);
        m.min = 0.2;
        m.max = 0.6;
        let map = MidiMap { mappings: vec![m], ..Default::default() };
        let actions = map.handle(MidiMessage::ControlChange { channel: 0, controller: 7, value: 127 });
        assert_eq!(actions.len(), 1);
        let MidiInput::Set(v) = actions[0].input else { panic!() };
        assert!((v - 0.6).abs() < 1e-6);
        // Another channel doesn't match.
        assert!(map.handle(MidiMessage::ControlChange { channel: 1, controller: 7, value: 127 }).is_empty());
    }

    #[test]
    fn buttons_trigger_on_press_only() {
        let map = MidiMap {
            mappings: vec![MidiMapping::new(
                note(36),
                MidiTarget::ClipLaunch { layer: 0, column: 2 },
                MidiBehaviour::Press,
            )],
            ..Default::default()
        };
        let mut c = comp();
        for a in map.handle(MidiMessage::NoteOn { channel: 0, note: 36, velocity: 90 }) {
            assert_eq!(apply_midi(&mut c, a, 1.0, 0), MidiEffect::Changed);
        }
        assert_eq!(c.layers[0].playing_column(), Some(2));
        assert!(map.handle(MidiMessage::NoteOff { channel: 0, note: 36 }).is_empty());
    }

    #[test]
    fn toggle_and_hold_switches() {
        let toggle =
            MidiMapping::new(note(1), MidiTarget::LayerBypass { layer: LayerRef::Index(1) }, MidiBehaviour::Toggle);
        let hold = MidiMapping::new(
            note(2),
            MidiTarget::EffectBypass { effect: EffectRef { layer: Some(LayerRef::Selected), index: 0 } },
            MidiBehaviour::Hold,
        );
        let map = MidiMap { mappings: vec![toggle, hold], ..Default::default() };
        let mut c = comp();
        let send = |c: &mut Composition, msg| {
            for a in map.handle(msg) {
                apply_midi(c, a, 0.0, 1);
            }
        };
        send(&mut c, MidiMessage::NoteOn { channel: 0, note: 1, velocity: 1 });
        send(&mut c, MidiMessage::NoteOff { channel: 0, note: 1 });
        assert!(c.layers[1].bypass);
        send(&mut c, MidiMessage::NoteOn { channel: 0, note: 1, velocity: 1 });
        assert!(!c.layers[1].bypass);

        send(&mut c, MidiMessage::NoteOn { channel: 0, note: 2, velocity: 1 });
        assert!(!c.layers[1].effects[0].enabled);
        send(&mut c, MidiMessage::NoteOff { channel: 0, note: 2 });
        assert!(c.layers[1].effects[0].enabled);
    }

    #[test]
    fn missing_targets_are_ignored() {
        let mut c = comp();
        let fx = EffectRef { layer: Some(LayerRef::Index(0)), index: 0 };
        let set = MidiInput::Set(1.0);
        for target in [
            MidiTarget::LayerOpacity { layer: LayerRef::Index(9) },
            MidiTarget::ClipLaunch { layer: 0, column: 1 },
            MidiTarget::EffectMix { effect: fx },
            // Blur has no second parameter.
            MidiTarget::EffectParam { effect: EffectRef { layer: Some(LayerRef::Index(1)), index: 0 }, param: 1 },
        ] {
            assert_eq!(
                apply_midi(&mut c, MidiAction { target, input: set }, 0.0, 0),
                MidiEffect::Ignored,
                "{target:?}"
            );
        }
    }

    #[test]
    fn next_clip_wraps_and_skips_empty_slots() {
        let mut c = comp();
        let next =
            MidiAction { target: MidiTarget::LayerNextClip { layer: LayerRef::Index(0) }, input: MidiInput::Trigger };
        apply_midi(&mut c, next, 0.0, 0);
        assert_eq!(c.layers[0].playing_column(), Some(0));
        apply_midi(&mut c, next, 0.1, 0);
        assert_eq!(c.layers[0].playing_column(), Some(2));
        apply_midi(&mut c, next, 0.2, 0);
        assert_eq!(c.layers[0].playing_column(), Some(0));
        let prev =
            MidiAction { target: MidiTarget::LayerPrevClip { layer: LayerRef::Index(0) }, input: MidiInput::Trigger };
        apply_midi(&mut c, prev, 0.3, 0);
        assert_eq!(c.layers[0].playing_column(), Some(2));
    }

    #[test]
    fn device_mappings_only_answer_their_device_while_it_is_connected() {
        let mut m = MidiMapping::new(cc(1), MidiTarget::Master, MidiBehaviour::Fader);
        m.device = Some("Circuit Tracks MIDI".into());
        let any = MidiMapping::new(cc(1), MidiTarget::ClearAll, MidiBehaviour::Press);
        let map = MidiMap { mappings: vec![m, any], ..Default::default() };
        let msg = MidiMessage::ControlChange { channel: 0, controller: 1, value: 127 };
        let ports = vec!["Circuit Tracks MIDI".to_owned(), "XONE:K2".to_owned()];
        assert_eq!(map.handle_from(msg, "Circuit Tracks MIDI", &ports).len(), 2);
        assert_eq!(map.handle_from(msg, "XONE:K2", &ports).len(), 1);
        // With the Circuit Tracks unplugged, anything can drive it.
        assert_eq!(map.handle_from(msg, "XONE:K2", &ports[1..]).len(), 2);
    }

    #[test]
    fn device_names_match_loosely() {
        assert!(same_device("Circuit Tracks:Circuit Tracks MIDI 1 24:0", "Circuit Tracks MIDI"));
        assert!(same_device("MIDI Mix", "midimix"));
        assert!(!same_device("APC40 mkII", "Circuit Tracks"));
        assert!(!same_device("", "Circuit Tracks"));
    }

    #[test]
    fn blend_fader_steps_through_modes() {
        let mut c = comp();
        let target = MidiTarget::LayerBlendFader { layer: LayerRef::Index(0) };
        apply_midi(&mut c, MidiAction { target, input: MidiInput::Set(1.0) }, 0.0, 0);
        assert_eq!(c.layers[0].blend, *BlendMode::ALL.last().unwrap());
        apply_midi(&mut c, MidiAction { target, input: MidiInput::Set(0.0) }, 0.0, 0);
        assert_eq!(c.layers[0].blend, BlendMode::ALL[0]);
    }

    #[test]
    fn deck_steps_wrap() {
        let mut c = comp();
        c.add_deck();
        c.switch_deck(0);
        let step = |forward| MidiAction { target: MidiTarget::DeckStep { forward }, input: MidiInput::Trigger };
        assert_eq!(apply_midi(&mut c, step(true), 0.0, 0), MidiEffect::OpenDeck(1));
        assert_eq!(apply_midi(&mut c, step(false), 0.0, 0), MidiEffect::OpenDeck(1));
        let select = |deck| MidiAction { target: MidiTarget::DeckSelect { deck }, input: MidiInput::Trigger };
        assert_eq!(apply_midi(&mut c, select(5), 0.0, 0), MidiEffect::Ignored);
    }

    #[test]
    fn tempo_fader_covers_resolume_range() {
        let mut c = comp();
        apply_midi(&mut c, MidiAction { target: MidiTarget::Tempo, input: MidiInput::Set(0.25) }, 0.0, 0);
        assert_eq!(c.bpm, 140.0);
    }

    #[test]
    fn map_round_trips_through_json() {
        let map = MidiMap {
            name: "Test".into(),
            device: Some("Circuit Tracks".into()),
            mappings: vec![MidiMapping::new(
                cc(80),
                MidiTarget::EffectParam { effect: EffectRef { layer: None, index: 0 }, param: 2 },
                MidiBehaviour::Fader,
            )],
        };
        let json = serde_json::to_string(&map).unwrap();
        assert_eq!(serde_json::from_str::<MidiMap>(&json).unwrap(), map);
    }
}
