# Tasks

## 1. Shadertoy names in the ISF engine
- [x] 1.1 Render preamble: `iTime`, `iTimeDelta`, `iFrame`, `iResolution`, `iDate`, `iMouse`, `iChannelTime`, `iChannelResolution`, `iSampleRate`; `mainImage` wrapper when there is no `main`
- [x] 1.2 512×2 Shadertoy audio texture, bound only when a shader uses a sound channel
- [x] 1.3 Scan `.glsl` files that define `mainImage` as generators
- [x] 1.4 Tests: naga check of a single-pass, a multi-pass and an audio shader

## 2. Converter
- [x] 2.1 core `shadertoy.rs`: API JSON (renderpass, inputs, outputs) → ISF text with PASSES, credit, link, licence; per-pass `mainImage` renaming; Common code
- [x] 2.2 Video/webcam `iChannel0` → effect with `inputImage`; unsupported channels listed
- [x] 2.3 Unit tests from hand-written JSON fixtures (no Shadertoy shaders committed)

## 3. Import UI
- [x] 3.1 Settings → Media: API key field, "uses the Shadertoy.com API" note
- [x] 3.2 Sources tab: Import from Shadertoy dialog (link/ID, or paste code), fetch on a worker thread, errors in the dialog
- [x] 3.3 Open on Shadertoy link in the clip and effect panels (from `OPENVJ.SOURCE`)
- [x] 3.4 Write to `~/Documents/openVJ/Effects/Shadertoy/`; re-importing a shader replaces its file, pasted code never overwrites another file

## 4. Finish
- [x] 4.1 README "What works" and THIRD_PARTY_NOTICES
- [x] 4.2 `openspec archive add-shadertoy-import -y`, `openspec validate --all --strict`

## 5. Texture channels (follow-up in the same PR)
- [x] 5.1 ISF `IMPORTED` images in core and render, a repeat sampler, a blank texture for the keyboard and unsupported channels
- [x] 5.2 Converter lists Shadertoy textures (wrap and flip); the import downloads them into `Shadertoy/textures/` before writing the shader
- [x] 5.3 GPU test: an imported texture repeats and clamps as Shadertoy's channel says
