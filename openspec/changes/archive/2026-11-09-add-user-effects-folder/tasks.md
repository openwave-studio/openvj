# Tasks

## 1. Loader (one PR)

- [x] 1.1 Make the effect library reloadable (an `Arc` snapshot instead of `'static`), update render, Resolume and MIDI users
- [x] 1.2 Scan effect folders, ids, clashes, skip generators, errors per file
- [x] 1.3 GLSL check off the GPU (naga) so errors show in the browser before use
- [x] 1.4 Watch folders (poll ~1 s) and swap the library; render recompiles changed effects
- [x] 1.5 Effects tab lists folder effects and dimmed error rows
- [x] 1.6 Tests: scan, clash rules, reload keeps params, broken file
- [x] 1.7 README "What works" and "Writing effects"
- [x] 1.8 Archive this change
