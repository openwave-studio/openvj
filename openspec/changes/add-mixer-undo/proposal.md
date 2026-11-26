# Proposal

## Why

In the Mixer, nothing can be undone: a clip dropped on the wrong slot, a deleted effect chain or a removed deck is gone, mid-show included. Resolume undoes almost every edit with Cmd+Z. openVJ already has undo inside Discover and Synthesis, but not where the composition is built. This was #1 in the top 5 of the Resolume 7 gap analysis (2026-11-26).

## What Changes

- **Cmd+Z** undoes and **Cmd+Shift+Z** redoes (Ctrl on Linux and Windows), with **Undo** and **Redo** buttons in the top bar that show what they'll undo on hover ("Undo: move clip"). The status bar says what was undone.
- **Edits are undoable**: placing, moving, copying and removing clips; clip settings and source parameters; adding, removing, reordering and bypassing effects and their parameters; layer, group, column and deck changes (add, remove, rename, reorder, blend mode, group and crossfader side); parameter mappings; MIDI (and later OSC) learned mappings; Transform; composition settings; importing a MIDI preset.
- **Performing is not undone**: launching clips and columns, clearing layers, tempo, tap and transport, and any value moved by MIDI, OSC, audio or an animation. A mouse drag on a fader is one undo step, as in Resolume, so a slip on the wrong opacity can be taken back.
- **Undo never interrupts the output**: clips that are playing keep playing, unless the undo removes them, and then they clear with the layer's transition.
- History holds the last 100 steps, is cleared by New and Open, and isn't saved to the file.

Out of scope: undo for the mapping window (Advanced Output), library changes (tags, crates, unwanted), the Player's playlists, and the Synthesis and Discover undo that already exists.

## Capabilities

### New Capabilities
- `undo`: undo and redo of composition edits in the Mixer.

## Touches

- crates/openvj-core: new `history.rs` (snapshots, coalescing, labels); `composition.rs` (carry runtime state over a restore)
- crates/openvj-app: `src/app.rs` (record steps at each edit entry point, restore), `ui/main.slint` (buttons, shortcuts)
- Specs: new undo
- Shared hot spots: `composition.rs`, `src/app.rs`, `ui/main.slint`, README.md

## Conflicts

- Every open change that adds an edit to the Mixer needs to record it. Land this last of the three in this PR, after add-keying-and-blend-modes and add-osc-control, and after add-opendj-sync (PR #35), so their edits are covered from the start.
- add-midi-learn (open): learned mappings become undoable; no code overlap beyond the record call.

## Decisions

- Mouse drags on faders and knobs are undoable, one step per drag, as in Resolume (the maintainer, 2026-11-26).

## Open questions

- Is 100 steps enough?
