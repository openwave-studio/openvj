## 1. Clock parsing and tempo
- [ ] 1.1 Parse clock, Start, Continue and Stop in `openvj-core/src/midi.rs`; stop the app's MIDI input from dropping them
- [ ] 1.2 `midi_clock.rs`: pulse timestamps to a smoothed BPM and a beat position; tests for steady, drifting and stopped clock
- [ ] 1.3 Drive the app's beat clock from it when CLOCK is on; make LINK and CLOCK exclusive

## 2. UI
- [ ] 2.1 CLOCK button in the top bar, with on, off and waiting states; disable ± and TAP while it's on
- [ ] 2.2 Clock input picker in Settings → MIDI; save the choice in settings
- [ ] 2.3 README "What works" (Tempo), archive the change
