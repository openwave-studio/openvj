# Proposal

## Why

Resolume exposes every control over OSC, so TouchOSC and Lemur layouts, Max/MSP, Ableton, TouchDesigner and show-control software can drive it and see its state. openVJ only listens to MIDI. add-opendj-sync (PR #35) adds an OSC receiver, but only for openDJ's beat and phrase events, and leaves "controlling openVJ from OSC (Resolume's OSC addresses)" for later. This is that change. It was #3 in the top 5 of the Resolume 7 gap analysis (2026-11-26).

## What Changes

- **OSC input and output** in Settings → Control, off by default, with Resolume's default ports: input on UDP **7000**, output to a host and port (default this Mac, **7001**). The Composition tab shows the last OSC message received, like it does for MIDI.
- **Resolume's address space** for the controls openVJ has, so a TouchOSC layout or script made for Resolume works unchanged. Values are 0–1 floats for faders and 1/0 for buttons, as in Resolume. Covered: `/composition/master`, `/composition/crossfader/phase`, `/composition/tempocontroller/tempo`, `…/tempotap`, `…/resync`, `/composition/columns/N/connect`, `/composition/layers/N/clips/M/connect`, `/composition/layers/N/clear`, `/composition/layers/N/video/opacity`, `…/bypassed`, `…/solo`, `…/select`, `/composition/layers/N/video/effects/<effect>/effect/<param>` and its `…/bypassed` and `…/opacity` (mix), the same for groups (`/composition/groups/N/…`) and the composition's own effects, `/composition/decks/N/select`, `/composition/disconnectall`, and the `/composition/selectedlayer/…` and `/composition/selectedclip/…` forms. Sending `?` as the argument asks for the current value, which is sent back.
- **OSC output** sends each of those addresses when its value changes (Resolume's "Output all OSC messages"), including which clip is playing in each layer, so a controller app's faders and buttons follow openVJ.
- **OSC learn**: in map mode (Cmd+M), an incoming OSC address can be learned onto any mappable control, like a MIDI CC, for apps that send their own addresses (TouchOSC's `/1/fader1`). Learned OSC mappings are saved with the composition.
- openVJ announces its OSC input over Bonjour (`_osc._udp`), so TouchOSC finds it by name.

Out of scope: OSCQuery, wildcard addresses, clip and clip-effect parameters (they aren't MIDI-mappable yet either), Resolume OSC shortcut preset import, the REST API, and timecode.

## Capabilities

### New Capabilities
- `osc-control`: OSC input and output with Resolume's addresses, and OSC learn.

## Touches

- crates/openvj-core: new `osc.rs` (address parsing to the same targets as `midi.rs`), `midi.rs` (learned mappings take an OSC address as a source)
- crates/openvj-app: `src/osc.rs` (from add-opendj-sync: one UDP receiver for both; adds a sender), `src/app.rs` (apply messages, send changes), settings UI, `ui/properties.slint` (last message)
- Specs: new osc-control; midi-control (map mode learns OSC too)
- Shared hot spots: `src/app.rs`, `composition.rs` (learned OSC mappings), README.md

## Conflicts

- add-opendj-sync (PR #35) creates `src/osc.rs` and the `rosc` dependency. Merge it first and build on its receiver; openDJ's messages and control messages share the input port.
- add-midi-learn (open) owns map mode and the target ids. OSC learn reuses both.
- add-keying-and-blend-modes and add-mixer-undo (this PR): undo should not record OSC-driven value changes, like MIDI. Build order: blend modes, OSC, undo.

## Decisions

- Ports are Resolume's: 7000 in, 7001 out, so existing layouts work (the maintainer, 2026-11-26). openDJ keeps its own port 9000 for what it sends.

## Open questions

- Should OSC output send every change all the time, or only addresses that a client asked for with `?` (less traffic on a busy network)? Proposed: every change, as Resolume does, with a switch.
- Is a bundled TouchOSC layout for openVJ worth making, or is Resolume-compatible enough?
