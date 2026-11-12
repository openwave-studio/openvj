# Spec Delta

## ADDED Requirements

### Requirement: One driver per parameter
Each animatable parameter SHALL have at most one driver: Timeline (seconds), Timeline (beats), Audio FFT, Dashboard dial, Clip position or Crossfader, as in Resolume.

#### Scenario: Beats timeline
- **WHEN** a parameter is driven by a 4-beat timeline at 120 BPM
- **THEN** it runs through its range once every 2 seconds

### Requirement: Envelope and range
A driver's phase SHALL pass through an optional envelope and then map into the parameter's in/out range.

#### Scenario: Partial range
- **GIVEN** opacity driven by a timeline with range 20% to 60%
- **WHEN** the phase goes from 0 to 1
- **THEN** opacity moves from 20% to 60%

### Requirement: Settings unfold under the parameter
A parameter's animation settings SHALL unfold under it in the properties panel, as in Resolume.

#### Scenario: Open settings
- **WHEN** the VJ opens a parameter's animation settings
- **THEN** driver, envelope and range show directly under that parameter

### Requirement: Import Resolume animations
Parameter animations in .avc files SHALL import with their driver, duration, envelope and range.

#### Scenario: Timeline in beats
- **WHEN** an .avc parameter uses a timeline with a 4-beat duration
- **THEN** it imports as a 4-beat timeline driver

## MODIFIED Requirements

### Requirement: Map a parameter
Every normalised parameter (clip parameters, layer opacity, master, effect parameters and mix) SHALL be animatable with one driver. Compositions saved with an audio source (band, Level or hits) or tempo shape and a depth SHALL open with the equivalent driver, envelope preset (Sine, Saw, Square, Pulse) and range [base, base + depth].

#### Scenario: Tempo shape
- **WHEN** layer opacity is mapped to Sine over 4 beats with 50% depth
- **THEN** opacity swings by up to half its range once every 4 beats

#### Scenario: Flash on kicks
- **WHEN** an effect's mix is mapped to Bass hits with 100% depth from 0
- **THEN** the effect flashes in on each kick and fades out between them

#### Scenario: Old composition
- **WHEN** a composition saved before this change, with opacity mapped to Bass at 50% depth, is opened
- **THEN** opacity is driven by Bass with range [base, base + 50%]
