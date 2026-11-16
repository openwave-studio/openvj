# Design

## Library file, not shader metadata

**Choice:** statuses, favorites, intensity, tags, crates and usage live in one per-user file, `library.json`, in openVJ's settings folder, keyed by a stable item id. Bundled shader files are never written to.

**Ids:** ISF items use `OPENVJ.ID` or the file stem plus the pack (folder) name, the same id rules as the effects folder; MilkDrop uses the preset path relative to its pack folder; presets use their file name. An item whose file disappears keeps its entry (marked missing) so its tags come back if the file returns.

**Why:** bundled files are read-only in the app bundle and replaced on update; MilkDrop `.milk` files have no metadata block; one small JSON file is easy to back up and to read in tests. A few thousand entries is well under 1 MB.

## Statuses

`not_reviewed` (default for anything new), `kept`, `unwanted`. My library = `kept` items plus saved presets (a saved preset is always kept). Items in use in the open composition count as kept, so a composition never references something hidden from the VJ.

## Presets

A saved preset is the Player's `Preset` (a clip source with parameter values, mappings and effects) written as its own `.ovjpreset` JSON file (versioned like composition files) in the presets folder. Saving a preset never changes the original item; "Keep" marks the original kept.

## Thumbnails

Rendered on a background queue at 256×144 into openVJ's cache folder, keyed by a hash of the file's content (and of the preset values for presets), as JPEG. Generators and MilkDrop render 3 s of time with a quiet synthetic audio signal and take the last frame; effects render on a standard built-in test image. At most one thumbnail renders per output frame so a show never stutters, and the queue pauses while the output is above its frame budget. The VJ can store the Preview's current frame as the thumbnail (VDMX's "Set Icon").

## Audio-reactive tag

Derived without rendering: the shader reads openVJ's audio, FFT, beat or BPM inputs (including the Ghost Arcade, Synesthesia and Shadertoy names the ISF shim maps). All MilkDrop presets are audio-reactive.
