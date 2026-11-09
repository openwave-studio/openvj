# 01 – OBS as a host: how much can we lean on it?

The short answer: **a lot for the plumbing, very little for the performance UX.** OBS already has a cross-platform GPU renderer, a plugin system, capture sources, outputs and a remote-control protocol. What it lacks is the VJ layer: clip launching, layer and column semantics, tempo, transitions inside a layer, a clip-oriented UI and real-time playback of performance codecs.

Checked against OBS Studio `master` (32.x, October 2026). API names are from `libobs/obs.h`, `libobs/obs-source.h` and `libobs/graphics/graphics.h`.

## What OBS gives us

| Need | What OBS provides | Notes |
|---|---|---|
| GPU rendering | `libobs` graphics subsystem (`gs_*`): Direct3D 11 on Windows, OpenGL on Linux and macOS, plus an **experimental Metal renderer** on Apple Silicon since 32.0 | One API for all backends. Shaders are written in OBS's `.effect` language (HLSL-flavoured) and translated per backend. |
| Offscreen render targets | `gs_texrender_*` | Needed for per-layer render-to-texture, feedback effects and multi-pass shaders. |
| Compressed textures | `GS_DXT1`, `GS_DXT3`, `GS_DXT5` in `enum gs_color_format`, mapped to `BC1`/`BC3` on D3D11 and `S3TC` on OpenGL | **Key enabler for HAP**: we can upload DXT frames straight to the GPU with no CPU decompression. No BC4/BC7 though (see HAP notes in 02). Metal support still needs checking. |
| Plugin model | `obs_source_info` (sources, filters, transitions), `obs_module_*`, frontend API for docks and menus | A composition source is just an `OBS_SOURCE_VIDEO \| OBS_SOURCE_COMPOSITE` source with custom `video_render`. |
| Hosting other sources | `obs_source_create_private`, `obs_source_video_render`, `enum_active_sources`, `audio_render` | Lets a layer play **any** OBS source (camera, browser, NDI, window capture) as a clip, with proper active/showing refcounting. |
| Hosting filters | `obs_source_filter_add` on private sources | Lets users drop **any** installed OBS filter onto an openVJ clip or layer. |
| Blend modes | `obs_sceneitem_set_blending_mode`: Normal, Additive, Subtract, Screen, Multiply, Lighten, Darken | Fine for a "scenes as layers" prototype. Too few for VJ use (no Overlay, Difference, Exclusion, Luma/Chroma key and so on), so openVJ will do its own blending in shaders. |
| Transitions | Transition sources (fade, stinger, luma wipe, plus the Move plugin) | Scene-level only. Per-layer clip transitions have to be our own. |
| Media playback | `ffmpeg_source` / `media-playback` (libavcodec, hardware decode for H.264/HEVC) | Good for streams and long-GOP files, poor for fast scrubbing and scratching. HAP decodes on the **CPU** today. |
| Output | Fullscreen projectors (per monitor), Virtual Camera, recording, streaming, Replay Buffer | The projector window is already a decent "output to projector" path. |
| Network video | NDI via DistroAV; Spout2 (Windows) and Syphon (macOS) via community plugins | Covered as long as openVJ is a normal source. |
| Remote control | **obs-websocket 5** is built in (since OBS 28) | We can register our own vendor requests (`obs_websocket_vendor_*`) so web and phone controllers can drive openVJ. |
| Audio | Per-source audio, `obs_source_add_audio_capture_callback` | Tap any OBS audio source for FFT and beat detection, so there is no need to open audio devices ourselves. |
| Settings and persistence | `obs_data_t` (JSON), saved in the scene collection | The composition can save inside the scene collection for free. A separate `.ovj` export is still wanted for portability. |
| Hotkeys | `obs_hotkey_register_*` | Basic keyboard triggering. |
| UI toolkit | Qt 6 (Widgets); plugins can add docks (`obs_frontend_add_dock_by_id`) | Use Qt Widgets. Don't count on Qt Quick being in every OBS build. |

## Where OBS falls short for VJing

1. **No clip/deck model.** Scenes and sources aren't clips in a grid with launch, cue and column-trigger behaviour.
2. **No tempo.** There's no BPM clock, tap tempo, beat-synced parameters or quantised launching.
3. **Too few blend modes** (7) and no per-layer opacity fader other than the Color Correction filter.
4. **Video timing.** `ffmpeg_source` is built for broadcast playback. Speed changes, reverse, ping-pong loops, scratching and frame-exact cue points need our own player.
5. **Frame-rate coupling.** Everything renders on OBS's single video clock (`obs_video_info.fps_*`). That's good for determinism, but the canvas *is* the output rate, so a 60 fps show needs OBS set to 60 fps.
6. **Shader authoring.** obs-shaderfilter (exeldro, v2.6) proves custom shaders work, but VJ content (ISF, Shadertoy, Resolume Wire-style FFGL) is written in GLSL, not OBS effect language.
7. **Output mapping.** There's no slicing, soft-edge or projection mapping. Projectors show the whole canvas.
8. **Plugin ABI churn.** OBS 32 won't load plugins built for a *newer* OBS. There was also a dock-API signature change in 32.0.x. Pin a minimum OBS version and build against it in CI.

## Integration options

### A. "Scenes are layers" (thin, script-level)

Use an OBS scene as the composition, scene items as layers, sources as clips and websocket or Lua as glue.

- Pro: almost no C/C++, works today, and every OBS feature applies.
- Con: no real clip grid, 7 blend modes, no per-layer transitions, no tempo, and the scene list becomes a mess. It's fine for a demo but hits a ceiling quickly.

### B. One composition source with its own render graph (recommended)

A native plugin registers an `openvj_composition` source. Internally it holds layers, then clips, then effects, renders each layer into a `gs_texrender`, and blends them with our own shaders. Clips can wrap private OBS sources, so we lose none of OBS's capture abilities.

- Pro: full control over blending, timing, transitions and tempo. The output is still a normal OBS source, so it works with projectors, streaming, NDI and virtual camera. Several compositions can coexist (for example a main output plus a separate stage-screen output).
- Con: more code, and we own a video player and a shader pipeline.

### C. Standalone app using libobs as an engine

Same engine as B, but in our own executable that links libobs (the way some broadcast tools embed it).

- Pro: complete control over the UI and window layout.
- Con: we lose the "it's just OBS" story, OBS's plugin ecosystem in the same process, and the user's existing setup.

**Recommendation: B**, structured so the core (render graph, clip players, tempo) doesn't depend on the OBS frontend. That leaves C open later if the UI outgrows OBS docks. Option A remains a useful week-one spike to validate the UX before building the C++.

## Performance notes

- OBS composites in **8-bit sRGB or 16-bit float** depending on the colour format and space settings. Blend shaders must respect `gs_get_linear_srgb()` or Screen and Add will look wrong.
- Keep everything on the GPU: no `gs_stagesurface` readbacks in the hot path. Thumbnails should come from the file decoder, not GPU readback.
- One `gs_texrender` per active layer, plus one per multi-pass effect. At 1080p RGBA16F that's about 16 MB each, which is cheap. At 4K it's about 66 MB, so budget it.
- Inactive clips must release decoders and private sources (`obs_source_dec_active`) or browser and camera sources keep running.
