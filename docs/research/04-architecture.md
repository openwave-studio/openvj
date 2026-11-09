# 04 – Architecture

## Overview

```
                         ┌─────────────────────── OBS Studio process ───────────────────────┐
                         │                                                                   │
 MIDI / OSC / Link ───▶  │  ┌──────────── openvj-control ────────────┐                       │
 obs-websocket ───────▶  │  │ mapping engine · tempo clock · LFOs     │                       │
                         │  └──────────────┬─────────────────────────┘                       │
                         │                 │ commands (lock-free queue)                       │
 Qt UI (docks) ───────▶  │  ┌──────────────▼──────────── openvj-core ───────────────────┐    │
                         │  │ Model: Composition ▸ Layers ▸ Clips ▸ Effects ▸ Params     │    │
                         │  │ Engine: clip players, transitions, param evaluation        │    │
                         │  └──────┬───────────────────────┬────────────────────────────┘    │
                         │         │ frame requests        │ render plan per frame             │
                         │  ┌──────▼────────┐      ┌───────▼──────── openvj-render ──────┐    │
                         │  │ openvj-media  │      │ gs_texrender per layer              │    │
                         │  │ decode threads│─────▶│ effect chains (ISF/.effect/OBS)     │    │
                         │  │ HAP·FFmpeg·img│ tex  │ blend shaders → composition texture │    │
                         │  └───────────────┘      └───────┬─────────────────────────────┘    │
                         │                                 │                                  │
                         │                     openvj_composition (obs source)               │
                         │                                 │                                  │
                         │         scenes ▸ projector · stream · record · vcam · NDI/Spout    │
                         └───────────────────────────────────────────────────────────────────┘
```

## Modules

| Module | Responsibility | Depends on OBS? |
|---|---|---|
| `openvj-core` | Data model, clip state machines, tempo maths, parameter evaluation (LFO, BPM sync, audio, mapping), command queue, serialisation | **No.** Pure C++, unit-tested headless. |
| `openvj-media` | Decoders (HAP direct, FFmpeg generic, image sequences), frame cache, read-ahead threads, thumbnail jobs | FFmpeg only. GPU upload goes through a small interface. |
| `openvj-shaders` | ISF parse, GLSL → SPIR-V → OBS `.effect` translation, shader cache, built-in effect library | Only for the final `gs_effect_create`. |
| `openvj-render` | Per-frame render plan executed with `gs_*`: layer textures, effect passes, blend shaders, feedback buffers | **Yes.** Graphics thread. |
| `openvj-obs` | Module entry, source/filter registration, hosting OBS sources as clips and OBS filters as effects, audio capture taps, websocket vendor, hotkeys | **Yes.** |
| `openvj-control` | MIDI (libremidi), OSC, Ableton Link, mapping profiles, LED feedback | No |
| `openvj-ui` | Qt docks, workspace window, clip grid, properties, browser, widget factory | Qt plus the OBS frontend API |

Keeping `core` independent of OBS makes it testable. It also leaves room for the "standalone libobs app" option later (01, option C) or even a non-OBS backend.

## OBS objects we register

| `obs_source_info.id` | Type | Purpose |
|---|---|---|
| `openvj_composition` | Input (`VIDEO \| COMPOSITE \| CUSTOM_DRAW`) | The mixer. Its settings store the composition. Several can exist. |
| `openvj_clip` | Input | A single openVJ clip (HAP file or ISF shader) usable as a normal OBS source without the mixer. Doubles as a test harness. |
| `openvj_isf_filter` | Filter | Run any ISF effect on any OBS source, like obs-shaderfilter but ISF. |
| `openvj_output` *(later)* | Input | Samples a composition through a slice/warp mesh for projection mapping. |

## Data model

```
Composition
 ├─ settings: resolution (default = OBS canvas), fps (follows OBS), bpm, quantise, crossfader
 ├─ columns[]: name, colour
 ├─ layers[]  (index 0 = bottom)
 │   ├─ opacity, blend_mode, bypass, solo, crossfader_group (A/B/none)
 │   ├─ transition: type, duration (beats|secs)
 │   ├─ effects[]                ── layer effect chain
 │   └─ clips[column] → Clip?
 │        ├─ source: { kind: hap|ffmpeg|imgseq|isf|shadertoy|obs_source, ref }
 │        ├─ playback: in/out, speed, direction, loop mode (loop|pingpong|once|hold), beat-sync length
 │        ├─ launch: mode (trigger|gate|toggle), quantise (inherit|off|beat|bar)
 │        ├─ transform: position, scale, rotation, anchor, crop, fit (fit|fill|stretch|native)
 │        ├─ effects[]           ── clip effect chain
 │        └─ dashboard: 8 macros
 ├─ effects[]                    ── master chain
 └─ mappings[]: { input: midi|osc|audio|lfo, target: param path, range, curve }

Effect
 ├─ kind: builtin | isf | obs_effect_file | obs_filter
 ├─ ref, bypass, mix (dry/wet)
 └─ params{ name → Param }

Param
 ├─ type: float|int|bool|color|point2d|choice|image
 ├─ value, default, min, max
 └─ modulator?: lfo|bpm|audio_band|envelope   (evaluated by core each frame)
```

