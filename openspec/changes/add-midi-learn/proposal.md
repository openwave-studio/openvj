# Proposal

## Why

openVJ's MIDI mappings come only from imported Resolume presets, so a VJ without Resolume can't map a controller at all. Resolume, Lumen and Imaginando VS all have a learn mode: press a key, every mappable control lights up, click one and move a knob. It was already planned for v0.2 and came first in the Lumen and VS comparison (2026-11-12).

## What Changes

- A **MIDI map mode**, turned on with **Cmd+M** (Ctrl+M on Linux and Windows), as in Resolume, or with a **MAP** button next to MIDI in the top bar.
- In map mode, every mappable control gets a cyan outline: faders, buttons, clip slots, column headers, layer bypass/solo/clear, effect parameters, master, crossfader, TAP. A mapped control shows its note or CC as a small label.
- Click a control, then move a knob or press a pad. The mapping is made. Moving another control re-learns it. **Delete** or **Backspace** removes the selected control's mapping.
- Each mapping keeps the existing per-controller rule (from the controller that was moved).
- Faders get Resolume's **min/max** range, and a min above max inverts. Buttons get Resolume's toggle, momentary or press behaviour. Both are set in a small panel shown in map mode for the selected control.
- Learned mappings use the same model as imported ones. They are saved with the composition and can be **exported as a Resolume MIDI shortcut preset** (.xml), so they work in Resolume too.
- Controller LED feedback is out of scope here and gets its own change.

Out of scope: LED feedback, 14-bit CC pairs, NRPN, OSC, and keyboard shortcut learning.

## Capabilities

### Modified Capabilities
- `midi-control`: learning mappings in openVJ, and exporting them as a Resolume preset.

## Touches

- crates/openvj-core: `midi.rs` (learn state, target ids for UI controls, preset export model)
- crates/openvj-resolume: `midi.rs` (write a MidiShortcutPreset .xml)
- crates/openvj-app: `src/app.rs` (map mode, last control moved), `ui/main.slint`, `ui/controls.slint` (map-mode outline and label on faders and buttons), `ui/properties.slint`, `ui/layouts.slint` (Synthesis knobs and macros if #18 has merged)
- Specs: midi-control
- Shared hot spots: `src/app.rs`, `ui/main.slint`, `crates/openvj-resolume/src/lib.rs` (only the export entry point)

## Conflicts

- PR #18 (Synthesis layout) adds knobs and macros that should be mappable. Merge #18 first, then make its knobs mappable here.
- add-midi-clock-in also touches the MIDI input path. Merge it first; it's smaller.
- add-parameter-animation proposes dashboard dials with MIDI mapping. Learning covers MIDI on any parameter now. Dials stay in that change.

## Status (2026-11-16)

Map mode, learning, min/max and behaviour are built (sections 1 and 2). Clip parameters, clip effects and group controls are not mappable yet, because `MidiTarget` has no targets for them. Export (section 3) is blocked: a Resolume shortcut key holds a 32-bit hash of the controller's name (Circuit Tracks MIDI → 0xbb513edb), and none of CRC32, FNV, murmur3, MD5, SHA1 or Java-style string hashes reproduce it. The change stays open until export is solved or dropped.

## Open questions

- Should a learned mapping hold its target by position (layer 2) or follow the selected layer, the way Resolume offers both? The default here is by position, with "selected layer" left to imported presets.
