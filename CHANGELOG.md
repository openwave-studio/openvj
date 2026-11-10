# Changelog

All notable changes to openVJ are listed here. Versions follow [Semantic Versioning](https://semver.org); until 1.0, a minor version may change file formats.

Each version's section is also its GitHub release notes: `scripts/release-notes.sh <version>` prints them, and the release workflow sets them, with the title "openVJ v<version>", when a release is published. Add a section here before tagging a version.

## [0.1.0] - 2026-11-09

The first release of openVJ: an open-source, layer-based VJ mixer for macOS, modelled on Resolume.

### Mixing

- Clip grid with decks (tabs, like Resolume's), columns and layers. Launch clips and whole columns by click or with the `1`–`9` keys.
- Layers with opacity, 9 blend modes, bypass, solo, clear and a crossfade transition time.
- Layer groups with their own opacity, blend mode, bypass, solo and effect chain.
- A crossfader with A and B sides, linear or full-in-the-middle curve.
- Clip playback modes (Loop, Bounce, Random, Play once & clear, Play once & hold) and beat sync from ¼ to 256 beats.
- GPU mixing in 16-bit float, with live thumbnails in the grid, a Preview and an Output monitor.

### Sources

- 9 built-in GPU generators, and 345 ISF generators from the [Ghost Arcade](https://github.com/riskcapital/ghost-arcade) pack, bundled.
- Images, and video through a bundled, decode-only FFmpeg. Resolume DXV and HAP (all variants) decode on the GPU as compressed textures.
- Cameras, capture cards and virtual cameras, picked at the best format automatically.
- Syphon input from Synesthesia, TouchDesigner, VDMX and any other Syphon server.

### Effects

- 12 built-in GPU effects, including Acuarela (watercolour smoke, rebuilt from Resolume's Wire patch), stacked in chains on clips, layers, groups and the composition, each with bypass and a dry/wet mix.
- A fixed Transform on every clip and layer, like Resolume's: position, scale, 3D rotation and anchor.
- Effects are ISF shader files with multi-pass and feedback support. Drop your own into `~/Documents/openVJ/Effects` (the standard ISF folders are read too) and they reload live while in use.

### Control and sync

- Map any parameter to an audio band or a tempo-synced shape, with depth.
- Audio input analysis with four auto-gained bands.
- BPM, tap tempo and Ableton Link.
- MIDI controllers mapped by importing Resolume MIDI shortcut presets.

### Output

- An output window that goes fullscreen on a projector.
- Output mapping modelled on Resolume's Advanced Output: corner-pinned slices, input selection, masks and a test card. Resolume Advanced Output presets import, with their grid warps.
- Syphon output named "openVJ", on by default, sending only while an app receives it.

### Files

- Save and open compositions as JSON.
- Import Resolume Arena and Avenue `.avc` compositions: decks, clips, playback settings, layers, groups, crossfader, Transforms, effects and their settings, with a media finder for moved files.

### Mac app

- A notarized openVJ.app for Apple Silicon with FFmpeg, Syphon and the Ghost Arcade shaders inside, and its own camera and microphone permissions.
- Settings → About shows the version, credits and Syphon's licence; the full third-party notices are in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

### Known limits

- Video clips play without their audio track.
- No image sequences yet.
- Only DXV and HAP decode on the GPU; other codecs, NotchLC included, decode on the CPU.
- No MIDI learn or controller LED feedback yet.

[0.1.0]: https://github.com/openwave-studio/openvj/releases/tag/v0.1.0
