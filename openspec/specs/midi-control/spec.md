# midi-control Specification

## Purpose

MIDI controllers drive openVJ through mappings imported from Resolume MIDI shortcut presets.

## Requirements

### Requirement: MIDI input
openVJ SHALL listen to every MIDI input when MIDI is on, pick up controllers plugged in while it runs, and show the last message received in the Composition tab.

#### Scenario: Hot-plug
- **WHEN** a controller is plugged in while openVJ runs
- **THEN** its messages are received without a restart

### Requirement: Import Resolume presets
Importing an .avc SHALL load the MIDI preset it was saved with (looked up next to the .avc and in ~/Documents/Resolume Arena/Shortcuts/MIDI/), and Import Resolume preset… SHALL load any preset .xml into the current composition.

#### Scenario: Preset with the set
- **WHEN** an .avc saved with the "Circuit Tracks" preset is imported
- **THEN** that preset's mappings are active

### Requirement: Supported shortcuts
Mappings SHALL cover clip and column launch, next/previous clip, column and deck, clear layer and clear all, layer opacity, bypass, solo, select, blend modes, master, effect parameters, mix and bypass, tap, resync and tempo, keeping Resolume's min/max range and toggle, momentary or press behaviour.

#### Scenario: Fader range
- **GIVEN** a preset maps a CC to layer opacity with range 20–80%
- **WHEN** the CC moves from 0 to 127
- **THEN** opacity moves from 20% to 80%

### Requirement: Per-controller mappings
Each mapping SHALL keep the controller it was made for, and while that controller is connected, the same note or CC from another controller SHALL be ignored.

#### Scenario: Two controllers
- **GIVEN** a preset maps CC 12 on Circuit Tracks
- **WHEN** a different controller sends CC 12
- **THEN** nothing changes

### Requirement: Saved with the composition
MIDI mappings SHALL be saved with the composition and carry over to New.

#### Scenario: New composition
- **WHEN** the VJ chooses New
- **THEN** the MIDI mappings stay
