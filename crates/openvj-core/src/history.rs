//! Undo and redo of composition edits in the Mixer.
//!
//! The app brackets every UI action with [`History::begin`] and [`History::end`], which
//! compare the composition's saved part (what a .json file holds) before and after. When
//! it changed, the values that differ, before and after, become a step. Launching and
//! clearing don't touch the saved part and tempo is left out, so they are never steps;
//! nor are values moved by MIDI, OSC, audio or animation, which don't go through UI
//! actions. Undo puts back only the values its step changed, so a fader moved from a
//! controller since stays where it is. Restoring goes through
//! [`Composition::restore_model`], which keeps what is playing.
//!
//! Consecutive changes to the same values within a moment (a fader drag, typing in a
//! field) merge into one step.

use std::time::{Duration, Instant};

use serde_json::Value;

use crate::Composition;

/// How many steps are kept.
pub const MAX_STEPS: usize = 100;
/// Changes to the same values closer together than this are one step.
const MERGE_WINDOW: Duration = Duration::from_millis(800);

/// Saved values that follow the tempo, not the VJ's edits.
const NOT_EDITS: &[&str] = &["bpm"];

/// One value an edit changed, by its JSON pointer path (without the leading `/`). `None`
/// is a value that wasn't there.
#[derive(Clone)]
struct Change {
    path: String,
    before: Option<Value>,
    after: Option<Value>,
}

struct Step {
    changes: Vec<Change>,
    label: String,
}

#[derive(Default)]
pub struct History {
    undo: Vec<Step>,
    redo: Vec<Step>,
    /// The snapshot taken by `begin`, while an action runs.
    pending: Option<Value>,
    /// What the last step changed and when, to merge a gesture into one step.
    last: Option<(Vec<String>, Instant)>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets every step (New and Open).
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// What Undo would take back, such as "layer 2 opacity".
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|s| s.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|s| s.label.as_str())
    }

    /// Call before a UI action runs.
    pub fn begin(&mut self, comp: &Composition) {
        self.pending = snapshot(comp);
    }

    /// Call after the action: records a step if the composition's saved part changed.
    pub fn end(&mut self, comp: &Composition) {
        self.end_at(comp, Instant::now());
    }

    fn end_at(&mut self, comp: &Composition, at: Instant) {
        let Some(before) = self.pending.take() else { return };
        let Some(after) = snapshot(comp) else { return };
        if before == after {
            return;
        }
        let mut paths = Vec::new();
        diff_paths(&before, &after, String::new(), &mut paths);
        paths.retain(|p| !NOT_EDITS.contains(&p.as_str()));
        if paths.is_empty() {
            return;
        }
        let value = |v: &Value, p: &str| v.pointer(&format!("/{p}")).cloned();
        let changes: Vec<Change> = paths
            .iter()
            .map(|p| Change { path: p.clone(), before: value(&before, p), after: value(&after, p) })
            .collect();
        let merge = matches!(&self.last, Some((p, t)) if *p == paths && at.duration_since(*t) < MERGE_WINDOW);
        self.last = Some((paths.clone(), at));
        self.redo.clear();
        if let Some(step) = self.undo.last_mut().filter(|_| merge) {
            // Same values within a gesture: keep the first before, take the latest after.
            for (old, new) in step.changes.iter_mut().zip(changes) {
                old.after = new.after;
            }
            return;
        }
        self.undo.push(Step { changes, label: describe(&paths) });
        if self.undo.len() > MAX_STEPS {
            self.undo.remove(0);
        }
    }

    /// Takes back the last step. Returns its label, or None when there was nothing to undo.
    pub fn undo(&mut self, comp: &mut Composition) -> Option<String> {
        self.pending = None;
        self.last = None;
        let step = self.undo.pop()?;
        let label = step.label.clone();
        let restored = apply(comp, &step.changes, |c| c.before.as_ref());
        self.redo.push(step);
        restored.then_some(label)
    }

    /// Does the last undone step again.
    pub fn redo(&mut self, comp: &mut Composition) -> Option<String> {
        self.pending = None;
        self.last = None;
        let step = self.redo.pop()?;
        let label = step.label.clone();
        let restored = apply(comp, &step.changes, |c| c.after.as_ref());
        self.undo.push(step);
        restored.then_some(label)
    }
}

