# Tasks

## 1. Ghost Arcade names (small PR, can land first)

- [x] 1.1 Preamble shim in `openvj-render/src/isf.rs` for Ghost Arcade audio names, `sampleFFT`/`sampleWaveform`, `renderSize`, redeclared uniforms, `precision`
- [x] 1.2 Tests: a Ghost Arcade-style filter compiles

## 2. ISF generator clips

- [x] 2.1 Library keeps generators (no `inputImage`) next to effects; folder scan lists them
- [x] 2.2 New clip source for ISF generators with named parameters and modulators; old compositions load unchanged
- [x] 2.3 Render ISF generator clips (TIME follows clip speed/beat sync, audio bands, feedback buffers per clip)
- [x] 2.4 Sources tab lists ISF generators by category; Clip panel shows their parameters (layout to revisit after the maintainer's answer)
- [x] 2.5 Tests: save/load round trip, missing generator, render moves over time

## 3. Bundled pack

- [x] 3.1 `scripts/import-ghost-arcade.sh`: fetch a pinned commit, drop excluded files, test-compile, write NOTICE
- [x] 3.2 `shaders/ghost-arcade/` with LICENSE (AGPL-3.0) and NOTICE; load from the app bundle and from the repo in dev
- [x] 3.3 Copy the folder into OpenVJ.app in `packaging/`
- [x] 3.4 Test: every bundled file compiles
- [ ] 3.5 README "What works" and credits; archive this change
