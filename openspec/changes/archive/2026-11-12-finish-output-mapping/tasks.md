## 1. Edit grid and bezier warps

- [x] 1.1 Model helpers: resize a warp grid keeping its shape; linear ↔ bezier switch
- [x] 1.2 Editor: draw grid points (and bezier handles), drag and nudge them; corners stay the corner pin
- [x] 1.3 Grid size and point mode controls in Output transformation
- [x] 1.4 Tests: resize keeps the shape; grid points follow the corner pin; drag, nudge and type grid points; reset keeps size and mode

## 2. Outlines on the output

- [x] 2.1 Outlines button in the mapping window; draw slice outlines and the selected point on the output while the window is open

## 3. Colour controls

- [x] 3.1 Screen Opacity, Brightness, Contrast, R/G/B and slice Brightness, Contrast, R/G/B controls in the mapping window

## 4. One output window per screen

- [x] 4.1 Display choice per screen, stored in the screen's existing output device (old `output-mapping.json` files load unchanged); Resolume's "Display N" names find the Nth display
- [x] 4.2 Render every enabled screen to its own target (already the case)
- [x] 4.3 Open, close and title one output window per enabled screen; fullscreen on the chosen display, fall back to a normal window when it's missing
- [x] 4.4 Mapping window: screen list (add, rename, enable, remove, select) and display menu; editor works on the selected screen
- [x] 4.5 Tests: add, edit, rename, disable and remove screens; display choice and lookup

## 5. Soft edge

- [x] 5.1 Import Luminance, Gamma and Red/Green/Blue gamma from `<SoftEdging>`; add them to the model
- [x] 5.2 Find overlaps between Soft Edge slices' input areas (on any screen) and pass per-edge widths to the shader
- [x] 5.3 Ramp in `mapping.wgsl` with Resolume's Power, Luminance and gammas (Paul Bourke's edge-blend curve)
- [x] 5.4 Soft Edge switch per slice and setup soft-edge settings in the mapping window
- [x] 5.5 Tests: ramps sum to one at Luminance 0.5; overlap widths; no fade with Soft Edge off or identical inputs; GPU render of two blended screens

## 6. Finish

- [x] 6.1 README "What works": output mapping paragraph
- [x] 6.2 `openspec archive finish-output-mapping -y`, `openspec validate --all --strict`
