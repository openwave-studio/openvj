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
