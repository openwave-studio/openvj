## 1. Clock parsing and tempo
- [x] 1.1 Parse clock, Start, Continue and Stop in `openvj-core/src/midi.rs`; stop the app's MIDI input from dropping them
- [x] 1.2 `midi_clock.rs`: pulse timestamps to a smoothed BPM and a beat position; tests for steady, drifting and stopped clock
- [x] 1.3 Drive the app's beat clock from it when CLOCK is on; make LINK and CLOCK exclusive

## 2. UI
- [x] 2.1 CLOCK button in the top bar, with on, off and waiting states; disable ± and TAP while it's on
- [x] 2.2 Follow the first input sending clock (named in the status bar); save CLOCK on/off in preferences
- [x] 2.3 README "What works" (Tempo), archive the change
