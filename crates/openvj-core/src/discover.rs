//! Discover: stepping through the library's Not reviewed items to sort them, with a
//! parameter panel that can be randomized. Nothing here reaches the output; the app shows
//! the current item on its own preview.
//!
//! [`Queue`] is the list of items to review and what the VJ decided this session.
//! [`Knobs`] holds the current item's parameter values with locks, undo and reset.

use std::collections::BTreeMap;

use crate::library::{Kind, Library, Status};

/// In which order the queue comes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Order {
    /// Shuffled, so a long pack doesn't come all at once.
    #[default]
    Random,
    /// Pack by pack, by name within a pack.
    Pack,
}

/// What the VJ decided about the current item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Into My library, unchanged.
    Keep,
    /// The original into My library, plus a new preset with the current values (the app
    /// saves the preset file).
    Preset,
    /// Left Not reviewed, for another session.
    Skip,
    /// Into the Unwanted crate.
    Unwanted,
}

/// How many of each decision this session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Session {
    pub kept: usize,
    pub presets: usize,
    pub skipped: usize,
    pub unwanted: usize,
}

impl Session {
    pub fn reviewed(&self) -> usize {
        self.kept + self.presets + self.skipped + self.unwanted
    }
}

/// Whether an item can be reviewed in Discover: Not reviewed, not a saved preset (those are
/// always kept), and from one of `packs` (all when empty).
fn reviewable(lib: &Library, id: &str, kind: Kind, pack: &str, packs: &[String]) -> bool {
    kind != Kind::Preset
        && lib.status(id) == Status::NotReviewed
        && (packs.is_empty() || packs.iter().any(|p| p == pack))
}

/// Not reviewed items per pack, for the From picker, sorted by pack name.
pub fn pack_counts(lib: &Library) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for item in lib.items() {
        if reviewable(lib, &item.id, item.kind, &item.pack, &[]) {
            *counts.entry(item.pack.clone()).or_default() += 1;
        }
    }
    counts.into_iter().collect()
}

/// Items to review, in order, and where the VJ is.
#[derive(Debug, Clone, Default)]
pub struct Queue {
    ids: Vec<String>,
    pos: usize,
    pub session: Session,
}

impl Queue {
    /// The Not reviewed items from `packs` (all when empty), in `order`. `seed` shuffles.
    pub fn build(lib: &Library, packs: &[String], order: Order, seed: u64) -> Self {
        let mut items: Vec<_> = lib.items().iter().filter(|i| reviewable(lib, &i.id, i.kind, &i.pack, packs)).collect();
        match order {
            Order::Pack => items.sort_by_cached_key(|i| (i.pack.to_lowercase(), i.name.to_lowercase())),
            Order::Random => Rng::new(seed).shuffle(&mut items),
        }
        Self { ids: items.into_iter().map(|i| i.id.clone()).collect(), pos: 0, session: Session::default() }
    }

    /// The item on screen, or `None` when everything was reviewed.
    pub fn current(&self) -> Option<&str> {
        self.ids.get(self.pos).map(String::as_str)
    }

    /// Up to `n` items after the current one.
    pub fn up_next(&self, n: usize) -> &[String] {
        let start = (self.pos + 1).min(self.ids.len());
        &self.ids[start..(start + n).min(self.ids.len())]
    }

    /// Items left, the current one included.
    pub fn remaining(&self) -> usize {
        self.ids.len() - self.pos
    }

    /// Items in the queue when it was built.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Records `decision` for the current item in `lib` and moves on.
    pub fn decide(&mut self, lib: &mut Library, decision: Decision) {
        let Some(id) = self.current().map(str::to_owned) else { return };
        match decision {
            Decision::Keep => {
                lib.set_status(&id, Status::Kept);
                self.session.kept += 1;
            }
            Decision::Preset => {
                lib.set_status(&id, Status::Kept);
                self.session.presets += 1;
            }
            Decision::Skip => self.session.skipped += 1,
            Decision::Unwanted => {
                lib.set_status(&id, Status::Unwanted);
                self.session.unwanted += 1;
            }
        }
        self.pos += 1;
    }

