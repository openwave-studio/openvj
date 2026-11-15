# Design

## Convert on import, not at load

The import writes a plain ISF `.fs` file instead of keeping Shadertoy JSON and converting it every time it loads. The file is then an ordinary user shader: it hot-reloads, can be edited by hand, shared with VDMX or Magic, and needs no network ever again. This is what VDMX's ISF Editor does ("Import from Shadertoy").

The Shadertoy names (`iTime`, `mainImage`…) are still handled by a preamble in the render crate rather than rewritten in the source, so hand-copied Shadertoy code dropped into the folder works too, and the imported file stays close to the original for diffing against Shadertoy.

## Passes

Shadertoy runs Buffer A, B, C, D, then Image, each frame; a buffer reading itself (or a later buffer) sees last frame's result. ISF `PASSES` run in order with `PERSISTENT` targets that keep the previous frame, so the mapping is direct: each buffer becomes a full-size, float, persistent pass named `BufferA`…`BufferD`, and the Image pass is the last, unnamed pass. Each pass's `iChannelN` becomes a `#define` to the matching pass target for that pass, so name clashes between passes (the known failure of VDMX's importer) are avoided by putting each pass's own code inside `if (PASSINDEX == n)` branches with renamed `mainImage` functions (`mainImage_BufferA`…).

## Audio texture

Shadertoy's sound channel is a 512×2 texture: row 0 the FFT (about 0–11 kHz, linear), row 1 the waveform. openVJ's `isf_spectrum` is 256 log-spaced bins. The renderer adds a second small texture, filled from the same analysis each frame, only when a shader uses a sound channel, so other shaders pay nothing.
