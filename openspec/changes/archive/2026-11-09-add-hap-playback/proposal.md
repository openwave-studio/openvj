# Proposal

## Why

HAP is the most common GPU codec in VJ libraries after DXV: Resolume, VDMX, Millumin, TouchDesigner and most clip packs ship it. Today openVJ plays HAP through FFmpeg, which expands every DXT frame to RGBA on the CPU and pipes 8 MB per 1080p frame, so a few HAP layers cost as much as H.264. HAP R doesn't play at all. HAP frames are DXT / BC7 textures behind Snappy compression, so they fit the GPU path DXV already uses: read the frame from the QuickTime file, undo Snappy, upload the blocks.

## What Changes

- HAP, HAP Alpha, HAP Q, HAP Q Alpha and HAP R movies play straight on the GPU: openVJ reads each frame from the `.mov`, decompresses Snappy (single or chunked, as encoders with "chunks" write), and uploads BC1 / BC3 / BC4 / BC7 textures.
- HAP Q's scaled YCoCg and HAP Q Alpha's separate alpha texture are turned into RGBA in the media shader, like DXV HQ.
- The frame-reading worker that follows the clip clock is shared by DXV and HAP instead of being copied.
- If the GPU can't sample BC textures or a HAP frame doesn't decode, the clip falls back to FFmpeg (HAP R has no fallback: FFmpeg can't decode it, so the clip shows the usual "Could not play video").
- The clip's codec label shows the HAP variant ("HAP Q", "HAP alpha"...).

Out of scope: HAP Alpha-Only and HAP HDR (rare in VJ packs; they can follow on the same path), HAP in AVI or MP4 containers, encoding or converting to HAP.

## Capabilities

### New Capabilities
- `hap-playback`: HAP movies play on the GPU as compressed textures.

### Modified Capabilities

## Impact

`openvj-media` gains a `hap` module and the `snap` crate (pure Rust Snappy, BSD-3). `openvj-render` gains BC layouts for straight-alpha BC3, BC7 and scaled YCoCg.

## Touches

- New: `crates/openvj-media/src/hap.rs` (+ tests), `crates/openvj-media/src/player.rs` (shared frame worker)
- `crates/openvj-media/src/dxv.rs` (uses the shared worker), `crates/openvj-media/src/lib.rs` (codec label), `crates/openvj-media/Cargo.toml`
- `crates/openvj-render/src/lib.rs` and `shaders.wgsl` (shared hot spot): new `BcLayout` variants, small `set_media_bc` change
- `crates/openvj-app/src/app.rs` (shared hot spot): `Player::Hap` and opening it
- README.md (shared hot spot): "What works"
- Specs: new `hap-playback`

## Conflicts

Open PR #18 (performance) edits `crates/openvj-app/src/app.rs` and `crates/openvj-media/src/lib.rs` near the video player code. Suggested order: #18 first, then this PR rebases (the overlap is a few lines in `open_player` and the `Player` enum).

## Open questions

- Is HAP R worth keeping in this change? It's cheap on the GPU path, but few packs use it. (Going ahead with it unless the maintainer says otherwise.)
- Any HAP files from the maintainer's library that misbehave today? Testing uses FFmpeg-made HAP files, not the maintainer's clips.
