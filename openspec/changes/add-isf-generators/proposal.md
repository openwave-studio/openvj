# Proposal

## Why

The maintainer wants Ghost Arcade's ISF pack (github.com/riskcapital/ghost-arcade, AGPL-3.0) in openVJ, effects and generators, bundled. A test run of all 409 unique Ghost Arcade `.fs` files through openVJ's ISF engine (naga, 4 rendered frames each) found that only 19 are filters (they already load through the effects folder); the other 390 are generators, which openVJ can't play as clips yet. 295 compile as they are, and 389 with a small translation of Ghost Arcade's own built-in names. On 2026-11-09 the maintainer asked to bring ISF generators forward to v0.1 for this; the standing "generators in v0.4" decision still covers new hand-written generators (Waveform, Line Scape, Metaballs, text).

## What Changes

- **ISF generator clips.** An ISF file without an `inputImage` input is a generator. Generators appear in the browser's Sources tab under the built-in ones, grouped by their `CATEGORIES`, and play as clips like Resolume's Sources (Resolume lists its generators the same way). Their inputs show as clip parameters with the same widgets, units and modulation as effect parameters, saved by name. Generators get `TIME`, `BEAT`, `BPM` and the audio bands, and follow the clip's speed and beat sync.
- **Generator folders.** ISF generators load from the same folders as effects (`~/Documents/openVJ/Effects`, the standard ISF folders, `OPENVJ_EFFECTS_DIR`), with the same hot reload and error rows.
- **Ghost Arcade compatibility.** The ISF preamble also understands Ghost Arcade's extensions: `audioLevel`, `audioBass`, `audioMid`, `audioHigh`, `audioBeat`, `audioBeatPhase`, `audioBPM`, `audioSpectralCentroid`, `sampleFFT()`, `sampleWaveform()`, `renderSize`, and files that redeclare built-in uniforms or set `precision`. Any ISF file can use these names.
- **Bundled pack.** openVJ ships Ghost Arcade's shaders that compile and are licence-clean in `shaders/ghost-arcade/` (about 378 files, 2.7 MB), with Ghost Arcade's AGPL-3.0 LICENSE, a NOTICE crediting Risk Capital Media and the authors, and the upstream commit. They are loaded from the app bundle at runtime (not compiled into the binary), and the macOS .app copies the folder in. The repo stays GPL-3.0; these files stay AGPL-3.0 (GPL-3.0 section 13 allows combining the two).
- **Left out of the bundle:** files derived from Shadertoy, glslsandbox or mojovideotech (about 11, usually CC BY-NC-SA, which is non-commercial and not GPL-compatible), and the ~20 that don't compile. Users can still drop those into their own folder.

Out of scope: extra image inputs (`videoInput`, 7 files), audio/FFT textures (approximated from the four audio bands), `.vs` vertex shaders, Ghost Arcade's thumbnails and featured lists, 3D/stage features, importing Resolume's own generators from .avc.

## Capabilities

### New Capabilities

### Modified Capabilities
- `media-sources`: ISF generator clips, generator folders, bundled Ghost Arcade pack.
- `effects`: Ghost Arcade names in the ISF preamble.

## Impact

Clips gain a third kind of source with named parameters, so saved compositions get a new clip source type. Old files load unchanged.

## Touches

- crates/openvj-core: `composition.rs` (new clip source, named params) — shared hot spot; `effect.rs`, `effect_folders.rs` (generators in the library)
- crates/openvj-render: `isf.rs` (preamble shim), `lib.rs` (render ISF generator clips) — shared hot spot
- crates/openvj-app: `app.rs` (Sources tab, clip parameters) — shared hot spot; `ui/main.slint` small
- New `shaders/ghost-arcade/` (AGPL files, LICENSE, NOTICE), `scripts/import-ghost-arcade.sh` (fetch, filter, test), `packaging/` (copy into .app)
- README.md ("What works", credits); specs: media-sources, effects

## Conflicts

- PR #19 (add-user-effects-folder): this builds on its branch (reloadable library, folder scan). Merge #19 first.
- PR #18 (perf) touches `openvj-render/src/lib.rs` and `app.rs`; merge before this.
- Output mapping, Syphon output and HAP threads touch `app.rs`, `main.slint`, `render/lib.rs`; whichever lands second rebases. Clip-source change in `composition.rs` will be announced to them.
- add-parameter-animation: both touch clip parameters; this keeps the existing per-parameter modulator so either order works.

## Open questions

1. Sources tab: 378 generators is a lot. Group by Ghost Arcade's categories (Generator, Visual, Simulation, 3D Room, Audio Reactive) in collapsible sections, or a search box first?
2. Bundle all ~378, or only a curated set (for example the ones that render something and move), with the rest as an optional download?
3. Fine to let users' folder generators and the bundled ones share one list, with the folder name shown?
