## ADDED Requirements

### Requirement: MIDI map mode
Cmd+M (Ctrl+M on Linux and Windows) or the MAP button SHALL toggle a map mode in which every mappable control is outlined and shows its current note or CC. This matches Resolume's MIDI shortcut edit mode.

#### Scenario: Enter map mode
- **WHEN** the VJ presses Cmd+M
- **THEN** every fader, button, clip slot and effect parameter that can be mapped is outlined in cyan

### Requirement: Learn a mapping
In map mode, clicking a control and then moving a knob or pressing a pad SHALL map that MIDI control to it, for the controller that sent it, replacing any earlier mapping of that control.

#### Scenario: Map a fader
- **GIVEN** map mode is on and layer 2's opacity is clicked
- **WHEN** the VJ turns CC 21 on the Circuit Tracks
- **THEN** CC 21 moves layer 2's opacity, after leaving map mode too

#### Scenario: Remove a mapping
- **GIVEN** map mode is on and a mapped control is selected
- **WHEN** the VJ presses Delete
- **THEN** that control's mapping is removed

### Requirement: Range and button behaviour
A learned fader mapping SHALL have a min and max (min above max inverts), and a learned button mapping SHALL be toggle, momentary or press, as in Resolume.

#### Scenario: Inverted fader
- **GIVEN** a CC is mapped to opacity with min 100% and max 0%
- **WHEN** the CC goes to 127
- **THEN** opacity goes to 0%

### Requirement: Export a Resolume preset
The VJ SHALL be able to export the composition's MIDI mappings as a Resolume MIDI shortcut preset (.xml) that imports back into openVJ unchanged.

#### Scenario: Round trip
- **WHEN** learned mappings are exported and the file is imported into a new composition
- **THEN** the same controls are mapped the same way
