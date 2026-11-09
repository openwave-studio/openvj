# parameter-modulation Specification

## Purpose

Parameters move with the music: any normalised parameter can follow an audio band or a tempo-synced shape.

## Requirements

### Requirement: Map a parameter
Every normalised parameter (clip parameters, layer opacity, master, effect parameters and mix) SHALL have a map ▾ button that maps it to an audio band (Bass, Low mid, High mid, Treble) or a tempo shape (Sine, Saw, Square, Pulse over ¼ to 16 beats), with a depth from −100% to +100%.

#### Scenario: Tempo shape
- **WHEN** layer opacity is mapped to Sine over 4 beats with 50% depth
- **THEN** opacity swings by up to half its range once every 4 beats

### Requirement: Show base and live value
A mapped parameter's bar SHALL show its base value and a live marker for the modulated value.

#### Scenario: Live marker
- **WHEN** a mapped parameter is moving
- **THEN** its bar shows the base value and the current value separately

### Requirement: Saved with the composition
Mappings SHALL be saved and loaded with the composition.

#### Scenario: Reopen
- **WHEN** a composition with mappings is saved and reopened
- **THEN** the same parameters are mapped the same way
