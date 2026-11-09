# 02 – Key libraries

Grouped by job. Before adding a dependency, check its licence against GPL-2.0-or-later (OBS's licence).

## Video playback

VJ playback cares about **instant start, scrubbing, reverse, varying speed and many simultaneous files**, not compression ratio. That points to intra-frame, GPU-decoded codecs.

| Codec | Why | Library path | Notes |
|---|---|---|---|
| **HAP / HAP Alpha** | Industry standard for VJ and media servers (Resolume, VDMX, TouchDesigner, Millumin). Frames are Snappy-compressed DXT1/DXT5. | Demux with **FFmpeg libavformat** (already in OBS). Unwrap HAP sections with Vidvox's **hap** reference code (BSD-2). Decompress with **Snappy** (BSD-3). Upload as `GS_DXT1`/`GS_DXT5` directly. | No CPU texture decode: the GPU samples DXT natively. This is the main reason to own our player instead of using `ffmpeg_source` (which calls FFmpeg's HAP decoder and gets CPU-side RGBA). |
| **HAP Q** | Higher quality. DXT5 holding scaled YCoCg. | Same as above, plus a YCoCg→RGB shader. | Easy. |
| **HAP Q Alpha / HAP R** | Use BC4 (alpha plane) and BC7. | libobs has no BC4/BC7 formats. | Options: decode alpha on the CPU, or upstream BC4/BC7 to libobs. Defer. |
| **DXV3** (Resolume's own) | Lots of existing VJ content. | FFmpeg has a `dxv` decoder (CPU). | CPU path at first. GPU path later, since it's also DXT-based. |
| **NotchLC** | Higher-quality intra codec. | FFmpeg `notchlc` decoder (CPU). | Low priority. |
| **ProRes, H.264, HEVC, AV1** | What users actually have. | FFmpeg plus hardware decode (`hwaccel`: D3D11VA, VideoToolbox, VAAPI). Or just use OBS's own `ffmpeg_source` as a clip. | Long-GOP files scrub badly. Offer a **"convert to HAP"** action in the browser (FFmpeg can encode HAP: `-c:v hap -format hap_q`). |
| **Image sequences / stills / GIF** | Common for loops. | `gs_create_texture_file_data` (OBS uses stb_image/libpng) or FFmpeg's image2 demuxer. | Preload small loops into VRAM. |

Supporting pieces:
- **FFmpeg** (LGPL/GPL): demuxing, software decoding, thumbnails, transcoding. Link to the copy OBS ships. Don't bundle a second one.
- **Snappy** (BSD-3): HAP payload decompression.
- **Vidvox/hap** (BSD-2): reference HAP section parser.
- A small **frame cache / ring buffer** per clip with read-ahead, in our own code. Decoding happens on worker threads and uploads on the graphics thread.

## Shaders and generators

| Option | What it is | Fit |
|---|---|---|
| **ISF** (Interactive Shader Format, VIDVOX, v2) | A GLSL fragment shader with a JSON header declaring inputs (float, color, point2D, image, audio, audioFFT), multiple passes and persistent buffers. Large free library (VDMX, Magic, and others). | **Primary shader format.** The header maps directly onto a parameter UI and onto MIDI/OSC mapping. Parsers exist (C++ `ISF-Spec` reference / VVISF, Rust `isf` crate). |
| **Shadertoy** | GLSL with `iTime`, `iResolution`, `iChannel0..3`, `mainImage()`. | Import path: wrap it into an ISF-like template. |
| **OBS `.effect`** | OBS's native HLSL-ish shader language. | What obs-shaderfilter uses. Support it as a third format so existing OBS shader packs work. |
| **FFGL** (Resolume plugin SDK) | Native C++ plugins with OpenGL. | Only works on the OpenGL backend, so it doesn't fit D3D11 OBS. Skip. |

**Getting GLSL to run on D3D11 and Metal.** OBS on Windows uses D3D11, so ISF/Shadertoy GLSL has to be cross-compiled:

- **glslang** (BSD-3 / Apache-2.0): GLSL → SPIR-V.
- **SPIRV-Cross** (Apache-2.0): SPIR-V → HLSL (D3D11, SM 5.0), MSL (Metal) or GLSL (OpenGL).

The catch is that libobs compiles its own `.effect` dialect, not raw HLSL or GLSL. Two ways around it:

1. **Translate to OBS effect syntax.** Generate an `.effect` source from the SPIRV-Cross HLSL output (they're close: OBS effects are HLSL-like with `technique`/`pass` blocks), then load it with `gs_effect_create`. That keeps us inside the public API and works on every backend OBS supports. **Start here.**
2. **Backend-native shaders.** Use `gs_get_device_obj()` to reach the raw D3D11 device or GL context and run our own shader pipeline. It's faster to iterate on but backend-specific and fragile. Avoid it.

Cache compiled shaders on disk, keyed by a hash of the source.

## Control: MIDI, OSC, tempo

| Need | Library | Notes |
|---|---|---|
| MIDI in/out | **libremidi** (BSD-2, modern C++, MIDI 2.0 ready) or **RtMidi** (MIT) | libremidi is preferred. Needed for controller mapping and LED feedback (APC40, Launchpad, Midi Fighter Twister, and the M32 in this repo). |
| OSC | **oscpack** (MIT-style) or **liblo** (LGPL-2.1) | Address space like `/openvj/layer/1/opacity`, mirroring Resolume's OSC layout so TouchOSC templates port easily. |
| Ableton Link | **Ableton Link** SDK (GPL-2.0+, so compatible) | Tempo and phase sync with DJs and Ableton. A big win for a small cost. |
| MIDI clock in | Via libremidi | Second tempo source. |
| Remote/web control | **obs-websocket** vendor requests (built into OBS) | Free web/phone control surface. |

## Audio analysis (audio-reactive visuals)

| Need | Library | Notes |
|---|---|---|
| FFT | **KissFFT** (BSD-3) or **PFFFT** (BSD-like) | Feed ISF `audioFFT` inputs and band levels (low/mid/high). |
| Onset and beat tracking | **aubio** (GPL-3.0) or **BTrack** (GPL-3.0) | Both GPL-3, so linking makes the whole plugin GPL-3. Either accept GPL-3 for openVJ or write a simple spectral-flux onset detector ourselves. Decide early. |
| Audio input | `obs_source_add_audio_capture_callback` | Tap any OBS audio source: mic, desktop audio, DJ mixer input. |

## Output beyond OBS

Most of this is already covered by OBS and its plugins: projector, virtual camera, NDI (DistroAV), Spout2, Syphon. Later we might add:
- **Advanced output / mapping**: slices, soft-edge blending, warping. Build it as a second source type, `openvj_output`, that samples a composition through a mesh.
- **DMX / Art-Net pixel mapping**: `libartnet` or a small custom sender. Very low priority.

## Build and tooling

- **CMake** with the official **obs-plugintemplate** (gives CI for Windows, macOS and Ubuntu, packaging and signing hooks).
- **C++20**, Qt 6 Widgets (match OBS's Qt version exactly).
- Dependencies through the plugintemplate's `buildspec.json`, or vcpkg for libremidi, glslang, SPIRV-Cross and snappy.
- Tests: **Catch2** or **doctest** for the core (tempo, clip-timing maths, ISF parsing, HAP parsing), which doesn't need OBS.
