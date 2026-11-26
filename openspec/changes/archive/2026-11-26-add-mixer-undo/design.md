# Design

## Snapshots, not commands

`Composition` is already `Clone` and `PartialEq`, and every edit goes through `App` in `app.rs`. The simplest history that can't miss a field is a stack of whole-composition snapshots taken before each edit, labelled for the UI. A command log would need an inverse for every edit and every future edit, which is where undo bugs come from.

Cost: a composition with 4 decks of 10×8 clips and effect chains clones in well under a millisecond and holds tens of KB of plain data (media is referenced by path, textures live in the renderer). 100 steps stays in the low MB. If a large set proves otherwise, clips can move behind `Arc` for structural sharing without changing the model.

## Coalescing

A step is recorded when an edit starts. Consecutive changes to the same target within one gesture (a fader or knob drag, typing in a field) merge into the step, the way Discover's `Knobs` already does. Values arriving from MIDI, OSC, audio and animations never record.

## Keeping the show running

Runtime state lives in serde-skipped fields of `Layer` (`active`, fading clips) and in the renderer. Restoring a snapshot copies the saved model fields onto the live composition and keeps runtime fields, matched by layer and clip id. A playing clip that no longer exists in the restored model is cleared with the layer's transition, never cut.

## As built

`with_app` in `main.rs`, which every UI callback goes through, calls `History::begin` before the callback and `History::end` after it. Both take the composition as a `serde_json::Value`, so the serde-skipped runtime fields are left out by construction. When they differ, the step keeps only the changed values, by JSON path, before and after (an array that changed length counts as one value). Undo writes the before values onto the composition as it is now and redo the after values, then goes through `restore_model`. So a fader moved from MIDI or OSC since the step, or the tempo following Link, stays where it is; whole snapshots would have put those back too.

Launches and clears don't touch the saved part, and `bpm` is left out of the diff, so they are never steps. MIDI, OSC, audio and animation act in the frame loop, not in callbacks, so they never record; MIDI and OSC learn bracket their own change so a learned mapping is a step. Coalescing compares the changed paths: the same paths within 800 ms merge into the step before. Deck switches change `current_deck`, so they are steps too.