Every parameter has a stable **path** (`/layer/2/clip/active/effect/0/amount`). MIDI, OSC, websocket, undo and the UI all address parameters the same way.

## Threads and data flow

| Thread | Owner | Does |
|---|---|---|
| **UI** (Qt main) | OBS | Edits the model by sending **commands**. Never touches GPU objects. |
| **Control** | openVJ | MIDI/OSC/Link callbacks, which become commands. |
| **Decode workers** (N) | openVJ | Read and decompress frames ahead into CPU buffers (HAP: snappy → DXT blocks). |
| **OBS video tick** | OBS (`video_tick`) | Drain the command queue, advance tempo and clip clocks, evaluate modulators, build the **render plan** (immutable snapshot). |
| **OBS graphics** | OBS (`video_render`) | Upload pending frames, run effect passes and blends from the render plan. |
| **OBS audio** | OBS | Capture callbacks feed lock-free FFT ring buffers. |

Rules:
- The model is mutated **only on the tick**, via commands. The UI gets back a state snapshot (atomic `shared_ptr` swap) for display. That avoids locks in the render path and makes everything replayable.
- GPU resources are created and destroyed **only on the graphics thread** (inside `obs_enter_graphics` / `video_render`).
- Clip time is derived from **OBS's frame timestamps**, not wall-clock, so recordings are frame-accurate and deterministic.

## Render pipeline per frame

```
for layer in layers (bottom → top), skipping bypassed / not-soloed:
    for each playing clip on layer (two during a transition):
        tex = clip.current_texture()                  # HAP upload, FFmpeg frame, ISF pass, or obs_source_video_render into texrender
        tex = apply(clip.effects, tex)                # ping-pong between two texrenders
        tex = apply(clip.transform, tex)
    tex = transition_blend(outgoing, incoming, t)      # if transitioning
    tex = apply(layer.effects, tex)
    comp = blend(comp, tex, layer.blend_mode, layer.opacity)   # our blend shader; comp is a ping-pong pair
comp = apply(master.effects, comp)
draw comp to OBS                                       # inside video_render
```

- Blend modes come from one shader with a technique per mode: Normal, Add, Subtract, Multiply, Screen, Overlay, Soft/Hard Light, Difference, Exclusion, Lighten, Darken, Colour Dodge/Burn, Luma Key, Chroma-ish Key, Alpha.
- Work in **linear float** (`GS_RGBA16F`) when OBS runs a high-precision colour format, or 8-bit sRGB-aware otherwise.
- Feedback effects keep a persistent per-effect texrender (ISF `PERSISTENT` buffers map directly onto this).

## Persistence

- **In OBS:** the composition serialises to JSON inside the `openvj_composition` source settings, so it's saved with the scene collection and works with OBS backups and profiles.
- **Portable:** `File ▸ Export Composition` writes a `.ovj` bundle (a zip holding the JSON plus optional copies of the media). Media paths are stored relative where possible, with a "relink missing media" dialog on load.
- **Schema version** in every file from day one, with migration functions in `core`.

## Roadmap

| Phase | Goal | Contents |
|---|---|---|
| **0 – Spike** (1–2 wks) | Validate OBS assumptions | Plugin template builds on all three OSes. `openvj_clip` plays HAP via direct DXT upload. One ISF shader compiles through glslang → SPIRV-Cross → `.effect` on D3D11 and GL. Measure: 8 × 1080p HAP layers at 60 fps. |
| **1 – Mixer core** | Usable with mouse only | Composition source, layers, blend modes, opacity, clip grid dock, launch/clear, FFmpeg and image clips, OBS sources as clips, save/load. |
| **2 – Performance** | Playable live | BPM, tap, Ableton Link, launch quantise, layer transitions, MIDI map mode, OSC, properties dock with effect stacks, ISF library browser. |
| **3 – Reactive** | Audio and modulation | FFT/onset from OBS audio, LFOs, dashboard macros, controller profiles with LED feedback. |
| **4 – Output** | Venue features | `openvj_output` slices/warping, multiple compositions, a workspace window, a HAP transcode helper in the browser. |

## Open questions

1. **Licence:** GPL-2.0+ (maximum compatibility) or GPL-3.0 (allows aubio/BTrack)? Decide before writing the audio module.
2. **Metal:** does OBS's Metal renderer support `GS_DXT*` and our generated effects? Check during Phase 0. Fall back to the OpenGL renderer on macOS if not.
3. **BC4/BC7 in libobs:** worth upstreaming for HAP Q Alpha and HAP R? It's a small, self-contained PR to obs-studio and would also help other plugins.
4. **Composition fps vs OBS fps:** should a composition ever run faster or slower than the OBS canvas? Probably not. Simpler to document that "OBS fps = show fps".
5. **Repository:** openVJ probably deserves its own repo (from obs-plugintemplate) rather than living next to the M32 bridge.