/// Sets each changed value to its before or after, keeping everything else as it is now.
fn apply(comp: &mut Composition, changes: &[Change], pick: impl Fn(&Change) -> Option<&Value>) -> bool {
    let Some(mut now) = snapshot(comp) else { return false };
    for change in changes {
        set(&mut now, &change.path, pick(change));
    }
    match serde_json::from_value::<Composition>(now) {
        Ok(model) => {
            comp.restore_model(model);
            true
        }
        Err(_) => false,
    }
}

/// Sets (or with `None` removes) the value at a pointer path.
fn set(root: &mut Value, path: &str, value: Option<&Value>) {
    let (parent, key) = match path.rfind('/') {
        Some(i) => (format!("/{}", &path[..i]), &path[i + 1..]),
        None => (String::new(), path),
    };
    let key = key.replace("~1", "/").replace("~0", "~");
    match root.pointer_mut(&parent) {
        Some(Value::Object(map)) => match value {
            Some(v) => {
                map.insert(key, v.clone());
            }
            None => {
                map.remove(&key);
            }
        },
        Some(Value::Array(items)) => {
            if let (Some(slot), Some(v)) = (key.parse::<usize>().ok().and_then(|i| items.get_mut(i)), value) {
                *slot = v.clone();
            }
        }
        _ => {}
    }
}

fn snapshot(comp: &Composition) -> Option<Value> {
    serde_json::to_value(comp).ok()
}

/// The paths of the values that differ, such as `layers/1/opacity`. Arrays that changed
/// length (a clip or effect added or removed) count as one path.
fn diff_paths(a: &Value, b: &Value, at: String, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let path = |k: &str| {
                let k = k.replace('~', "~0").replace('/', "~1");
                if at.is_empty() { k } else { format!("{at}/{k}") }
            };
            for (k, va) in x {
                let p = path(k);
                match y.get(k) {
                    Some(vb) => diff_paths(va, vb, p, out),
                    None => out.push(p),
                }
            }
            for k in y.keys().filter(|k| !x.contains_key(*k)) {
                out.push(path(k));
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() && !at.is_empty() => {
            for (i, (va, vb)) in x.iter().zip(y).enumerate() {
                diff_paths(va, vb, format!("{at}/{i}"), out);
            }
        }
        _ if a != b => out.push(at),
        _ => {}
    }
}

