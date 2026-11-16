# Design

## Engine: lactea, not projectM

lactea takes the host's `wgpu::Device` and `Queue` and records into the host's command encoder, rendering into any texture view of the configured format. openVJ uses wgpu 30, as lactea 0.2 does, so a MilkDrop frame is one more draw in the frame's command buffer: no second graphics API, no copies. projectM would need an OpenGL context (deprecated on macOS) and an IOSurface or CPU copy per frame into Metal, plus a C++/CMake build. lactea is young (one author, Oct 2026), so the version is pinned and, being MIT/Apache, it can be vendored or forked if needed.

## One engine per clip

A MilkDrop preset is a simulation with feedback, and its per-frame and per-vertex equations run on the CPU. Running one per place a clip is drawn (layer, preview, thumbnail) would triple the CPU cost and give three different pictures. Instead each MilkDrop clip owns one lactea engine that renders at most once per frame at the composition size into its own `Rgba16Float` texture; the layer, preview and thumbnail copy from it. A clip that is only visible as a thumbnail advances at the thumbnail rate (15 fps). Engines of clips that weren't drawn in a frame are dropped, like ISF feedback buffers.

## Audio

MilkDrop presets read their own bass/mid/treble from raw PCM, not openVJ's bands, so the app hands each frame's mono samples to the render engine, which pushes them to every MilkDrop engine. Sensitivity × Reactivity is lactea's volume sensitivity.
