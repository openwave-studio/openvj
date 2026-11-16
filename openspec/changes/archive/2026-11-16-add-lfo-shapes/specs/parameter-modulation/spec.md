## ADDED Requirements

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
