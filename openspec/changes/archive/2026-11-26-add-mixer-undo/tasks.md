## 1. Model
- [x] 1.1 `history.rs`: snapshot stack with labels, coalescing by target and gesture, 100-step cap; tests
- [x] 1.2 Restore that keeps runtime playback state and clears removed playing clips with their transition; tests

## 2. Recording
- [x] 2.1 Record at each edit entry point in `app.rs` (clips, effects, layers, groups, columns, decks, mappings, Transform, settings, preset import); skip MIDI/OSC/audio/animation values
- [x] 2.2 A test that walks every edit action and checks undo returns an equal composition

## 3. UI
- [x] 3.1 Cmd/Ctrl+Z and Shift variant, Undo/Redo buttons with hover labels, status bar message

## 4. Finish
- [x] 4.1 README "What works" and Controls table; archive the change
