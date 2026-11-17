//! The Synthesis layout's group tools: lock, undo, default and random for each section of
//! knobs, and for a whole row of sections at once, like Synesthesia's scene and meta
//! controls. Sections are named by a [`Key`]; this holds locks and undo history and works out
//! the new values, and the app writes them to the parameters.

use std::collections::{HashMap, HashSet};

use openvj_core::discover::Rng;

/// A section: the visual it belongs to (0 for the layer's own sections), and the
/// (scope, owner) its parameters are set with.
pub type Key = (u64, i32, i32);

/// One knob of a section, as it is now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Knob {
    pub index: usize,
    pub value: f32,
    pub reset: f32,
    /// Random may move it (not speed, timing or momentary inputs).
    pub random: bool,
}

/// Values to write: (index, value).
pub type Values = Vec<(usize, f32)>;

/// How many steps undo keeps per section and per row.
const UNDO_DEPTH: usize = 64;

#[derive(Debug)]
pub struct Groups {
    locked: HashSet<Key>,
    undo: HashMap<Key, Vec<Values>>,
    row_undo: HashMap<(u64, i32), Vec<Vec<(Key, Values)>>>,
    rng: Rng,
}

impl Default for Groups {
    fn default() -> Self {
        Self::new(crate::discover::seed())
    }
}

fn snapshot(knobs: &[Knob]) -> Values {
    knobs.iter().map(|k| (k.index, k.value)).collect()
}

fn push<T>(stack: &mut Vec<T>, v: T) {
    if stack.len() >= UNDO_DEPTH {
        stack.remove(0);
    }
    stack.push(v);
}

impl Groups {
    pub fn new(seed: u64) -> Self {
        Self { locked: HashSet::new(), undo: HashMap::new(), row_undo: HashMap::new(), rng: Rng::new(seed) }
    }

    pub fn is_locked(&self, key: Key) -> bool {
        self.locked.contains(&key)
    }

    pub fn toggle_lock(&mut self, key: Key) {
        if !self.locked.remove(&key) {
            self.locked.insert(key);
        }
    }

    #[cfg(test)]
    pub fn can_undo(&self, key: Key) -> bool {
        self.undo.get(&key).is_some_and(|s| !s.is_empty())
    }

    /// Remembers the section as it is, before a knob drag or another change.
    pub fn checkpoint(&mut self, key: Key, knobs: &[Knob]) {
        push(self.undo.entry(key).or_default(), snapshot(knobs));
    }

    fn random_values(&mut self, knobs: &[Knob]) -> Values {
        knobs.iter().filter(|k| k.random).map(|k| (k.index, self.rng.next_f32())).collect()
    }

    fn default_values(knobs: &[Knob]) -> Values {
        knobs.iter().filter(|k| k.value != k.reset).map(|k| (k.index, k.reset)).collect()
    }

    /// Random values for the section's knobs. A locked section stays as it is.
    pub fn random(&mut self, key: Key, knobs: &[Knob]) -> Values {
        if self.is_locked(key) {
            return Vec::new();
        }
        let v = self.random_values(knobs);
        if !v.is_empty() {
            self.checkpoint(key, knobs);
        }
        v
    }

    /// The section back to its defaults. A locked section stays as it is.
    pub fn default(&mut self, key: Key, knobs: &[Knob]) -> Values {
        if self.is_locked(key) {
            return Vec::new();
        }
        let v = Self::default_values(knobs);
        if !v.is_empty() {
            self.checkpoint(key, knobs);
        }
        v
    }

    /// The section as it was before its last change.
    pub fn undo(&mut self, key: Key) -> Values {
        self.undo.get_mut(&key).and_then(Vec::pop).unwrap_or_default()
    }

    fn row_op(&mut self, row: (u64, i32), sections: &[(Key, Vec<Knob>)], random: bool) -> Vec<(Key, Values)> {
        let mut before = Vec::new();
        let mut out = Vec::new();
        for (key, knobs) in sections {
            if self.is_locked(*key) {
                continue;
            }
            let v = if random { self.random_values(knobs) } else { Self::default_values(knobs) };
            if v.is_empty() {
                continue;
            }
            self.checkpoint(*key, knobs);
            before.push((*key, snapshot(knobs)));
            out.push((*key, v));
        }
        if !before.is_empty() {
            push(self.row_undo.entry(row).or_default(), before);
        }
        out
    }

