# Tasks

## 1. OSC input and openDJ connection

- [x] 1.1 OSC receiver thread (rosc), off the render thread; events into the app through a channel
- [x] 1.2 Find openDJ over Bonjour (`_osc._udp`, name openDJ), subscribe to `/beat`, `/phrase`, `/deck/*/track`, `/deck/*/cue`, re-subscribe after openDJ restarts
- [x] 1.3 openDJ switch in Settings, connection and phrase shown in the tempo area
- [x] 1.4 Tests with OSC packets from a stand-in openDJ

## 2. Player

- [ ] 2.1 Lengths Phrase and Track; transitions timed to land on the downbeat
- [ ] 2.2 Song uses openDJ's track changes when connected

## 3. Mixer and drivers

- [ ] 3.1 Phrase names as learnable sources in map mode (launch column, clip, Autopilot next)
- [ ] 3.2 Phrase position and Track key drivers in the map menu, saved with the composition

## 4. Docs

- [ ] 4.1 User guide: openDJ setup, Session MIDI through the IAC Driver
- [ ] 4.2 README "What works", archive the change
