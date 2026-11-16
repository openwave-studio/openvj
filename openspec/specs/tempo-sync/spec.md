# tempo-sync Specification

## Purpose

One tempo drives beat-synced clips, generators and modulation, set by hand, by tap or by Ableton Link.

## Requirements

### Requirement: Set the tempo
The VJ SHALL be able to set the BPM with ± and TAP, with a 4-beat indicator.

#### Scenario: Tap
- **WHEN** the VJ taps TAP four times at 0.5 s intervals
- **THEN** the tempo becomes 120 BPM

### Requirement: Ableton Link
LINK SHALL join an Ableton Link session: the beat and tempo follow the session, and tempo changes made in openVJ are sent to it.

#### Scenario: Tempo from Live
- **GIVEN** LINK is on and Ableton Live is in the session
- **WHEN** Live changes tempo to 128
- **THEN** openVJ's tempo becomes 128 and beat-synced clips follow

### Requirement: MIDI clock
CLOCK SHALL make openVJ follow MIDI clock from a MIDI input: the tempo comes from the pulse rate (24 per quarter note), smoothed, and Start resets the beat to the start of a bar. This matches Resolume's external MIDI clock sync.

#### Scenario: Tempo from a groovebox
- **GIVEN** CLOCK is on and a Circuit Tracks sends clock at 124 BPM
- **WHEN** a few beats have passed
- **THEN** openVJ's tempo shows 124 BPM and beat-synced clips follow it

#### Scenario: Start resets the bar
- **GIVEN** CLOCK is on
- **WHEN** the groovebox sends Start
- **THEN** the beat indicator restarts at beat 1

#### Scenario: Clock stops
- **GIVEN** CLOCK is on and following 124 BPM
- **WHEN** no clock arrives for a second
- **THEN** the tempo stays at 124 BPM and the CLOCK button shows that it's waiting

### Requirement: One external tempo source
LINK and CLOCK SHALL NOT both be on: turning one on SHALL turn the other off, and ± and TAP SHALL be disabled while CLOCK is on.

#### Scenario: Switch to clock
- **GIVEN** LINK is on
- **WHEN** the VJ turns CLOCK on
- **THEN** LINK turns off and the tempo follows the MIDI clock
