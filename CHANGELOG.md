# Changelog

All notable changes to openVJ are listed here. Versions follow [Semantic Versioning](https://semver.org); until 1.0, a minor version may change file formats.

Each version's section is also its GitHub release notes: `scripts/release-notes.sh <version>` prints them, and the release workflow sets them, with the title "openVJ v<version>", when a release is published. Add a section here before tagging a version.

## [0.2.0] - 2026-11-27

Four layouts to perform with, a library to sort hundreds of visuals, MilkDrop and Shadertoy, MIDI learn, OSC, undo, and more of Resolume's blend modes, effects and mapping.

### Layouts

- A layout menu at the top left (Cmd+1 to 4) switches between **Mixer**, **Player**, **Synthesis** and **Discover**, as in openDJ. Switching never interrupts the output.
- **Player**: an automix player that plays named playlists of presets (a generator with its values, mappings, Transform and effects), each for some bars, seconds or until the song changes, with openDJ's LENGTH and STYLE transitions, Previous, Next, Shuffle, thumbnails with hover previews and an Edit mode.
- **Synthesis**: perform one visual laid out like a synth, with knobs for the source, each clip effect and the output, 8 macros per visual, META controls on the layer (colour, mirror, limit colours, alpha), lock, undo, default and random per section, and a media overlay with its own browser.
- **Discover**: sort new sources before a show without touching the output. Randomize the unlocked parameters, set intensity, crates and tags, then keep, save as a preset, skip or mark unwanted, all from the keyboard.

### Library and sources

- One library for generators, effects, MilkDrop presets, Shadertoy imports and presets, with kept, not reviewed and unwanted, favorites, intensity, tags and crates.
- The Sources and Effects tabs show it as a thumbnail grid (thumbnails render in the background and are cached), with search, filters and click to preview. **Tab** opens an expanded library over the grid.
- Drag and drop: sources onto slots and layers, effects onto clips, layers and chains, clips between slots (Ctrl copies), and files straight from the Finder.
- **MilkDrop presets** play as generator clips, and on first launch openVJ offers to download projectM's Cream of the Crop pack.
- **Import from Shadertoy**: paste a link or code and it becomes an ISF generator or effect, with its buffers, sound and textures.
- **Layer Router**: a source that shows another layer, a group or the composition, as in Resolume.
- Saved presets (`.ovjpreset`) and playlists (`.ovjplaylist`).

### Mixing and effects

- 10 more blend modes, as in Resolume: Soft Light, Hard Light, Dodge, Burn, Exclusion, Luma Key, Mask, Mask Inverted, Stencil and Silhouette, imported from `.avc` by name.
- 10 more effects: Luma Key, Chroma Key, Trails, Strobe, Threshold, Tile, Slide, Bloom, Vignette and Infinite Zoom.
- Clip **Autopilot** (next, previous, random, first or last clip when a clip ends) and **trigger styles** Normal, Toggle and Piano, with MIDI velocity.
- **Undo and redo** (Cmd+Z, Cmd+Shift+Z) for every edit to the composition, without restarting what is playing.

### Control and sync

- **MIDI learn**: Cmd+M or MAP, click a control, move a knob. Min, max and Fader, Toggle, Press or Hold behaviour.
- **OSC control** with Resolume's addresses (input on port 7000, output on 7001), so TouchOSC layouts made for Resolume work. OSC addresses can be learned too.
- **openDJ link**: follows openDJ's beat, phrases, tracks and key over OSC, shown in the toolbar.
- **MIDI clock**: CLOCK next to LINK follows the tempo and Start/Stop of a groovebox, drum machine or DAW.
- Synesthesia-style audio reactivity: hits, presence and audio clocks per band, a spectrum, Synesthesia's `syn_*` names in ISF files, and a Reactivity control.
- More tempo shapes (Triangle, Ramp down, Random, Smooth random), with phase and restart on launch.

### Output

- One output window per screen, each sent to its own display.
- Edit warp grids in the mapping window (linear or bezier), with outlines on the output while editing.
- Soft edge between overlapping slices for blended projectors, and brightness, contrast and colour per screen and slice, imported from Resolume presets.

### Mac app

- The window has no title bar: the top bar moves it.

### Known limits

- Video clips still play without their audio track, and there are no image sequences yet.
- Learned MIDI mappings can't be exported as a Resolume preset, and there's no controller LED feedback. Clip parameters, clip effects and group controls can't be mapped to MIDI or OSC yet.
- Only DXV and HAP decode on the GPU.

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