    /// Jumps to item `i` of [`Queue::up_next`], skipping the ones before it (they stay Not
    /// reviewed and aren't counted).
    pub fn jump(&mut self, i: usize) {
        let target = self.pos + 1 + i;
        if target < self.ids.len() {
            self.pos = target;
        }
    }
}

/// The current item's parameters, normalised 0..1, with a lock per parameter that
/// Randomize leaves alone, and undo.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Knobs {
    values: Vec<f32>,
    defaults: Vec<f32>,
    locked: Vec<bool>,
    /// Parameters Randomize never touches (momentary triggers).
    fixed: Vec<bool>,
    undo: Vec<Vec<f32>>,
    /// The parameter last moved by [`Knobs::set`]: a drag is one undo step.
    dragging: Option<usize>,
}

impl Knobs {
    /// `values` as they are now, `defaults` for Reset; `fixed` ones are never randomized.
    pub fn new(values: Vec<f32>, defaults: Vec<f32>, fixed: Vec<bool>) -> Self {
        let n = values.len();
        assert!(defaults.len() == n && fixed.len() == n, "knob lists differ in length");
        Self { values, defaults, locked: vec![false; n], fixed, undo: Vec::new(), dragging: None }
    }

    pub fn values(&self) -> &[f32] {
        &self.values
    }

    pub fn is_locked(&self, i: usize) -> bool {
        self.locked.get(i).copied().unwrap_or(false)
    }

    pub fn toggle_lock(&mut self, i: usize) {
        if let Some(l) = self.locked.get_mut(i) {
            *l = !*l;
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    fn checkpoint(&mut self) {
        self.undo.push(self.values.clone());
        self.dragging = None;
    }

    /// Sets parameter `i`. Consecutive sets of the same parameter undo as one step.
    pub fn set(&mut self, i: usize, value: f32) {
        if i >= self.values.len() {
            return;
        }
        if self.dragging != Some(i) {
            self.checkpoint();
            self.dragging = Some(i);
        }
        self.values[i] = value.clamp(0.0, 1.0);
    }

    /// Every unlocked, randomizable parameter to a random value. Returns whether any moved.
    pub fn randomize(&mut self, rng: &mut Rng) -> bool {
        let free: Vec<usize> = (0..self.values.len()).filter(|&i| !self.locked[i] && !self.fixed[i]).collect();
        if free.is_empty() {
            return false;
        }
        self.checkpoint();
        for i in free {
            self.values[i] = rng.next_f32();
        }
        true
    }

    /// Every parameter back to its default.
    pub fn reset(&mut self) {
        if self.values != self.defaults {
            self.checkpoint();
            self.values.clone_from(&self.defaults);
        }
    }

    /// Back to the values before the last change. Returns whether there was one.
    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(v) => {
                self.values = v;
                self.dragging = None;
                true
            }
            None => false,
        }
    }
}

