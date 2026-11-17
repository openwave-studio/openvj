# Proposal

## Why

Placing something from the library took two clicks: click a tile (which also armed it), then click a slot. Seba found it awkward ("that is weird now with double clicking things", 2026-11-17). It was done that way because Slint had no drag and drop when the library browser was built (PR #20). Slint 1.18, which openVJ already uses, has `DragArea` and `DropArea`, so the library can work like Resolume's browser: drag a thing where it should go.

## What Changes

- **Drag sources** from the Sources grid or list, the expanded library, the inputs row (cameras, Syphon, Layer Router) and the Files tab **onto a clip slot** to place them there (replacing what's there), or **onto a layer** for its first empty slot.
- **Drag effects** from the Effects tab or the expanded library **onto a clip** (clip effect), **onto a layer** (layer effect), or **onto an effect chain** in the properties panel (composition, group, layer or clip effects).
- While dragging, the target under the cursor lights up in the accent with what will happen ("Place: Bars", "Add to layer: Blur"), and the status bar says where it can go.
- Dragging out of the **expanded library** hides it for the drag so the clip grid underneath takes the drop; it comes back after.
- Click-then-click-a-slot is **removed**: clicking a tile only previews it, and clicking a slot only selects or launches. Double-click and Enter still put the selected item in the selected slot.

Out of scope: dragging clips between slots, dropping files from the Finder (Slint's winit backend doesn't pass them on yet), and dropping onto the Player playlist or Discover (neither has slots).

## Capabilities

### Modified Capabilities
- `library`: drag and drop instead of click-then-slot.
- `clip-grid`: placing from the browser by dragging.

## Touches

- crates/openvj-app: new `ui/drag.slint`; `ui/library.slint` (tiles and rows drag), `ui/main.slint` (slot and layer drop targets, inputs and files drag, expanded library over the grid, status bar), `ui/properties.slint` (effect chain drop target), `ui/state.slint` (`BrowserItem.armed` → `picked`, `LibTile.armed` removed), `src/app.rs` (drop handlers, arming removed), `src/main.rs` (wiring)
- README.md (Library paragraph)
- Specs: library, clip-grid
- Shared hot spots: `src/app.rs`, `ui/main.slint`, README.md

## Conflicts

None known: no other PR is open (2026-11-17). Changes that later touch `ClipCell`, `LayerStrip` or `EffectChain` should keep their `DropArea`.

## Open questions

- Should dropping onto a slot that is playing replace it (as now, like Resolume), or be refused so the output can't change by accident?
