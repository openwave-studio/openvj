# library Specification

## Purpose
Browse, sort and place every visual openVJ can play (generators, effects, MilkDrop presets, Shadertoy imports and saved presets) from one library that scales to thousands of items, with thumbnails, a small kept set (My library), favorites, intensity, tags and crates.

## Requirements

### Requirement: One library
openVJ SHALL list every ISF generator and effect (bundled and from the user folders), every MilkDrop preset, every Shadertoy import and every saved preset in one library. Each item SHALL show its kind (Generator, Effect, MilkDrop, Preset) and its pack (the bundled pack or folder it comes from).

#### Scenario: New file appears
- **WHEN** the VJ drops a new ISF generator into the user Effects folder
- **THEN** it appears in the library within a few seconds, with status Not reviewed

### Requirement: Review status
Each item SHALL have one status: Not reviewed, Kept or Unwanted. New items SHALL start Not reviewed. Marking an item Unwanted SHALL hide it from My library and Discover and move it to the Unwanted crate, without deleting any file. Items used by the open composition SHALL count as Kept.

#### Scenario: Restore an unwanted item
- **WHEN** the VJ opens the Unwanted crate and chooses Keep on an item
- **THEN** the item shows in My library again with its tags and intensity intact

### Requirement: Favorites, intensity, tags and crates
The VJ SHALL be able to star an item as a favorite, give it an intensity from 0 to 5 (how intense the visual is, from calm to peak), add free tags, and add it to any number of crates they name. openVJ SHALL record how often and when each item was last used. These SHALL be stored per user, outside the shader files, and survive app updates.

#### Scenario: Set intensity from the keyboard
- **WHEN** an item is selected in the library and the VJ presses 4
- **THEN** its intensity becomes 4 and its tile shows four of five intensity bars

### Requirement: Sources grid
The browser's Sources tab SHALL show the library as a grid of thumbnails by default, with a list view as an option and a tile size control. It SHALL show My library by default, with chips for Favorites, Recent, each crate, Discover and Unwanted, filters for kind, intensity and audio-reactive, and a search box matching name, description, credit, categories and tags that takes typing without clicking it first. The Effects tab SHALL use the same grid limited to effects.

#### Scenario: Find a tunnel
- **WHEN** the VJ types "tunnel" with the Sources tab open
- **THEN** the grid shows only My library items matching "tunnel", and the count shows how many matched

### Requirement: Click previews, never outputs
Clicking an item in the library SHALL play it on the Preview monitor only. Double-clicking it, dragging it onto a clip slot, or pressing Enter SHALL put it into the selected clip slot. The output SHALL change only when a slot is launched, as before.

#### Scenario: Audition during a show
- **WHEN** clips are playing to the output and the VJ clicks a library tile
- **THEN** the Preview monitor shows that item and the output is unchanged

### Requirement: Expanded library
Pressing Tab or the browser's expand button SHALL show the library over the clip grid with a filter sidebar (library, kind, pack, crates, category), a larger grid, and an info panel for the selected item with its preview, intensity, favorite, Unwanted, tags, crates, "Put in" the selected slot and "Add to playlist". Tab or Esc SHALL return to the Mixer screen.

#### Scenario: Place from the expanded library
- **WHEN** the VJ selects layer 2 column 4, expands the library, picks an item and presses "Put in Layer 2 · column 4"
- **THEN** the item is in that slot and the Mixer screen is back

### Requirement: Thumbnails
Each item SHALL get a thumbnail rendered in the background and cached, without dropping output frames. The VJ SHALL be able to set the thumbnail to the Preview's current frame.

#### Scenario: First launch with a large pack
- **WHEN** openVJ starts with thousands of items that have no thumbnail yet
- **THEN** tiles show a placeholder, fill in over time, and the output frame rate is unaffected

### Requirement: Audio-reactive tag
openVJ SHALL tag an item Audio-reactive when its shader reads audio, spectrum, beat or tempo inputs, and SHALL treat all MilkDrop presets as audio-reactive.

#### Scenario: Filter reactive visuals
- **WHEN** the VJ turns on the Audio-reactive filter
- **THEN** only items tagged Audio-reactive are shown
