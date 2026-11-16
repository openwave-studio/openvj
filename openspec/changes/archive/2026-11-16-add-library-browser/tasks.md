# Tasks

## 1. Library model (own PR)

- [x] 1.1 `library.rs`: item ids for ISF, MilkDrop, Shadertoy and preset items; status, favorite, intensity, tags, crates, usage
- [x] 1.2 `library.json` read/write in the settings folder, versioned, with tests; missing files keep their entries
- [x] 1.3 Audio-reactive detection from shader inputs, with tests
- [x] 1.4 Presets as `.ovjpreset` files reusing the Player's `Preset`, load/save with tests (in ~/Documents/openVJ/Presets, the proposed default)
- [x] 1.5 First-launch starter set (proposed default: built-ins, bundled effects, one bundled generator per category up to 30)

## 2. Thumbnails (own PR)

- [x] 2.1 Background renderer at 256×144, one per frame at most, paused when over frame budget
- [x] 2.2 Disk cache keyed by content hash; effects on a built-in test image
- [x] 2.3 "Use Preview frame as thumbnail" (button in the grid's selection bar)

## 3. Sources and Effects grids (own PR)

- [x] 3.1 Tile and chip controls in openDJ style (theme tokens only)
- [x] 3.2 Grid with size control and list view; My library default; chips, kind, intensity and audio-reactive filters; MilkDrop tab folded into Kind
- [x] 3.3 Type-anywhere search
- [x] 3.4 Click → Preview; double-click, Enter (or click a slot after) → slot; keys 0–5, F, X, K. Dragging onto a slot waits for drag and drop in Slint
- [x] 3.5 Effects tab on the same grid

## 4. Expanded library (own PR)

- [x] 4.1 Tab/expand opens it over the clip grid; Tab/Esc returns
- [x] 4.2 Sidebar filters, larger grid, info panel with Put in and Add to playlist

## 5. Wrap up

- [x] 5.1 README "What works"
- [x] 5.2 `openspec archive add-library-browser -y`
