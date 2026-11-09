# Proposal

## Why

The maintainer's Resolume sets lean on clips: 52 clips carry their own effects and 67 have a moved clip Transform (mostly Scale, Position Y and Rotation Z). openVJ has no clip effects and no Transform outside the effect chain, so the import drops all of it and those clips play full-frame and unprocessed. Effect settings are also lost, because the import only matches effect types, not their parameters.

## What Changes

- **Clip effect chains.** Each clip gets its own chain, run before the clip reaches its layer (so before the layer transition crossfade). Thumbnails stay raw; the preview shows the chain.
- **Fixed Transform on clips and layers**, like Resolume's: Position X/Y (px), Scale and Scale W/H (%), Rotation X/Y/Z (degrees, X/Y with perspective) and Anchor X/Y. It always sits at the top of the clip or layer card, runs before that chain's effects, and costs nothing when left at its defaults. Its parameters are mappable like any other.
- **.avc import** brings over clip and layer Transforms and clip effect chains, and now imports **effect settings**: each ISF input can name the Resolume parameter it matches (`"RESOLUME": "Strength"`, with an optional scale), so Bypass, mix and the non-default values Resolume saved come across.
- Existing effects that have a Resolume counterpart (Mirror, Kaleido, Edges, Pixelate, Blur, Color, RGB Split, Wave, Posterize) take Resolume's parameter names, ranges and defaults where they differ, so settings import 1:1. Saved compositions keep working through name aliases.
- A Transform *effect* in a Resolume chain imports into the fixed Transform. openVJ's Transform effect stays for chains where it's placed on purpose.

Out of scope: animated parameters from .avc (add-parameter-animation), new effects (Bloom, Trails, Fish Eye, Hue Rotate, …: next FX step), composition and group Transforms.

## Capabilities

### New Capabilities

### Modified Capabilities
- `effects`: clip effect chains, the fixed clip and layer Transform.
- `resolume-import`: clip and layer Transforms, clip effects and effect settings are imported.

## Impact

Merges after output mapping (#21) and ISF generators (#24), per the v0.1 release plan.

## Touches

- crates/openvj-core: `composition.rs` (Clip gets `effects` and `transform`, Layer gets `transform`), `effect.rs`, `isf.rs` (per-input `RESOLUME`), `effects/*.fs`
- crates/openvj-render: `lib.rs` (clip chain + transform pass per playing clip)
- crates/openvj-resolume: `lib.rs`, `tests.rs`
- crates/openvj-app: `app.rs`, `ui/properties.slint` (clip card, Transform section)
- README.md
- Specs: effects, resolume-import
- Shared hot spots: composition.rs, app.rs, render lib.rs, resolume lib.rs, README.md

## Conflicts

- **Output mapping (#21)** changes composition.rs, the render path and app.rs. Land after it.
- **ISF generators (#24)** adds a generator clip kind to the Clip model and changes render isf.rs and the effects spec. Land after it; generator clips get a chain and Transform like any other clip.
- **User effects folder (#19)** changes effect.rs, render lib.rs and archives into specs/effects. Small, lands first.
- **Perf (#18)** changes render lib.rs and app.rs; adding a pass per clip must keep its savings (an idle Transform and an empty chain add no passes).
- **add-parameter-animation** will import animated values; this change imports only the static ones it will build on.

## Open questions

- Resolume can also rotate in X/Y (3D). The maintainer's sets use it on 7 clips, so it's in; it renders as a flat card tilted in perspective, without depth sorting between layers.
