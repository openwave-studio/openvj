## 1. Group tools (own PR)

- [ ] 1.1 Section knob state in the app built on core `discover::Knobs` (lock, undo, default, random), one per Synthesis section, kept per focused visual
- [ ] 1.2 Section header buttons (lock, undo, default, random) in `ui/synthesis.slint`, openVJ style
- [ ] 1.3 Row-wide buttons for the SCENE row (source + effects) that skip locked sections, with their own undo
- [ ] 1.4 Tests: random skips locked sections; undo restores; default resets

## 2. META controls

- [ ] 2.1 `effects/look.fs` hidden fixed effect: master, invert, brightness, gamma, contrast, hue, saturation, mirror v/h, limit colors + low/high, alpha, flips, fit/fill; no-op at defaults
- [ ] 2.2 Store it on the layer (decided); save/load with the composition
- [ ] 2.3 Render: one pass after the effects, skipped when no-op
- [ ] 2.4 META row in Synthesis: switches, knobs, colour swatches; group tools on each section and the row
- [ ] 2.5 Render tests: defaults are identical; invert, limit colors and alpha give the expected pixels

## 3. MEDIA overlay

- [ ] 3.1 Overlay model: a small clip (image, video or camera) + blend mode + amount + its own `look` for transform and colour
- [ ] 3.2 Render the overlay through the clip pipeline and blend it over the visual; skip when there is none
- [ ] 3.3 Transport: play, stop, previous/next in folder, loop mode, scrub bar, playback speed
- [ ] 3.4 MEDIA section UI with transform and colour sections and their group tools

## 4. Media browser

- [ ] 4.1 Media folders in preferences; listing images and videos via `browser.rs`; Live sources from the camera list
- [ ] 4.2 Thumbnails for images and video first frames, cached like library thumbnails, built in the background
- [ ] 4.3 Browser UI: chips, grid, refresh, add folder, lock

## 5. Finish

- [ ] 5.1 README "What works"
- [ ] 5.2 `openspec archive add-synthesis-controls -y`, `openspec validate --all --strict`
