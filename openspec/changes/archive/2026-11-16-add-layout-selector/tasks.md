# Tasks

## 1. Layout selector and Mixer (first PR)

- [x] 1.1 Layout icons and selector menu in openDJ's style, cyan accent
- [x] 1.2 Split MainWindow's body so Mixer is one layout; selector at the left of the top bar
- [x] 1.3 Cmd/Ctrl+1/2/3 shortcuts
- [x] 1.4 Remember the last layout in openVJ's settings folder
- [x] 1.5 Player and Synthesis show a simple placeholder until their groups land

## 2. Player model

- [x] 2.1 Presets and playlist types in openvj-core, `.ovjplaylist` read/write with tests
- [x] 2.2 Playlist clock: durations in bars or seconds, LENGTH, STYLE, Shuffle, Repeat, with tests

## 3. Player playback and screen

- [x] 3.1 Engine renders the Player's A/B layers and transition beside the composition
- [x] 3.2 Player screen in openDJ's Automix style: list, Automix button, LENGTH and STYLE pickers, Shuffle, Repeat, Open/Save playlist
- [x] 3.3 "Add to playlist" on a clip; AUTOMIX chip with Stop in other layouts

## 4. Synthesis

- [x] 4.1 Macro model in openvj-core (8 per visual, targets with ranges), saved in compositions and presets, with tests
- [x] 4.2 Synthesis screen: layer strip, preview, macro row, SOURCE / FX / OUTPUT sections, knobs
- [x] 4.3 Assigning macro targets (Map / Done) and drivers on macros
- [x] 4.4 Add to playlist (Save preset waits for the preset library being designed in the library thread)

## 5. Wrap up

- [x] 5.1 README "What works"
- [x] 5.2 Archive the change

## Deferred to later changes

- Videos and images in presets (need media loading for the Player's clips)
- MIDI learn for macros (with add-midi-learn), renaming macros, editing target ranges
