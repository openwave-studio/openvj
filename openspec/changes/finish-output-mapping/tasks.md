## 1. One output window per screen

- [ ] 1.1 Model: display choice per screen (name + id), loading old `output-mapping.json` files
- [ ] 1.2 Render every enabled screen to its own target (today only the first)
- [ ] 1.3 Open, close and title one output window per enabled screen; fullscreen on the chosen display, fall back to a normal window when it's missing
- [ ] 1.4 Mapping window: screen list (add, rename, enable, remove, select) and display menu; editor works on the selected screen
- [ ] 1.5 Tests: two-screen import shows both screens; old setup file loads

## 2. Edit grid and bezier warps

- [ ] 2.1 Model helpers: resize a warp grid keeping its shape; linear ↔ bezier switch
- [ ] 2.2 Editor: draw grid points (and bezier handles), drag and nudge them; corners stay the corner pin
- [ ] 2.3 Grid size and point mode controls in Output transformation
- [ ] 2.4 Tests: resize keeps an identity warp an identity; imported bezier warp saves back unchanged

## 3. Soft edge

- [ ] 3.1 Import Luminance, Gamma and Red/Green/Blue gamma from `<SoftEdging>`; add them to the model
- [ ] 3.2 Find overlaps between Soft Edge slices' input areas and pass ramps to the shader (after Varda's `edge_blend.rs`, MIT, with attribution)
- [ ] 3.3 Ramp in `mapping.wgsl` with Resolume's Power, Luminance and gammas
- [ ] 3.4 Soft Edge switch per slice and setup soft-edge settings in the mapping window
- [ ] 3.5 Tests: overlap ramps sum to one at Luminance 0.5, Gamma 1; no ramp with Soft Edge off

## 4. Colour controls

- [ ] 4.1 Screen Opacity, Brightness, Contrast, R/G/B and slice Brightness, Contrast, R/G/B controls in the mapping window

## 5. Finish

- [ ] 5.1 README "What works": output mapping paragraph
- [ ] 5.2 `openspec archive finish-output-mapping -y`, `openspec validate --all --strict`
