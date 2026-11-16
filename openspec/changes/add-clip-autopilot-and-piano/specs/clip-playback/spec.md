## ADDED Requirements

### Requirement: Autopilot
Each clip SHALL have an Autopilot setting (Off, Next clip, Previous clip, Random clip, First clip, Last clip) that launches that clip in the same layer when the clip finishes a pass, skipping empty slots. This matches Resolume's clip Autopilot.

#### Scenario: Next clip
- **GIVEN** a layer with clips in columns 1, 2 and 4, and the clip in column 2 set to Autopilot Next clip
- **WHEN** the clip in column 2 finishes a pass
- **THEN** the clip in column 4 launches

#### Scenario: Random clip
- **GIVEN** a clip set to Autopilot Random clip
- **WHEN** it finishes a pass
- **THEN** another clip in its layer launches, picked at random

### Requirement: Trigger style
Each clip SHALL have a trigger style: Normal, Toggle (launching it again clears its layer) or Piano (it plays while its note or mouse button is held, and its layer clears on release). This matches Resolume's clip Trigger Style.

#### Scenario: Piano from MIDI
- **GIVEN** a clip in Piano style mapped to a note
- **WHEN** the note is held for two beats and released
- **THEN** the clip plays for those two beats and its layer then clears using the layer's transition time

#### Scenario: Velocity
- **GIVEN** a clip in Piano style
- **WHEN** its note arrives with velocity 64
- **THEN** the clip plays at about half opacity