/// A short name for what changed, for the Undo button's tooltip.
fn describe(paths: &[String]) -> String {
    let Some(first) = paths.first() else { return "change".into() };
    let parts: Vec<&str> = first.split('/').collect();
    let effect = |rest: &[&str]| if rest.first() == Some(&"effects") { " effect" } else { "" };
    match parts.as_slice() {
        ["layers", l, "clips", ..] => {
            let what = if paths.len() > 1 { "clips" } else { "clip" };
            format!("{what} in layer {}", l.parse::<usize>().map_or(0, |l| l + 1))
        }
        ["layers", l, field, rest @ ..] => {
            let layer = l.parse::<usize>().map_or(0, |l| l + 1);
            let field = match *field {
                "effects" => "effects",
                "blend" => "blend mode",
                f => f,
            };
            format!("layer {layer} {field}{}", effect(rest).trim_start_matches(" effect"))
        }
        ["layers"] => "layers".into(),
        ["effects", ..] => "composition effects".into(),
        ["groups", ..] => "group".into(),
        ["decks", ..] | ["current_deck"] => "decks".into(),
        ["columns", ..] => "columns".into(),
        ["midi", ..] => "mappings".into(),
        ["crossfader", ..] => "crossfader".into(),
        [field, ..] => field.replace('_', " "),
        [] => "change".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipSource, Effect, GeneratorKind};

    fn comp() -> Composition {
        let mut c = Composition::new(64, 64, 2, 3);
        c.set_clip(0, 0, ClipSource::Generator { kind: GeneratorKind::ALL[0] });
        c
    }

    fn edit(h: &mut History, c: &mut Composition, at: Instant, f: impl FnOnce(&mut Composition)) {
        h.begin(c);
        f(c);
        h.end_at(c, at);
    }

    #[test]
    fn undo_and_redo_an_edit() {
        let (mut h, mut c) = (History::new(), comp());
        let t = Instant::now();
        edit(&mut h, &mut c, t, |c| c.layers[1].effects.push(Effect::new("blur")));
        assert!(h.can_undo());
        assert_eq!(h.undo(&mut c).as_deref(), Some("layer 2 effects"));
        assert!(c.layers[1].effects.is_empty());
        h.redo(&mut c);
        assert_eq!(c.layers[1].effects.len(), 1);
    }

    #[test]
    fn launches_are_not_steps() {
        let (mut h, mut c) = (History::new(), comp());
        edit(&mut h, &mut c, Instant::now(), |c| c.trigger(0, 0, 0.0));
        assert!(!h.can_undo());
    }

    #[test]
    fn a_drag_is_one_step() {
        let (mut h, mut c) = (History::new(), comp());
        let t = Instant::now();
        for i in 0..10 {
            edit(&mut h, &mut c, t + Duration::from_millis(i * 30), |c| c.layers[0].opacity = 1.0 - i as f32 * 0.08);
        }
        // Another fader is a new step, and so is the same fader a while later.
        edit(&mut h, &mut c, t + Duration::from_millis(400), |c| c.master = 0.5);
        edit(&mut h, &mut c, t + Duration::from_secs(3), |c| c.layers[0].opacity = 0.1);
        h.undo(&mut c);
        h.undo(&mut c);
        assert_eq!(c.master, 1.0);
        assert!((c.layers[0].opacity - 0.28).abs() < 1e-5);
        h.undo(&mut c);
        assert_eq!(c.layers[0].opacity, 1.0, "the whole drag is undone at once");
        assert!(!h.can_undo());
    }

    #[test]
    fn undo_keeps_playing_and_fades_out_removed_clips() {
        let (mut h, mut c) = (History::new(), comp());
        c.layers[1].transition_secs = 1.0;
        c.trigger(0, 0, 0.0);
        c.tick(1.0, 2.0);
        // Undoing a change elsewhere keeps the clip playing.
        edit(&mut h, &mut c, Instant::now(), |c| c.master = 0.5);
        h.undo(&mut c);
        assert_eq!(c.layers[0].playing_column(), Some(0));
        // A clip added and launched, then undone: the slot is empty and the layer fades out.
        let t = Instant::now() + Duration::from_secs(5);
        edit(&mut h, &mut c, t, |c| {
            c.set_clip(1, 1, ClipSource::Generator { kind: GeneratorKind::ALL[1] });
        });
        c.trigger(1, 1, 1.0);
        h.undo(&mut c);
        assert!(c.clip(1, 1).is_none());
        assert!(c.layers[1].active.is_none(), "the removed clip stops");
        assert!(c.layers[1].fade.is_some(), "and fades out");
        let from = c.layers[1].fade.unwrap().from.unwrap().clip_id;
        assert!(c.layers[1].clip_by_id(from).is_some(), "the fading clip is still there to draw");
        assert_eq!(c.layers[0].playing_column(), Some(0));
    }

    #[test]
    fn undo_keeps_values_moved_from_elsewhere() {
        let (mut h, mut c) = (History::new(), comp());
        edit(&mut h, &mut c, Instant::now(), |c| c.layers[1].effects.push(Effect::new("blur")));
        // A controller moves a fader and the tempo follows Link, outside any UI action.
        c.layers[0].opacity = 0.3;
        c.bpm = 128.0;
        h.undo(&mut c);
        assert!(c.layers[1].effects.is_empty());
        assert_eq!(c.layers[0].opacity, 0.3);
        assert_eq!(c.bpm, 128.0);
    }

    #[test]
    fn tempo_is_not_a_step() {
        let (mut h, mut c) = (History::new(), comp());
        edit(&mut h, &mut c, Instant::now(), |c| c.bpm = 140.0);
        assert!(!h.can_undo());
    }

    #[test]
    fn a_new_edit_clears_redo_and_history_is_capped() {
        let (mut h, mut c) = (History::new(), comp());
        let t = Instant::now();
        edit(&mut h, &mut c, t, |c| c.master = 0.5);
        h.undo(&mut c);
        assert!(h.can_redo());
        edit(&mut h, &mut c, t + Duration::from_secs(2), |c| c.master = 0.4);
        assert!(!h.can_redo());
        for i in 0..(MAX_STEPS + 20) {
            edit(&mut h, &mut c, t + Duration::from_secs(10 + i as u64), |c| c.master = (i % 7) as f32 / 10.0 + 0.01);
        }
        assert_eq!(h.undo.len(), MAX_STEPS);
    }
}
