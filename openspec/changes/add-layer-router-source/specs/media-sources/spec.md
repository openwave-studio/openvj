## ADDED Requirements

### Requirement: Layer Router
The Sources tab SHALL offer a Layer Router source whose Input is a layer, a group or the composition, and whose clip shows that input's frame. This matches Resolume's Layer Router and Group Router sources.

#### Scenario: Re-effect a layer
- **GIVEN** layer 1 plays a video and a Layer Router on layer 3 has Input layer 1
- **WHEN** an effect is added to the router clip
- **THEN** layer 3 shows layer 1's video with that effect, and layer 1 is unchanged

### Requirement: Feedback through the previous frame
A Layer Router whose input is its own layer, a layer above it, or the composition SHALL show that input's previous frame, so it can feed back into the mix.

#### Scenario: Trails
- **GIVEN** a Layer Router with Input Composition on the top layer, scaled to 98% at 90% opacity
- **WHEN** a moving generator plays below it
- **THEN** the output shows trails that shrink toward the centre and fade out

#### Scenario: Unused
- **WHEN** no Layer Router clip is playing
- **THEN** no extra frames are kept on the GPU
