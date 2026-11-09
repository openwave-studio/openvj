# Proposal

## Why

The maintainer's sets lean on effects openVJ lacks (Acuarela, Bloom, Trails, Bendoscope, Cube Tiles), and hard-coding each one in WGSL doesn't scale. Effects as ISF files make new ones quick to add, let us borrow Varda's MIT shader set, and give parameters real units that map cleanly to Resolume's.

## What Changes

- Effects become ISF `.fs` files (JSON header + GLSL), compiled at load time. The 11 existing effects are ported.
- Effects can have several passes and persistent (feedback) buffers.
- Mixing and effects run in 16-bit float; output, preview and thumbnails stay 8-bit.
- Effect parameters have real units (degrees, %, px, named choices) and are saved by name.
- New effect: Acuarela, a watercolour-smoke feedback effect rebuilt from Resolume's Wire patch.

Out of scope here (next FX steps, each its own change): clip effect chains and the fixed clip/layer Transform, importing effect settings from `.avc`, more effects, a user effects folder.

## Capabilities

### New Capabilities

### Modified Capabilities
- `effects`: effects are ISF files with real-unit parameters, multi-pass and feedback, on a 16-bit pipeline; Acuarela added.

## Impact

Implemented in PR #9 (merged). Archived with the add-clip-effects-and-transform proposal.

## Touches

- crates/openvj-core: new `isf.rs`, `effect.rs` rewritten, `effects/*.fs`, small edits to `composition.rs`, `midi.rs`, `modulation.rs`
- crates/openvj-render: new `isf.rs`, `lib.rs`, `shaders.wgsl`
- crates/openvj-resolume: `lib.rs`, `midi.rs` (effect ids)
- crates/openvj-app/src/app.rs, README.md
- Specs: effects
- Shared hot spots: composition.rs, app.rs, render lib.rs, resolume lib.rs, README.md

## Conflicts

- restyle-ui-like-opendj (PR #10) touches the UI only; no file overlap expected, but effect cards in properties.slint may need restyling after both land.
- add-parameter-animation will change how effect parameters are driven; it should build on this change's typed parameters. Merge this first.

## Open questions

- Acuarela's Wire mixer mode 11 is unknown, so the effect has a Blend choice (Lighten by default). The maintainer to compare against Resolume.
