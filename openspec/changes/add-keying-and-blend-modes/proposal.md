# Proposal

## Why

openVJ has 9 blend modes and 12 effects. Resolume Arena 7 has about 50 blend modes and about 100 effects, and a VJ coming from Resolume reaches for a handful of them in every set: keying a logo or a greenscreen clip, masking one layer with another, trails and strobes on the drop. Without them, many .avc compositions import with layers falling back to Normal and effects dropped. This was #4 in the top 5 of the Resolume 7 gap analysis (2026-11-26).

## What Changes

- **Blend modes**, with Resolume's names, on layers and groups: **Soft Light**, **Hard Light**, **Dodge**, **Burn**, **Exclusion**, and the keying and masking modes **Luma Key** (dark parts of the layer turn transparent), **Mask** (the layer's brightness decides how much of what's below shows; black hides it), **Mask Inverted**, **Stencil** (what's below shows only where the layer is opaque) and **Silhouette** (what's below is cut out where the layer is opaque). Mask, Stencil and Silhouette work on what is below the layer inside its group, or the composition so far, as in Resolume. The blend menu groups them as Mix, Light and Key.
- **Effects**, as ISF files, named and imported like Resolume's:
  - Keying: **Luma Key** (threshold, softness, invert) and **Chroma Key** (key colour, tolerance, softness, spill).
  - Time: **Trails** (each frame fades over the last; feedback amount, fade), **Strobe** (flashes to a colour or black, on a rate in beats or Hz).
  - Look: **Threshold** (two-colour), **Tile** (repeat in a grid, with mirror), **Slide** (scroll with wrap), **Bloom** (glow on bright parts), **Vignette**, **Infinite Zoom**.
- **.avc import** maps these blend modes and effects with their settings, so they stop being listed as dropped.

Out of scope: Resolume's transition-style blends (wipes, push, cube, zoom), the depth and background-removal effects, FFGL plugins, and clip-level blend modes (Resolume 7.25 per-clip transitions).

## Capabilities

### Modified Capabilities
- `layer-mixing`: more blend modes, including keying and masking against what's below.
- `effects`: ten new built-in effects.
- `resolume-import`: imports them.

## Touches

- crates/openvj-core: `composition.rs` (`BlendMode` variants, names and menu groups), `effects/*.fs` (ten new files)
- crates/openvj-render: `shaders.wgsl` (`fs_blend` modes), `lib.rs` (only if Mask/Stencil need the base's alpha passed differently)
- crates/openvj-resolume: `lib.rs` (`blend_mode` names)
- crates/openvj-app: `ui/properties.slint` (grouped blend menu)
- Specs: layer-mixing, effects, resolume-import
- Shared hot spots: `composition.rs`, `crates/openvj-render/src/lib.rs`, `crates/openvj-resolume/src/lib.rs`, README.md

## Conflicts

- add-opendj-sync and add-gated-envelopes (PR #35, proposals only) touch `composition.rs` for mappings only. No overlap in blend or effect code; either order works.
- add-osc-control and add-mixer-undo (this PR) don't touch blend or effects code. Suggested build order: this, then OSC, then undo.

## Open questions

- Resolume stores blend modes by name in .avc (`type="Alpha"`, `"Add"`…). Seba's sample files only use Alpha and Add, so the exact names of Luma Key, Mask, Stencil and Silhouette need one test .avc (a layer set to each). Can Seba save one?
- Should Mask and Stencil be able to reach below a group (the whole composition), or stay inside the group as proposed?
- Are these the right ten effects, or is there one Seba uses in Resolume more than any of them?
