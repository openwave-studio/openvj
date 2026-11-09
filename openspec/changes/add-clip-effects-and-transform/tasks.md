# Tasks

## 1. Proposal

- [x] 1.1 Proposal approved (PR #25 merged)

## 2. Clip chains and Transform

- [x] 2.1 Clip `effects` and `transform`, Layer `transform` in composition.rs (serde defaults, so old files load)
- [x] 2.2 Render: Transform pass (skipped at defaults) and clip chain per playing clip, before the layer transition; the preview shows the chain
- [x] 2.3 App: clip card with Transform and effect chain, layer Transform card, **+ Clip** in the Effects tab; parameter mapping reaches clip and Transform parameters
- [x] 2.4 Tests: clip chain runs per clip, identity Transform adds no pass, position, scale and tilt
- [ ] 2.5 MIDI shortcuts for clip effects and Transforms (Resolume presets address them by clip; later)

## 3. .avc effect settings

- [x] 3.1 Per-input `RESOLUME` name and range in ISF headers (isf.rs, README "Writing effects")
- [x] 3.2 Map built-in effects onto Resolume's parameters where they match (Transform, Acuarela, Edge Detection with Use Color and Color, Hue Rotate, Kaleidoscope), keeping their saved names
- [x] 3.3 Import clip and layer Transforms, clip chains, Bypass, mix and parameter values; report params that don't map
- [x] 3.4 Tests with synthetic .avc snippets (no personal files)

## 4. Finish

- [x] 4.1 README "What works" and import notes
- [ ] 4.2 Check an imported set against Resolume (Transform rotation direction, Edge Detection Multiplier range)
- [ ] 4.3 Archive this change
