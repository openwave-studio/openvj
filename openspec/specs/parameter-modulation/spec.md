# parameter-modulation Specification

## Purpose

Parameters move with the music: any normalised parameter can follow an audio band or a tempo-synced shape.

## Requirements

### Requirement: Map a parameter
Every normalised parameter (clip parameters, layer opacity, master, effect parameters and mix) SHALL have a map ▾ button that maps it to an audio band level (Bass, Low mid, High mid, Treble), the overall Level, audio hits (Bass hits, Low mid hits, High mid hits, Treble hits, Hits) or a tempo shape (Sine, Saw, Square, Pulse over ¼ to 16 beats), with a depth from −100% to +100%. Audio sources are scaled by the composition's Reactivity.

#### Scenario: Tempo shape
- **WHEN** layer opacity is mapped to Sine over 4 beats with 50% depth
- **THEN** opacity swings by up to half its range once every 4 beats

#### Scenario: Flash on kicks
- **WHEN** an effect's mix is mapped to Bass hits with 100% depth from 0
- **THEN** the effect flashes in on each kick and fades out between them

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
