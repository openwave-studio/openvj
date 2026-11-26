# Proposal

## Why

Two of Resolume's clip settings make a set run itself or play like an instrument, and openVJ has neither. **Autopilot** launches another clip when a clip ends, which suits a long set or an unattended screen. **Piano** trigger style shows a clip only while its MIDI note or key is held, which is close to Imaginando VS's note-triggered layers. Both are stored in .avc files, so importing them improves Resolume compatibility. They came out of the Lumen and VS comparison (2026-11-12).

## What Changes

- **Autopilot**, per clip, in the Clip tab's playback section, as in Resolume: **Off**, **Next clip**, **Previous clip**, **Random clip**, **First clip**, **Last clip**. When the clip finishes a pass (its video length, its beat-sync length, or its duration for stills and generators), openVJ launches that clip in the same layer and deck. Empty slots are skipped. Autopilot needs a play mode with an end: it takes effect with Loop and Bounce at the end of each pass, and with Play once.
- **Trigger style**, per clip, as in Resolume: **Normal** (launch), **Toggle** (a second launch clears the layer), **Piano** (the clip plays while its MIDI note or mouse button is held and the layer clears on release, using the layer's transition time as the release). MIDI velocity scales the clip's opacity in Piano style.
- Both settings are saved with the clip and imported from .avc: Autopilot from each clip's `AutoPilot`/`Target` param, trigger style from the clip's trigger-style choice.

Out of scope: layer- or deck-wide autopilot, Fader Start, Beat Snap, Ignore Column Trigger (later, same section).

## Capabilities

### Modified Capabilities
- `clip-playback`: Autopilot and trigger style.
- `resolume-import`: imports both.

## Touches

- crates/openvj-core: `composition.rs` (two clip fields; end-of-pass event; autopilot target choice; release on note off), `midi.rs` (note off reaches clip launch targets)
- crates/openvj-resolume: `lib.rs` (decode AutoPilot Target and trigger style)
- crates/openvj-app: `ui/properties.slint` (Clip tab), `src/app.rs`
- Specs: clip-playback, resolume-import
- Shared hot spots: `composition.rs`, `src/app.rs`, `crates/openvj-resolume/src/lib.rs`

## Conflicts

- PR #18 (Synthesis layout) touches `src/app.rs`. Merge #18 first.
- add-midi-learn: Piano needs note off on learned clip mappings too. Either order works. If learn lands first, its clip-slot mappings carry note off.

## Open questions

- The AutoPilot `Target` value is base64 of a 64-bit number. The samples have 1 and 7. Could the maintainer save a small .avc with one clip per Autopilot option (Off, Next, Previous, Random, First, Last) and one per trigger style, so the codes can be decoded exactly?
