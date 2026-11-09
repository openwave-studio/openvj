# Tasks

## 1. Proposal

- [ ] 1.1 The maintainer approves this proposal

## 2. Clip chains and Transform (one PR, after #21 and #24)

- [ ] 2.1 Clip `effects` and `transform`, Layer `transform` in composition.rs (serde defaults, so old files load)
- [ ] 2.2 Render: Transform pass (skipped at defaults) and clip chain per playing clip, before the layer transition; persistent fx state per clip effect
- [ ] 2.3 App: clip card with Transform section and effect chain; drop effects on clips; MIDI and mappings reach clip and Transform params
- [ ] 2.4 Tests: clip chain runs per clip, identity Transform adds no pass, old compositions load

## 3. .avc effect settings (one PR)

- [ ] 3.1 Per-input `RESOLUME` name and scale in ISF headers (isf.rs, README "Writing effects")
- [ ] 3.2 Reshape built-in effects to Resolume's parameter sets, with aliases for old names
- [ ] 3.3 Import clip and layer Transforms, clip chains, Bypass, mix and parameter values; report params that don't map
- [ ] 3.4 Tests with synthetic .avc snippets (no personal files)

## 4. Finish

- [ ] 4.1 README "What works" and import notes
- [ ] 4.2 The maintainer checks an imported set against Resolume
- [ ] 4.3 Archive this change
