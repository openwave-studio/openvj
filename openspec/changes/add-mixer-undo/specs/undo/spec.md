## ADDED Requirements

### Requirement: Undo and redo
Cmd+Z (Ctrl+Z on Linux and Windows) and an Undo button SHALL undo the last composition edit, and Cmd+Shift+Z and a Redo button SHALL redo it, as in Resolume. The buttons SHALL show what they will undo or redo on hover, and the status bar SHALL say what was undone. A new edit after an undo SHALL clear the redo steps.

#### Scenario: Undo a move
- **GIVEN** the VJ drags a clip from slot 1 to slot 4
- **WHEN** they press Cmd+Z
- **THEN** the clip is back in slot 1, and Redo moves it to slot 4 again

### Requirement: What is undoable
Undo SHALL cover edits to the composition: clips placed, moved, copied, removed and their settings; effects added, removed, reordered, bypassed and their parameters; layers, groups, columns and decks; mappings and learned MIDI and OSC mappings; Transform; composition settings. It SHALL NOT cover launching, clearing, tempo, tap, transport, or values moved by MIDI, OSC, audio or animation. One mouse drag or one typed value SHALL be one step.

#### Scenario: Launches aren't undone
- **GIVEN** the VJ removes an effect, then launches column 2
- **WHEN** they press Cmd+Z
- **THEN** the effect comes back and column 2 keeps playing

#### Scenario: One drag, one step
- **WHEN** the VJ drags layer 1's opacity from 100% to 20% in one drag, then presses Cmd+Z
- **THEN** opacity is back at 100%

### Requirement: Undo keeps the output running
Undo and redo SHALL NOT restart or cut clips that are playing. A playing clip that an undo removes SHALL clear with its layer's transition.

#### Scenario: Undo a clip drop while it plays
- **GIVEN** the VJ dropped a generator on an empty slot and launched it
- **WHEN** they press Cmd+Z
- **THEN** the slot is empty again and the layer fades out with its transition time

### Requirement: History lifetime
The history SHALL keep the last 100 steps, be cleared by New and Open, and not be saved with the composition.

#### Scenario: Open clears history
- **WHEN** the VJ opens another composition
- **THEN** Undo is disabled until the next edit
