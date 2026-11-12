# Proposal

## Why

openVJ imports Resolume Advanced Output better than any free tool, but it can only drive one projector: only Screen 1 is shown and edited, grid and bezier warps can't be edited, soft edges aren't drawn and colour can't be adjusted. The maintainer approved finishing these four after the [mapping tools comparison](https://claude.ai/artifact/QzN1jQPQeoTf8joJ2LfAgQ) (2026-11-12). Resolume files already carry all of them, so this is mostly rendering and UI on the existing model.

## What Changes

- **One output window per screen.** Each enabled screen opens its own output window and can be sent fullscreen to a display picked in the mapping window, like Resolume's per-screen Output Device. Imported display screens go to the display with the same name when it is connected. The mapping window gets a screen list to add, rename, remove and pick screens.
- **Edit grid and bezier warps.** In Output transformation the VJ sets a slice's grid size and switches points between linear and bezier, then drags points and bezier handles, as with Resolume's warp points. Edits save back in the same form Resolume files use.
- **Draw soft edges.** Slices with Soft Edge on fade where their input areas overlap, as in Resolume Arena, using the setup's Power, Luminance, Gamma and per-channel gamma. Luminance and the gammas start importing; black level compensation stays out.
- **Outlines on the projector while editing.** While the mapping window is open, a button shows slice outlines and the selected point on the output, as Resolume does.
- **Edit colour.** Screen and slice Brightness, Contrast and Red/Green/Blue (already imported and drawn) become editable in the mapping window, plus screen Opacity.

Out of scope: slice input from a single layer, mask feather, the Output Guide image, black level compensation, DMX screens, Syphon/NDI output per screen, camera calibration.

## Capabilities

### New Capabilities

### Modified Capabilities
- `output-mapping`: several screens shown and edited, warp point editing, soft edge, colour controls.
- `monitors-and-output`: one output window per screen instead of a single output window.

## Impact

Builds on the merged mapping screen (renderer per screen target already exists). No change to the composition format; the per-machine `output-mapping.json` gains display choice and soft-edge settings, with old files still loading.

## Touches

- crates/openvj-core: `output.rs` (display choice per screen, soft-edge gamma/luminance, warp grid resize helpers)
- crates/openvj-resolume: `advanced_output.rs` (import Soft Edge Gamma/Luminance), tests
- crates/openvj-render: `mapping.rs`, `mapping.wgsl` (soft-edge ramp, render every screen); `lib.rs` only if a hook is needed (shared hot spot)
- crates/openvj-app: `mapping.rs`, `mapping_ui.rs`, `main.rs` (output windows), `app.rs` (frame loop, shared hot spot), `ui/mapping.slint`, `ui/main.slint` (OutputWindow, shared hot spot)
- Specs: output-mapping, monitors-and-output
- README.md "What works" (shared hot spot)
- Code to borrow (MIT, with attribution): Varda `src/internal/renderer/shaders/edge_blend.wgsl`, `src/internal/surface/warp.rs`, `src/internal/output/`

## Conflicts

- PR #7 (audio reactivity) touches `app.rs`, `openvj-render/src/lib.rs` and README.md. Merge #7 first; this change rebases on it.
- add-parameter-animation (not started) touches `app.rs`; whichever lands second rebases. No spec overlap.

## Decisions (maintainer, 2026-11-12)

- The maintainer runs one projector today and may test two later, so build order is: warp editing, projector outlines, colour, output window per screen, then soft edge.
- A screen whose display isn't connected opens as a normal window the VJ can drag to a projector.
- Slice outlines and the selected point show on the projector while editing, only while the mapping window is open, toggled by a button.

## Open questions

None.
