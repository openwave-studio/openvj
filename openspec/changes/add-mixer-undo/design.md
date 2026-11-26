# Design

## Snapshots, not commands

`Composition` is already `Clone` and `PartialEq`, and every edit goes through `App` in `app.rs`. The simplest history that can't miss a field is a stack of whole-composition snapshots taken before each edit, labelled for the UI. A command log would need an inverse for every edit and every future edit, which is where undo bugs come from.

Cost: a composition with 4 decks of 10×8 clips and effect chains clones in well under a millisecond and holds tens of KB of plain data (media is referenced by path, textures live in the renderer). 100 steps stays in the low MB. If a large set proves otherwise, clips can move behind `Arc` for structural sharing without changing the model.

## Coalescing

A step is recorded when an edit starts. Consecutive changes to the same target within one gesture (a fader or knob drag, typing in a field) merge into the step, the way Discover's `Knobs` already does. Values arriving from MIDI, OSC, audio and animations never record.

## Keeping the show running

Runtime state lives in serde-skipped fields of `Layer` (`active`, fading clips) and in the renderer. Restoring a snapshot copies the saved model fields onto the live composition and keeps runtime fields, matched by layer and clip id. A playing clip that no longer exists in the restored model is cleared with the layer's transition, never cut.
