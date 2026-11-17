# Proposal

## Why

PR #31 added drag and drop from the browser and library, but left out two things Seba asked for right after (2026-11-17): moving clips between slots, and dropping media files from the Finder.

## What Changes

- **Drag a clip** onto another slot to move it there. If that slot has a clip, the two swap, so nothing is lost. Hold **Ctrl** while dragging to copy the clip instead, with its parameters and effects. Dropping a clip on a layer's strip moves it to that layer's first empty slot.
- **Drop files from the Finder** onto a slot: the first file goes there, the next ones into the columns to its right. Dropped on a layer, they start at its first empty slot. Files openVJ can't play are skipped. While files hover over the window, the slot or layer under the cursor lights up as for an in-app drag.
- Dropping on a playing slot replaces or swaps it live, as in Resolume (Seba, 2026-11-17).

Out of scope: dragging whole columns or layers, Alt as the copy modifier (Slint fixes Ctrl for copy), dropping folders.

## Capabilities

### Modified Capabilities
- `clip-grid`: moving and copying clips by dragging, and files from the Finder.

## Touches

- crates/openvj-app: `ui/main.slint` (slots drag and take clips), `ui/drag.slint`, `src/app.rs` (move, copy, files), new `src/finder_drop.rs` (feeds the system file drag to Slint at the cursor), `src/main.rs`, `Cargo.toml` (i-slint-core pinned to slint's version; objc2-app-kit on macOS for the cursor)
- README.md
- Specs: clip-grid
- Shared hot spots: `src/app.rs`, `ui/main.slint`, README.md

## Conflicts

None known: no other PR is open (2026-11-17).

## Open questions

None.
