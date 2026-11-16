# Proposal

## Why

openVJ now ships 345 Ghost Arcade generators and 19 effects, reads a user ISF folder, plays MilkDrop presets (the "cream of the crop" pack alone is ~9,800) and imports Shadertoy shaders. The Sources panel is a three-column list of text cards with separate tabs per kind; it does not scale past a few dozen items, and ISF CATEGORIES are too thin to filter on (112 of the 345 bundled generators only say "Generator").

The maintainer wants a clean library that shows **only what the VJ has chosen to keep**, with thousands of candidates sorted in a separate Discover layout (change `add-discover-layout`). Research and mockups: the "Browsing hundreds of visuals and FX" doc and the "openVJ library browser mockups" canvas (2026-11-15/16).

## What Changes

- **One library** of everything that can go in a clip slot or on a layer: ISF generators and effects (bundled and user folder), MilkDrop presets, Shadertoy imports, and saved **presets** (a source with its parameter values, mappings and effects; the same Preset the Player plays).
- Each item has a **status**: *Not reviewed*, *Kept* (in My library) or *Unwanted*. Nothing is ever deleted by the library; Unwanted is a crate the VJ can restore from.
- Per item the VJ can set: **favorite** (star), **intensity 0–5** (how intense the visual is, calm to peak; not a quality rating), **tags** and **crates** (VJ-made groupings like "Peak hour"). openVJ records use count and last used.
- **Sources panel** in Mixer becomes a **thumbnail grid** (list view as an option) showing **My library** by default, with a search box (type anywhere), chips for Favorites, Recent, each crate, Discover and Unwanted, an intensity filter, and a Kind filter replacing the separate MilkDrop tab. Effects tab gets the same grid with Kind = Effect.
- **Click** a tile → it plays on the **Preview** monitor only. **Double-click, drag onto a slot, or Enter** → it goes into a clip slot. The output changes only when a slot is launched, as today.
- **Expanded library** (Tab or an expand button): the same library over the clip grid, with a filter sidebar (library, kind, pack, crates, category), a larger grid and an info panel (preview, intensity, star, unwanted, tags, crates, "Put in Layer X · column Y", "Add to playlist"). Tab or Esc returns.
- **Thumbnails** rendered once in the background at small size and cached by file content; MilkDrop and generators after a few seconds of time, effects on a standard test image. The VJ can re-pick the thumbnail frame.
- **Audio-reactive** tag derived automatically (the shader reads audio, FFT or beat inputs).

Out of scope: smart crates (saved filters), similarity / "more like this", colour filters (the maintainer: low priority), auto motion / GPU-cost / strobe analysis, animated hover previews, video files in the library (Files tab stays as is), cloud sync.

## Capabilities

### New Capabilities
- `library`: the library of sources, effects and presets; statuses, favorites, intensity, tags, crates; the Sources and Effects grids; the expanded library.

### Modified Capabilities
- None. `media-sources` and `effects` keep how items load and render; this change only adds how they are found.

## Impact

A per-user library file and a thumbnail cache in openVJ's settings and cache folders. UI work in the browser panel. No change to composition files: slots keep referring to sources by name/path as today.

## Touches

- crates/openvj-core: new `library.rs` (item ids, statuses, favorites, intensity, tags, crates, usage; `library.json` read/write; preset files), reads from `effect_folders.rs`, `milkdrop.rs`, `shadertoy.rs`, `isf.rs` (audio-reactive detection)
- crates/openvj-render: background thumbnail rendering at small size
- crates/openvj-app: `ui/main.slint` (browser panel: grid, chips, search, expanded view), new `ui/library.slint`, `ui/controls.slint` (tile, chip), `src/app.rs` (library model, preview on click, placing into slots)
- Specs: new `library`
- Shared hot spots: `ui/main.slint`, `src/app.rs`, `crates/openvj-render/src/lib.rs`

## Conflicts

- **PR #14 (Player layout)** touches `ui/main.slint`, `src/app.rs` and adds `playlist.rs` with the `Preset` type. This change reuses that `Preset` for saved presets. Suggested order: #14 merges first, then this change builds on it.
- **add-layout-selector** (Synthesis, task group 4) saves presets too. Both save through the library's preset files described here; whichever lands second uses the other's code.
- **add-discover-layout** depends on this change (statuses, tags, crates, presets). Merge this first.
- **add-parameter-animation**: no file overlap known.

## Open questions

1. **First launch:** with everything *Not reviewed*, My library starts empty. Proposed default: a starter set is Kept on first launch (the sources in the demo composition plus ~30 picked from Ghost Arcade, one per category), and an empty-library hint points to Discover. OK, or would you rather My library start with everything bundled and Discover only for MilkDrop/imports?
2. **Where saved presets live:** proposed `~/Documents/openVJ/Presets/*.ovjpreset` (one file each, next to the Effects folder), so they can be shared and backed up. OK?
3. **Intensity on the tile:** five small bars (mockup) or a single number?
