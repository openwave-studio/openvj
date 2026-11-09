# Tasks

## 1. Render the model (PR 1)

- [x] 1.1 `mapping.wgsl` and a mapping pass: draw each slice's warp mesh into a screen target, sampling the composition output
- [x] 1.2 Screen and slice masks: rasterise mask contours into a mask texture when the setup changes; inverted masks union as in Resolume
- [x] 1.3 Output window shows Screen 1; passthrough when there is no setup
- [x] 1.4 Load and save the setup in openVJ's settings folder; import a Resolume preset from Settings → Output (moves to the mapping window in step 2)
- [x] 1.5 Tests: input crop, corner pin, masks and test card on the GPU; mask rasterising against `Screen::shows`
- [x] 1.6 Merged; the maintainer tests a downloaded build once GitHub Actions run again

## 2. Mapping window: corner pin and input selection (PR 2)

- [x] 2.1 `mapping.slint` window in the openDJ look: screen list, canvas, slice panel, stage tabs
- [x] 2.2 Output transformation: drag, nudge and type corners; reset warp
- [x] 2.3 Input selection: move and resize input rectangles
- [x] 2.4 Test card
- [x] 2.5 "Mapping" button next to "Output window"

## 3. Slices and masks (PR 3)

- [x] 3.1 Add, rename, reorder, enable, delete slices
- [x] 3.2 Add masks, edit points, invert
- [x] 3.3 README "What works"
- [x] 3.4 Archive this change
