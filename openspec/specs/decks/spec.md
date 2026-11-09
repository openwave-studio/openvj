# decks Specification

## Purpose

Decks let one composition hold several sets of clips, shown as tabs under the grid like Resolume's decks, while sharing layers and effects.

## Requirements

### Requirement: Decks share layers
Each deck SHALL have its own columns and clips, and all decks SHALL share the composition's layers, layer settings and effects.

#### Scenario: Switch deck
- **WHEN** the VJ opens another deck tab
- **THEN** the grid shows that deck's columns and clips on the same layers

### Requirement: Switching keeps playback
Switching decks SHALL NOT stop or change what is playing.

#### Scenario: Clip keeps playing
- **GIVEN** a clip from deck A is playing on layer 1
- **WHEN** the VJ switches to deck B
- **THEN** the clip keeps playing until something on layer 1 is launched or cleared

### Requirement: Manage decks
The VJ SHALL be able to add a deck (+ Deck), rename one (double-click its tab) and remove the open deck (×).

#### Scenario: Add and rename
- **WHEN** the VJ clicks + Deck and double-clicks the new tab
- **THEN** a new empty deck opens and its name can be edited

### Requirement: Deck shortcuts
Next/previous deck SHALL be available as keyboard and MIDI shortcuts.

#### Scenario: MIDI next deck
- **GIVEN** an imported Resolume preset maps a button to next deck
- **WHEN** that button is pressed
- **THEN** the next deck tab opens
