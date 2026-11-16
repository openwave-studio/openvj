# Proposal

## Why

MilkDrop presets are the largest library of audio-reactive visuals there is: thousands of community `.milk` files built on per-frame and per-pixel equations, warp and composite shaders, and feedback on the previous frame. Playing them as clips gives openVJ, and later the Player layout, thousands of ready-made looks. The maintainer chose to do this with [lactea](https://github.com/M0Rf30/lactea) (2026-11-15), a pure-Rust MilkDrop 2/3 engine on wgpu 30 (MIT/Apache-2.0), over projectM (C++, OpenGL only) or a port of our own.

## What Changes

- `.milk` files play as generator clips. openVJ loads them from `~/Documents/openVJ/MilkDrop` (subfolders included, and `OPENVJ_MILKDROP_DIR`), rescanning about once a second like the effects folder.
- The browser gets a MilkDrop tab: a scrolling list of every preset with its subfolder, and an Open folder button (a pack has ~10,000 presets, too many for the Sources tab's cards). A `.milk` file from the Files tab also places as a MilkDrop clip.
- A MilkDrop clip reacts to the audio input (raw samples, so presets see MilkDrop's own bass/mid/treble), follows the clip's speed, and has one parameter, Sensitivity, with the usual modulation. The composition's Reactivity scales it too.
- Clips save the preset by its path inside the MilkDrop folder (or the full path for a file elsewhere). A missing preset shows as missing and plays once the file is back, like a missing ISF generator.
- Each MilkDrop clip runs one lactea engine at the composition size, shared by its layer, preview and thumbnail.
- No presets are bundled: community packs have no clear licence. README points to the projectM preset packs.

Out of scope: MilkDrop's own preset-to-preset blends and the Player playlist (after add-layout-selector lands), user sprites and custom preset textures beyond what lactea loads from the preset's own folder, editing presets.

## Capabilities

### Modified Capabilities
- `media-sources`: MilkDrop presets as generator clips, from a MilkDrop folder.

## Touches

- New dependency: `lactea` 0.2 (pinned) in crates/openvj-render.
- crates/openvj-core: `composition.rs` (new `ClipSource::Milkdrop`, saved format), new `milkdrop.rs` (folder scan).
- crates/openvj-render: `lib.rs` (a MilkDrop draw, engines per clip, PCM input), new `milkdrop.rs`.
- crates/openvj-audio: `input.rs` (hand raw samples to the engine as well as the analyser).
- crates/openvj-app: `app.rs` (Sources tab, clip panel, audio), `browser.rs`.
- README.md "What works", THIRD_PARTY_NOTICES.md (lactea, MIT/Apache; MilkDrop 2 BSD parts).
- Specs: media-sources.
- Shared hot spots: composition.rs, app.rs, openvj-render lib.rs, README.md.

## Conflicts

- add-layout-selector (PR #10, draft): touches app.rs and main.slint. No overlap in logic; whichever merges second rebases. The Player will later list MilkDrop presets as playlist entries.
- add-parameter-animation (no PR yet): touches composition.rs parameter types. MilkDrop's one parameter uses the existing clip params, so it follows whatever that change does to them.

## Open questions

- Is `~/Documents/openVJ/MilkDrop` the right folder, or should `.milk` files go in the Effects folder too?
- Bundling: none for now (licences are unclear). Should openVJ offer a one-click download of the projectM "cream of the crop" pack into the folder later?
- A ~10,000 preset list needs search and favourites. Follow-up?
