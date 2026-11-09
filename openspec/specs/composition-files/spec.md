# composition-files Specification

## Purpose

Compositions save and open as openVJ's own JSON files.

## Requirements

### Requirement: Save and open
Save and Open SHALL write and read compositions as JSON, storing media paths as they are. Open SHALL also accept Resolume .avc files (see resolume-import).

#### Scenario: Round trip
- **WHEN** a composition is saved and reopened
- **THEN** decks, layers, clips, effects, mappings and MIDI shortcuts are as they were

### Requirement: Open from the command line
Passing a composition file as the first argument SHALL open it; passing a folder SHALL open the file browser there.

#### Scenario: Folder argument
- **WHEN** openVJ is started with ~/Movies/Loops
- **THEN** the Files tab starts in that folder
