# Proposal

## Why

The maintainer's Novation Circuit Tracks sends MIDI clock but doesn't speak Ableton Link. Today openVJ's tempo comes only from ±, TAP or Link, so a set driven by the Circuit drifts unless someone taps along. Resolume and Imaginando VS both follow an external MIDI clock. This came out of the Lumen and VS comparison (2026-11-12) as the smallest high-value item.

## What Changes

- A **CLOCK** button next to LINK in the top bar. When it's on, openVJ follows MIDI clock (24 pulses per quarter note) from the first MIDI input sending it. The BPM shows the measured tempo, smoothed over about 2 beats so it doesn't jitter.
- Transport messages are followed. **Start** (0xFA) resets the beat to bar 1. **Continue** (0xFB) carries on. **Stop** (0xFC) holds the beat where it is. Song Position Pointer is ignored.
- While CLOCK is on, ± and TAP are greyed out. LINK and CLOCK can't both drive the tempo: turning one on turns the other off.
- If clock stops arriving for 1 second, the tempo holds at the last value and the button shows that it's waiting.
- Settings → MIDI can name the clock input; the default is "Any".
- The choice is saved in the app's settings, not in the composition, because it depends on the rig.

Out of scope: sending MIDI clock out, MTC/timecode, and sending tempo from openVJ to the Circuit.

## Capabilities

### Modified Capabilities
- `tempo-sync`: MIDI clock as a third tempo source.

## Touches

- crates/openvj-core: `midi.rs` (parse 0xF8/0xFA/0xFB/0xFC), new `midi_clock.rs` (pulse timing to BPM and beat; unit-tested)
- crates/openvj-app: `src/midi.rs` (stop ignoring clock messages), `src/app.rs` (tempo source, beat clock sync), `ui/main.slint` (CLOCK button), `ui/settings.slint` (clock input)
- Specs: tempo-sync
- Shared hot spots: `src/app.rs`, `ui/main.slint`

## Conflicts

- PR #18 (Synthesis layout) edits `ui/main.slint` and `src/app.rs`. The CLOCK button is a few lines in the top bar. Merge #18 first.

## Open questions

- Should Stop also pause beat-synced clips, or only hold the beat? The default here is to hold only the beat.
