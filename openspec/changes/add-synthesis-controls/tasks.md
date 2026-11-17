## 1. Group tools (own PR)

- [x] 1.1 `synth_controls.rs`: locks and undo per section and per row, seeded random from core `discover::Rng`
- [x] 1.2 Section header buttons (lock, undo, default, random) in `ui/synthesis.slint`, openVJ style
- [x] 1.3 Row-wide buttons for the SCENE row (source + effects) that skip locked sections, with their own undo
- [x] 1.4 Tests: random skips locked sections; undo restores; default resets

## 2. META controls

- [x] 2.1 `effects/layer_look.fs` hidden fixed effect: invert, brightness, gamma, contrast, hue, saturation, mirror v/h, limit colors + low/high, alpha; no-op at defaults (Master = the layer's Opacity)
- [x] 2.2 Store it on the layer (decided); save/load with the composition
- [x] 2.3 Render: one pass after the effects, skipped when no-op
- [x] 2.4 META row in Synthesis: switches, knobs, colour swatches; group tools on each section and the row
- [x] 2.5 Render tests: defaults are identical; invert, limit colors and alpha give the expected pixels

## 3. MEDIA overlay

- [x] 3.1 Overlay model (core `overlay.rs`): a small clip (image, video or camera) + blend mode + amount + scale/fit/flips + its own `look` for colour, on the layer
- [x] 3.2 Render the overlay through the clip pipeline and blend it over the visual; skip when there is none
- [x] 3.3 Transport: play, stop, previous/next in folder, loop mode, scrub bar, playback speed
- [x] 3.4 MEDIA section UI with transform and colour sections and their group tools

## 4. Media browser

- [x] 4.1 Media folders in preferences; listing images and videos via `browser.rs`; Live sources from the camera list
- [x] 4.2 Thumbnails for images and video first frames, made one at a time on a background thread and kept in memory
- [x] 4.3 Browser UI: chips, grid, refresh, add folder, lock

## 5. Finish

- [x] 5.1 README "What works"
- [x] 5.2 `openspec archive add-synthesis-controls -y`, `openspec validate --all --strict`