/// A small seeded random number generator (SplitMix64), so randomizing and shuffling need
/// no extra crate and tests are repeatable.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in 0..=1.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / ((1u64 << 24) - 1) as f32
    }

    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = (self.next_u64() % (i as u64 + 1)) as usize;
            v.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{Item, LibraryFile};

    fn item(id: &str, kind: Kind, pack: &str) -> Item {
        Item {
            id: id.into(),
            kind,
            name: id.into(),
            pack: pack.into(),
            description: String::new(),
            credit: String::new(),
            categories: Vec::new(),
            audio_reactive: false,
            bundled: true,
            path: None,
        }
    }

    fn lib() -> Library {
        let mut lib = Library::new(
            vec![
                item("a", Kind::Generator, "Ghost Arcade"),
                item("b", Kind::Generator, "Ghost Arcade"),
                item("c", Kind::MilkDrop, "MilkDrop"),
                item("d", Kind::Effect, "Ghost Arcade"),
                item("e", Kind::Preset, "Presets"),
                item("f", Kind::Generator, "Ghost Arcade"),
            ],
            LibraryFile::default(),
        );
        lib.set_status("f", Status::Kept);
        lib
    }

    #[test]
    fn queue_holds_not_reviewed_items_from_chosen_packs() {
        let lib = lib();
        let q = Queue::build(&lib, &[], Order::Pack, 1);
        assert_eq!(q.ids, ["a", "b", "d", "c"]);
        let q = Queue::build(&lib, &["MilkDrop".into()], Order::Pack, 1);
        assert_eq!(q.ids, ["c"]);
        assert_eq!(pack_counts(&lib), [("Ghost Arcade".into(), 3), ("MilkDrop".into(), 1)]);
    }

    #[test]
    fn random_order_is_a_repeatable_shuffle() {
        let lib = lib();
        let a = Queue::build(&lib, &[], Order::Random, 7);
        let b = Queue::build(&lib, &[], Order::Random, 7);
        assert_eq!(a.ids, b.ids);
        let mut sorted = a.ids.clone();
        sorted.sort();
        assert_eq!(sorted, ["a", "b", "c", "d"]);
    }

    #[test]
    fn decisions_set_status_count_and_advance() {
        let mut lib = lib();
        let mut q = Queue::build(&lib, &[], Order::Pack, 1);
        assert_eq!(q.up_next(2), ["b", "d"]);
        q.decide(&mut lib, Decision::Keep);
        q.decide(&mut lib, Decision::Preset);
        q.decide(&mut lib, Decision::Skip);
        q.decide(&mut lib, Decision::Unwanted);
        assert_eq!(lib.status("a"), Status::Kept);
        assert_eq!(lib.status("b"), Status::Kept);
        assert_eq!(lib.status("d"), Status::NotReviewed);
        assert_eq!(lib.status("c"), Status::Unwanted);
        assert_eq!(q.session, Session { kept: 1, presets: 1, skipped: 1, unwanted: 1 });
        assert_eq!(q.current(), None);
        assert_eq!(q.remaining(), 0);
        q.decide(&mut lib, Decision::Keep); // nothing left: no-op
        assert_eq!(q.session.reviewed(), 4);
    }

    #[test]
    fn jump_skips_ahead_without_counting() {
        let mut lib = lib();
        let mut q = Queue::build(&lib, &[], Order::Pack, 1);
        q.jump(1);
        assert_eq!(q.current(), Some("d"));
        assert_eq!(q.session.reviewed(), 0);
        q.decide(&mut lib, Decision::Keep);
        assert_eq!(q.current(), Some("c"));
    }

    #[test]
    fn randomize_leaves_locked_and_fixed_alone() {
        let mut k = Knobs::new(vec![0.5; 4], vec![0.5; 4], vec![false, false, false, true]);
        k.toggle_lock(1);
        let mut rng = Rng::new(3);
        assert!(k.randomize(&mut rng));
        assert_eq!(k.values()[1], 0.5);
        assert_eq!(k.values()[3], 0.5);
        assert_ne!(k.values()[0], 0.5);
        assert!(k.values().iter().all(|v| (0.0..=1.0).contains(v)));
        k.toggle_lock(0);
        k.toggle_lock(2);
        let before = k.values().to_vec();
        assert!(!k.randomize(&mut rng));
        assert_eq!(k.values(), before);
    }

    #[test]
    fn undo_steps_back_through_randomize_drag_and_reset() {
        let mut k = Knobs::new(vec![0.2, 0.4], vec![0.2, 0.4], vec![false, false]);
        let mut rng = Rng::new(9);
        k.randomize(&mut rng);
        let random = k.values().to_vec();
        k.set(0, 0.1);
        k.set(0, 0.15); // same drag
        k.set(0, 2.0); // clamped
        assert_eq!(k.values()[0], 1.0);
        k.reset();
        assert_eq!(k.values(), [0.2, 0.4]);
        assert!(k.undo());
        assert_eq!(k.values()[0], 1.0);
        assert!(k.undo());
        assert_eq!(k.values(), random);
        assert!(k.undo());
        assert_eq!(k.values(), [0.2, 0.4]);
        assert!(!k.undo());
    }

    #[test]
    fn rng_floats_cover_the_unit_range() {
        let mut rng = Rng::new(1);
        let v: Vec<f32> = (0..1000).map(|_| rng.next_f32()).collect();
        assert!(v.iter().all(|x| (0.0..=1.0).contains(x)));
        assert!(v.iter().any(|&x| x < 0.1) && v.iter().any(|&x| x > 0.9));
    }
}