    /// Random values for every unlocked section of a row.
    pub fn random_row(&mut self, row: (u64, i32), sections: &[(Key, Vec<Knob>)]) -> Vec<(Key, Values)> {
        self.row_op(row, sections, true)
    }

    /// Every unlocked section of a row back to its defaults.
    pub fn default_row(&mut self, row: (u64, i32), sections: &[(Key, Vec<Knob>)]) -> Vec<(Key, Values)> {
        self.row_op(row, sections, false)
    }

    /// Every section of a row as it was before the row's last random or default.
    pub fn undo_row(&mut self, row: (u64, i32)) -> Vec<(Key, Values)> {
        let out = self.row_undo.get_mut(&row).and_then(Vec::pop).unwrap_or_default();
        // The sections' own undo held the same step; drop it so it isn't undone twice.
        for (key, _) in &out {
            if let Some(s) = self.undo.get_mut(key) {
                s.pop();
            }
        }
        out
    }

    /// Whether the row has a step to undo.
    #[cfg(test)]
    pub fn can_undo_row(&self, row: (u64, i32)) -> bool {
        self.row_undo.get(&row).is_some_and(|s| !s.is_empty())
    }

    /// Whether every section listed is locked.
    pub fn all_locked(&self, keys: impl IntoIterator<Item = Key>) -> bool {
        let mut any = false;
        for k in keys {
            any = true;
            if !self.is_locked(k) {
                return false;
            }
        }
        any
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn knobs(values: &[f32]) -> Vec<Knob> {
        values.iter().enumerate().map(|(i, &v)| Knob { index: i, value: v, reset: 0.5, random: true }).collect()
    }

    fn apply(k: &mut [Knob], v: &Values) {
        for &(i, x) in v {
            k.iter_mut().find(|k| k.index == i).unwrap().value = x;
        }
    }

    #[test]
    fn random_then_undo_restores() {
        let mut g = Groups::new(7);
        let key = (1, 4, 0);
        let mut k = knobs(&[0.1, 0.2, 0.3]);
        let v = g.random(key, &k);
        assert_eq!(v.len(), 3);
        apply(&mut k, &v);
        assert!(g.can_undo(key));
        let u = g.undo(key);
        apply(&mut k, &u);
        assert_eq!(k.iter().map(|k| k.value).collect::<Vec<_>>(), vec![0.1, 0.2, 0.3]);
        assert!(!g.can_undo(key));
    }

    #[test]
    fn random_skips_fixed_knobs_and_locked_sections() {
        let mut g = Groups::new(7);
        let key = (1, 4, 0);
        let mut k = knobs(&[0.1, 0.2]);
        k[1].random = false;
        assert_eq!(g.random(key, &k).iter().map(|v| v.0).collect::<Vec<_>>(), vec![0]);
        g.toggle_lock(key);
        assert!(g.random(key, &k).is_empty());
        assert!(g.default(key, &k).is_empty());
    }

    #[test]
    fn default_resets_only_changed_knobs() {
        let mut g = Groups::new(7);
        let k = knobs(&[0.5, 0.9]);
        assert_eq!(g.default((1, 4, 0), &k), vec![(1, 0.5)]);
    }

    #[test]
    fn row_random_skips_locked_sections_and_undoes_as_one() {
        let mut g = Groups::new(3);
        let (a, b) = ((1, 4, 0), (1, 7, 0));
        let mut ka = knobs(&[0.1]);
        let mut kb = knobs(&[0.2]);
        g.toggle_lock(b);
        let out = g.random_row((1, 0), &[(a, ka.clone()), (b, kb.clone())]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, a);
        apply(&mut ka, &out[0].1);
        g.toggle_lock(b);
        let out = g.default_row((1, 0), &[(a, ka.clone()), (b, kb.clone())]);
        assert_eq!(out.len(), 2);
        apply(&mut ka, &out[0].1);
        apply(&mut kb, &out[1].1);
        // Undo the default, then the random.
        for (key, v) in g.undo_row((1, 0)) {
            apply(if key == a { &mut ka } else { &mut kb }, &v);
        }
        assert_eq!(kb[0].value, 0.2);
        for (key, v) in g.undo_row((1, 0)) {
            apply(if key == a { &mut ka } else { &mut kb }, &v);
        }
        assert_eq!(ka[0].value, 0.1);
        assert!(!g.can_undo_row((1, 0)));
        assert!(!g.can_undo(a));
    }
}
