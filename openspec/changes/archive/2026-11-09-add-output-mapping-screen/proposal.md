# Proposal

## Why

openVJ already imports Resolume Advanced Output presets (screens, slices, masks, warps) into a model, but nothing draws it and nothing edits it. A VJ can't pin the output onto a wall, a screen or a sculpture, which is the first thing a projection gig needs. Resolume users expect the Advanced Output window: an Input Selection stage and an Output Transformation stage with slices, masks and warps.

## What Changes

- **Mapped output.** The engine draws each screen from the output model: every slice cuts its input quad out of the composition and draws it through its warp (corner pin, linear grid or bezier grid), and masks black out what they should. With no setup, a passthrough screen keeps today's behaviour. The output window shows Screen 1.
- **Output mapping window**, Resolume-style, in openVJ's look ([mockup](https://claude.ai/artifact/AJ7pSVUMPj7n7XQBBNsYWM)): screens, slices and masks listed on the left, the screen in the middle, the selected slice's settings on the right, and two stages:
  - *Output transformation*: drag a slice's four corners to pin it (perspective-correct), nudge with the arrow keys, type exact pixel values, reset.
  - *Input selection*: drag and resize the rectangle each slice takes from the composition.
- **Slices and masks.** Add, rename, reorder, enable and delete slices; add a mask, edit its points, invert it.
- **Saved per machine**, like Resolume: the setup lives in openVJ's settings folder, not in the composition, so a show file opens with the venue's mapping. Resolume Advanced Output presets import from the mapping window.
- **Test card** on the output while mapping, so slices can be lined up without content.

Built in steps, each its own PR: (1) render the model, (2) window with corner pin and input selection, (3) slice and mask editing. Grid and bezier warp editing, soft edges and edge blending follow later; imported grid and bezier warps already render in step 1.

Out of scope: several output windows at once and Syphon output screens (see Conflicts), slices fed by one layer instead of the whole composition, polygon slice editing, exporting back to Resolume XML, camera-based auto-calibration.

## Capabilities

### New Capabilities

### Modified Capabilities
- `output-mapping`: the model is drawn and edited, not only imported.

## Impact

New render pass after the master stage; a new window in the app; the output window shows the mapped screen instead of the raw composition.

## Touches

- `crates/openvj-render/src/lib.rs` (hot spot): a mapping pass that draws slice meshes into a screen target; new `mapping.wgsl`.
- `crates/openvj-core/src/output.rs`: helpers for editing (move a corner, add a slice or mask); serde already in place.
- `crates/openvj-app/src/app.rs` (hot spot) and `main.rs`: own the setup, load and save it, wire the window.
- New `crates/openvj-app/ui/mapping.slint`; `main.slint` (hot spot) only for a "Mapping" button next to "Output window"; `theme.slint` only if a token is missing.
- `crates/openvj-resolume`: none; the importer is reused as is.
- Specs: `output-mapping`.

## Conflicts

- **Syphon output** (thread started the same day): it publishes the output as a Syphon server and will touch the engine's output target and `app.rs`. Suggested order: mapping step 1 first, so Syphon publishes a mapped screen (a Resolume screen whose device is Syphon), or Syphon first if it is ready sooner and step 1 rebases onto it. Whichever is second keeps the other's output path.
- **PR #18** (perf): touches `openvj-render/src/lib.rs` and `app.rs`. Merge #18 first; step 1 rebases.
- `add-isf-effect-engine`, `add-parameter-animation`: no overlap expected beyond `app.rs`.

## Open questions

1. Window: a separate Mapping window like Resolume's Advanced Output (recommended, it can sit on the laptop screen while the projector shows the output), or a full-window mode inside the main window?
2. Should the slice outlines and handles also show on the projector while editing (Resolume shows them only in the mapping window, plus an optional test card)?
3. Do you need a slice to take a single layer instead of the composition (Resolume's per-slice input source) before v0.1?
