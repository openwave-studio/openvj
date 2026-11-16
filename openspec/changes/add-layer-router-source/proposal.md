# Proposal

## Why

Feedback is where video synths like Lumen get much of their look, and Imaginando VS can use its own output as a layer. In openVJ, feedback is only possible inside one effect. Resolume does this with the **Layer Router** source: a clip that shows another layer, a group or the whole composition, so it can be re-effected, or fed back into itself through the previous frame. Adding the same source gives openVJ feedback and re-use with Resolume's names, and lets .avc files that use Layer Routers import. It came out of the Lumen and VS comparison (2026-11-12).

## What Changes

- A **Layer Router** source in the Sources tab, as in Resolume. Its **Input** chooses a layer, a group or **Composition**.
- A router on a layer **above** its input shows that input's current frame. A router that reads its own layer, a layer above it, or the composition shows the **previous frame**. That's the feedback path: a router with Transform and a dry/wet below 100% makes trails and tunnels, like Lumen's patched framebuffer.
- Routed frames come straight from the render's existing intermediate textures. Nothing is copied to the CPU. A router whose input needs the previous frame keeps one extra texture of the composition size. When no router is in use, nothing extra is kept.
- Imports from .avc: a Layer Router clip (and Group Router) maps to this source with its input.

Out of scope: routing between decks, routing an Advanced Output slice, and Resolume's Input Rotation.

## Capabilities

### Modified Capabilities
- `media-sources`: the Layer Router source.
- `resolume-import`: imports Layer and Group Routers.

## Touches

- crates/openvj-core: `composition.rs` (a `ClipSource::Router { input }` variant)
- crates/openvj-render: `src/lib.rs` (keep the requested layer outputs, and the previous composition frame when needed; bind them as the router clip's texture)
- crates/openvj-resolume: `lib.rs` (Layer/Group Router import)
- crates/openvj-app: `ui/library.slint` or the Sources entry, `ui/properties.slint` (Input picker), `src/app.rs`
- Specs: media-sources, resolume-import
- Shared hot spots: `composition.rs`, `crates/openvj-render/src/lib.rs`, `crates/openvj-resolume/src/lib.rs`, `src/app.rs`

## Conflicts

- PR #18 (Synthesis layout) touches the render loop and `src/app.rs`. Merge #18 first.
- add-clip-autopilot-and-piano edits `composition.rs` and the resolume importer. These are small, separate areas. Either order works.

## Open questions

- Should the Sources tab list one Layer Router entry with an Input picker (Resolume's way), or one entry per layer and "Composition"? The default here is Resolume's way.
