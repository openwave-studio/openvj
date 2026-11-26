# Proposal

## Why

The Lucinate comparison (2026-11-25) ranked gated envelopes second, and the maintainer wants them triggered by MIDI notes and beats. Today a parameter can follow audio or a looping tempo shape, but nothing makes a hit: a pad or a kick that opens a flash, a zoom punch or a strobe that then decays. With openDJ running next to openVJ, the Session's MIDI notes (through the IAC Driver) and openDJ's downbeats and phrases become the triggers.

## What Changes

- A new driver in every parameter's map menu: **Envelope**, with Attack, Decay, Sustain and Release (ADSR; AR and AD as presets), and a trigger:
  - **MIDI note**: a learned note (any channel or one), velocity sets the peak; Sustain holds while the note is held, as Lucinate's gated EnvGen and openVJ's Piano trigger do.
  - **Beat**: every beat, every downbeat, or every 2, 4, 8 or 16 bars.
  - **Audio hits**: Bass, Low mid, High mid, Treble or any hit.
  - **openDJ** (when add-opendj-sync is in): phrase start or a hot cue.
- Times in milliseconds or in beats (1/16 to 4 beats), so an envelope can follow the tempo.
- Depth works as for the other drivers; the bar shows the live value.

Out of scope: several drivers on one parameter (standing decision: one driver), envelope curve editing (add-parameter-animation), Chance and Sample & hold shapes (a separate small change).

## Capabilities

### Modified Capabilities
- `parameter-modulation`: an Envelope driver with MIDI note, beat, audio hit and openDJ triggers.

## Touches

- crates/openvj-core: `modulation.rs` (envelope state per mapping, triggers), `midi.rs` (note on/off routed to envelopes)
- crates/openvj-app: `src/app.rs`, `ui/properties.slint` and `ui/controls.slint` (envelope settings under the map menu), Synthesis macros' map menu
- Specs: parameter-modulation
- Shared hot spots: `src/app.rs`, `composition.rs` (saved envelope settings)

## Conflicts

- add-opendj-sync (proposed alongside): its phrase and cue triggers. Either order works; the openDJ triggers stay hidden until both are in.
- add-parameter-animation (not started): its Resolume envelopes are curve shapes along a looping phase. These gated envelopes become one more driver there.
- add-midi-learn: note learning reuses map mode's last-note capture.

## Open questions

- Should a MIDI note envelope also listen to the Piano-triggered clip's note, so one pad both launches a clip and punches a parameter? Proposed: yes, any mapping can share a note.
