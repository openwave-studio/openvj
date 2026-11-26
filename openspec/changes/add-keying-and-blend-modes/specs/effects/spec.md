## MODIFIED Requirements

### Requirement: Built-in effects
openVJ SHALL ship Color, Invert, Transform, Mirror, Kaleido, Pixelate, RGB Split, Wave, Posterize, Blur, Edges, Acuarela, Luma Key, Chroma Key, Trails, Strobe, Threshold, Tile, Slide, Bloom, Vignette and Infinite Zoom, added from the browser's Effects tab.

#### Scenario: Add an effect
- **WHEN** the VJ picks Mirror in the Effects tab and clicks a layer
- **THEN** Mirror is appended to that layer's chain

#### Scenario: Acuarela
- **WHEN** Acuarela is added after Edges on a layer
- **THEN** the edges smear and dissipate like watercolour over the following frames

## ADDED Requirements

### Requirement: Keying effects
Luma Key SHALL make pixels below a brightness threshold transparent, with softness and invert. Chroma Key SHALL make pixels near a key colour transparent, with tolerance, softness and spill suppression. Both SHALL keep the alpha they produce, so the layer's blend mode shows what is below.

#### Scenario: Green screen
- **GIVEN** a clip of a dancer on green with Chroma Key set to green
- **WHEN** it plays on a layer above a generator
- **THEN** the dancer shows over the generator, without a green fringe

### Requirement: Time effects
Trails SHALL blend each frame over the previous result with a feedback amount, using a persistent buffer. Strobe SHALL flash the image to a colour at a rate set in beats (following the tempo) or in Hz.

#### Scenario: Strobe on the beat
- **GIVEN** Strobe with a rate of 1/4 beat at 128 BPM
- **THEN** it flashes four times per beat, in phase with the beat indicator
