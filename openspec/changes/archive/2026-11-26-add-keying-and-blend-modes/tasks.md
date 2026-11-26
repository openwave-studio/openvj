## 1. Blend modes
- [x] 1.1 `BlendMode` variants, display names, menu groups (Mix, Light, Key); saved by name; tests for old files
- [x] 1.2 `fs_blend`: Soft Light, Hard Light, Dodge, Burn, Exclusion; Luma Key; Mask, Mask Inverted, Stencil, Silhouette against the base (group or composition so far)
- [x] 1.3 Grouped blend menu on layers and groups; MIDI blend-mode shortcuts cover the new modes
- [x] 1.4 .avc import names (matched by menu name; Seba skipped the test file); importer tests

## 2. Keying effects
- [x] 2.1 Luma Key and Chroma Key ISF files with Resolume names and parameter mapping
- [x] 2.2 Check alpha survives the effect chain into the layer blend; test with a Luma Key over a generator

## 3. Time and look effects
- [x] 3.1 Trails (persistent buffer) and Strobe (beats or Hz)
- [x] 3.2 Threshold, Tile, Slide, Bloom, Vignette, Infinite Zoom
- [x] 3.3 Resolume names and parameter imports for each; thumbnails render

## 4. Finish
- [x] 4.1 README "What works" (layers, effects, import); archive the change
