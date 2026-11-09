# Proposal

## Why

Every effect is an ISF file, but only the built-in ones compiled into the binary show up. VJs collect ISF shaders (VDMX, ISF Editor, Ghost Arcade's 200+ pack, Varda's set) and expect to drop them into a folder and use them, the way Resolume picks up FFGL plugins from `Documents/Resolume Arena/Extra Effects`. The "Import Ghost Arcade ISF shaders" work depends on this loader.

## What Changes

- openVJ scans effect folders at launch, with no setup:
  - `~/Documents/openVJ/Effects` (created on first launch, like Resolume's Extra Effects folder),
  - the standard ISF folders on macOS, `~/Library/Graphics/ISF` and `/Library/Graphics/ISF`, where ISF Editor and VDMX install shaders,
  - `OPENVJ_EFFECTS_DIR` if set (for development and tests).
  Subfolders are scanned too. Every `.fs` file that has an `inputImage` input is an effect; ISF generators (no `inputImage`) are skipped until generators land in v0.4.
- Folder effects appear in the browser's Effects tab after the built-ins, sorted by name.
- Hot reload: openVJ checks the folders about once a second. A new, changed or deleted file updates the Effects tab, and effects already in a chain recompile on the next frame and keep their parameter values (they're saved by name). Feedback buffers of a changed effect restart.
- Clear errors: a file whose header or GLSL doesn't compile is listed in the Effects tab, dimmed, with the first line of the compiler error, and the full message goes to stderr. Fixing and saving the file makes it usable without restarting.
- An effect's id is its `OPENVJ.ID` header key, or the file name without `.fs`. Built-in ids can't be taken by a folder file; a clash is shown as an error on that file. Between two folder files with the same id, the first folder in the list above wins.
- A composition that uses a folder effect which isn't installed keeps its settings and passes video through, as unknown effects already do.

Out of scope: custom vertex shaders (`.vs` files next to a `.fs`; such effects are listed with an error for now), extra image inputs and `IMPORTED` images, audio/FFT image inputs, a folder picker in Settings, Windows and Linux standard folders (only the openVJ folder and `OPENVJ_EFFECTS_DIR` there), importing Ghost Arcade's pack itself (its own change).

## Capabilities

### New Capabilities

### Modified Capabilities
- `effects`: effects also load from user and standard ISF folders, reload when the files change, and show compile errors in the browser.

## Impact

The effect library becomes swappable at runtime instead of a fixed list built once. Code that holds effect types (render cache, MIDI mapping, Resolume import) takes a shared handle instead of a `'static` reference.

## Touches

- crates/openvj-core: `effect.rs` (library can be reloaded, folder scan), new `effect_folders.rs`
- crates/openvj-render: `isf.rs` (public GLSL check), `lib.rs` (recompile when an effect changes) — shared hot spot
- crates/openvj-resolume: `midi.rs`, `lib.rs` (library handle) — shared hot spot, a few lines
- crates/openvj-app: `app.rs` (reload Effects tab, start watcher) — shared hot spot; `ui/main.slint` (dimmed error rows in the Effects tab) — shared hot spot, small
- README.md: "What works" and "Writing effects"
- Specs: effects

## Conflicts

- add-isf-effect-engine: not archived yet; this change builds on its spec delta. Archive it first (or in the same PR).
- PR #18 (perf) touches `openvj-render/src/lib.rs` and `app.rs`. Merge #18 first; this PR rebases on it.
- "Import Ghost Arcade ISF shaders" depends on this loader: merge this first.
- "Build the output mapping screen" and "Add Syphon output" will touch `app.rs`/`main.slint`; the edits here are small, whichever lands second rebases.

## Open questions

1. Folder name: `~/Documents/openVJ/Effects` OK? (Resolume uses `Documents/Resolume Arena/Extra Effects`.)
2. Should a folder file be able to replace a built-in effect with the same id (handy for tweaking Acuarela), or should built-ins stay fixed as proposed?
