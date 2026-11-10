# resolume-import Specification

## Purpose

Resolume Arena and Avenue compositions (.avc) open in openVJ, so a VJ can move an existing set over. Resolume compatibility is a top project goal.

## Requirements

### Requirement: Open .avc files
Open… SHALL read Resolume Arena and Avenue .avc files as a new composition. Saving SHALL write openVJ JSON and never change the .avc.

#### Scenario: Import a set
- **WHEN** the VJ opens an .avc
- **THEN** it opens as a new, unsaved openVJ composition and the .avc is untouched

### Requirement: Decks become tabs
Every Resolume deck with clips SHALL become a deck tab, and the deck open when the file was saved SHALL open first.

#### Scenario: Several decks
- **WHEN** an .avc with three non-empty decks is imported
- **THEN** three deck tabs appear and the saved deck is open

### Requirement: What is imported
The import SHALL bring over clip media and names, camera and Syphon inputs, clip playback (play mode, BPM sync and beats, duration), column names, layer names, opacity, blend mode, bypass and transition time, layer groups with their effects, crossfader sides and position, BPM, the MIDI shortcut preset, clip and layer Transforms, and clip, layer, group and composition effects openVJ has an equivalent for, with their Bypass, mix and parameter values.

#### Scenario: Layer opacity
- **WHEN** a layer's VideoTrack Opacity differs from its blend mixer's stale copy
- **THEN** the VideoTrack value is used

#### Scenario: Clip Transform
- **WHEN** a clip in the .avc has Scale 140% and Position Y −120
- **THEN** the imported clip has the same Scale and Position Y in its Transform

#### Scenario: Effect settings
- **WHEN** a clip has Edge Detection followed by Acuarela with a raised Strength
- **THEN** the imported clip has Edges then Acuarela, with the same Strength

### Requirement: Report what is dropped
Anything without an openVJ equivalent SHALL be skipped and listed in the status bar and on stderr.

#### Scenario: Wire patch
- **WHEN** an .avc uses a Wire effect openVJ doesn't have
- **THEN** the effect is skipped and named in the import report

### Requirement: Find media
Because Resolume stores absolute paths, missing media SHALL be looked up next to the .avc, and Find media SHALL search a chosen folder for the rest by file name.

#### Scenario: Moved set
- **GIVEN** a set copied from another Mac with its media folder
- **WHEN** the VJ runs Find media on that folder
- **THEN** clips are relinked by file name

### Requirement: Inspect a file
`cargo run -p openvj-resolume --example avc_info -- set.avc` SHALL print what a file would import as.

#### Scenario: CLI inspect
- **WHEN** a developer runs avc_info on an .avc
- **THEN** it lists decks, layers, clips and dropped items

### Requirement: Effect parameters match Resolume's
Built-in effects with a Resolume counterpart SHALL use Resolume's parameter names, units and defaults where they can, and each ISF input MAY name the Resolume parameter it imports from. Compositions saved with older parameter names SHALL still load.

#### Scenario: Old composition
- **WHEN** a composition saved before a parameter was renamed is opened
- **THEN** the effect loads with its saved value under the new name
