# Proposal

## Why

Shadertoy is the largest public library of GLSL visuals, and VJs already pull shaders from it into VDMX, Magic and Synesthesia. openVJ's ISF engine already does most of what Shadertoy needs (multiple passes, feedback buffers, a spectrum texture, name shims for Synesthesia and Ghost Arcade), so letting a VJ bring a Shadertoy shader in by its link is cheap and adds a huge pool of looks. Shadertoy shaders are CC BY-NC-SA 3.0 unless the author says otherwise, so openVJ can't ship them; it can let each VJ import the ones they want.

## What Changes

- **Import from Shadertoy** in the Sources tab: the VJ pastes a Shadertoy link or ID, openVJ fetches the shader through the Shadertoy API and writes it as an ISF `.fs` file in `~/Documents/openVJ/Effects/Shadertoy/`, named after the shader. Hot reload then lists it like any folder generator. The file's header keeps the title, author, link and licence (`CREDIT`, `DESCRIPTION`, `OPENVJ.SOURCE`).
- **Paste code** in the same dialog, for shaders the API doesn't return (only "Public + API" shaders are visible to it): the VJ pastes the Image tab's code, plus an optional link for the credit. Pasted code is one pass: its channels read black.
- **API key in Settings** (Settings → Media): the VJ's own key from shadertoy.com/myapps. No key ships in the public repo. Without a key, only Paste code is offered. Settings names the "Shadertoy.com API", as its terms require.
- **Shadertoy names in ISF files**: `mainImage`, `iTime`, `iTimeDelta`, `iFrame`, `iResolution`, `iDate`, `iMouse` (zero, as on Shadertoy before a click), `iChannelResolution`, `iChannelTime` and `iSampleRate` work in any ISF file, like the Synesthesia and Ghost Arcade names. So a `.fs` or `.glsl` file with a hand-copied `mainImage` also plays.
- **Channels**: a music, sound or microphone channel reads openVJ's audio as Shadertoy's 512×2 texture (spectrum on row 0, waveform on row 1). Buffers A–D become persistent ISF passes in Shadertoy's order, then the Image pass. A channel that reads the shader's own buffer keeps its previous frame.
- Clips and effects from an imported shader show an **Open on Shadertoy** link in their panel, from the link saved in the file.
- A shader whose `iChannel0` is a video or webcam input imports as an **effect** (its `iChannel0` becomes `inputImage`); every other shader imports as a generator.
- Unsupported channels (keyboard, cubemaps, volumes, textures, video files) are left unbound, reading black, and the import says which ones were dropped. A Sound tab is left out, with a note.

Out of scope: browsing or searching Shadertoy inside openVJ, bundling any Shadertoy shader, Shadertoy textures (needs ISF `IMPORTED` images, a separate change), Common tab code beyond pasting it into every pass, mouse and keyboard interaction, VR shaders.

## Capabilities

### Modified Capabilities
- `media-sources`: import a Shadertoy shader as a generator clip.
- `effects`: Shadertoy names in ISF files; Shadertoy shaders that take a video input import as effects.

## Touches

- New dependency: a small blocking HTTP client (`ureq` with rustls) in crates/openvj-app, used only by the import.
- crates/openvj-core: new `shadertoy.rs` (API JSON → ISF `.fs` text, pure and unit-tested), `effect_folders.rs` (also scan `.glsl` files that define `mainImage`).
- crates/openvj-render: `isf.rs` (Shadertoy preamble and `mainImage` wrapper; 512×2 audio texture next to `isf_spectrum`).
- crates/openvj-app: new `shadertoy_import.rs` (fetch on a worker thread, save, API key in `preferences.json`), new `ui/shadertoy.slint` (dialog and a `Shadertoy` global), `app.rs`, `main.rs`, `ui/main.slint` (Sources tab button), `ui/settings.slint` (API key), `ui/properties.slint` (Open on Shadertoy), `ui/state.slint` (`EffectData.source`), `ui/controls.slint` (disabled button label).
- README.md "What works", THIRD_PARTY_NOTICES.md (Shadertoy.com API mention).
- Specs: media-sources, effects.
- Shared hot spots: app.rs, main.slint, README.md.

## Conflicts

- add-milkdrop-presets (PR #11, draft, proposal only): also adds to the Sources tab and media-sources spec. No shared logic. Suggested order: either first; the second rebases its Sources tab button and spec section.
- add-layout-selector (PR #10): touches main.slint and adds prefs. Merge #10 first, then keep the key in its prefs file rather than a new one. Imported shaders will show up as Player playlist entries like any generator.
- PR #12 (hide the macOS title bar): touches main.slint only at the top bar. Trivial rebase.
- add-parameter-animation (no PR yet): no overlap.

## Decided (the maintainer, 2026-11-15)

- Imports go in the `Shadertoy` subfolder of the Effects folder.
- The clip panel shows an "Open on Shadertoy" link for imported shaders.
- Without an API key, Paste code is the only path for now.
