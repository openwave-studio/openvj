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

### Requirement: More tempo shapes
The map ▾ menu SHALL offer the tempo shapes Sine, Saw, Ramp down, Triangle, Square, Pulse, Random and Smooth random, over ¼ to 64 beats.

#### Scenario: Random
- **WHEN** a parameter is mapped to Random over 1 beat
- **THEN** it jumps to a new value on every beat and holds it until the next

#### Scenario: Smooth random
- **WHEN** a parameter is mapped to Smooth random over 4 beats
- **THEN** it glides to a new value every 4 beats, without jumps

### Requirement: Phase
Every tempo shape SHALL have a phase offset from 0 to 100% of its cycle.

#### Scenario: Opposite motion
- **GIVEN** two parameters mapped to Sine over 2 beats, one at 0% phase and one at 50%
- **WHEN** the first is at its top
- **THEN** the second is at its bottom

### Requirement: Restart on launch
A tempo-shape mapping on a clip SHALL be able to restart its cycle when the clip is launched instead of following the global beat.

#### Scenario: Retrigger
- **GIVEN** a clip parameter mapped to Saw over 4 beats with Restart on launch
- **WHEN** the clip is launched mid-bar
- **THEN** the saw starts at 0 at that moment
