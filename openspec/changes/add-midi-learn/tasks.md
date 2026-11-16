## 1. Model
- [x] 1.1 Learn state in core: selected target, last control moved, replace and remove; tests
- [x] 1.2 Ids for every mappable UI control, matching existing `MidiTarget`s (add targets for crossfader and clip slots if missing)

## 2. Map mode UI
- [x] 2.1 Cmd/Ctrl+M and the MAP button; outline and note/CC label on faders, buttons, slots and effect parameters
- [x] 2.2 Min/max and button-behaviour panel for the selected control
- [ ] 2.3 Synthesis knobs and macros (after PR #18)

## 3. Export
- [ ] 3.1 Write MidiShortcutPreset .xml from the mappings; round-trip test through the existing importer (blocked: Resolume keys each shortcut with a 32-bit hash of the controller name, and the hash function is unknown, so exported presets would not match the controller in Resolume)
- [ ] 3.2 Export button in the Composition tab; README "What works" and Known limits; archive the change
