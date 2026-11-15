# Tasks

## 1. Shadertoy names in the ISF engine
- [ ] 1.1 Render preamble: `iTime`, `iTimeDelta`, `iFrame`, `iResolution`, `iDate`, `iMouse`, `iChannelTime`, `iChannelResolution`, `iSampleRate`; `mainImage` wrapper when there is no `main`
- [ ] 1.2 512×2 Shadertoy audio texture, bound only when a shader uses a sound channel
- [ ] 1.3 Scan `.glsl` files that define `mainImage` as generators
- [ ] 1.4 Tests: naga check of a single-pass, a multi-pass and an audio shader

## 2. Converter
- [ ] 2.1 core `shadertoy.rs`: API JSON (renderpass, inputs, outputs) → ISF text with PASSES, credit, link, licence; per-pass `mainImage` renaming; Common code
- [ ] 2.2 Video/webcam `iChannel0` → effect with `inputImage`; unsupported channels listed
- [ ] 2.3 Unit tests from hand-written JSON fixtures (no Shadertoy shaders committed)

## 3. Import UI
- [ ] 3.1 Settings → Sources: API key field, "uses the Shadertoy.com API" note
- [ ] 3.2 Sources tab: Import from Shadertoy dialog (link/ID, or paste code), fetch on a worker thread, errors in the dialog
- [ ] 3.3 Open on Shadertoy link in the clip and effect panels (from `OPENVJ.SOURCE`)
- [ ] 3.4 Write to `~/Documents/openVJ/Effects/Shadertoy/`, never overwrite without asking

## 4. Finish
- [ ] 4.1 README "What works" and THIRD_PARTY_NOTICES
- [ ] 4.2 `openspec archive add-shadertoy-import -y`, `openspec validate --all --strict`
