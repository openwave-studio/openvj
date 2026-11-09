# Spec Delta

## MODIFIED Requirements

### Requirement: Built-in effects
openVJ SHALL ship Color, Invert, Transform, Mirror, Kaleido, Pixelate, RGB Split, Wave, Posterize, Blur, Edges and Acuarela, added from the browser's Effects tab.

#### Scenario: Add an effect
- **WHEN** the VJ picks Mirror in the Effects tab and clicks a layer
- **THEN** Mirror is appended to that layer's chain

#### Scenario: Acuarela
- **WHEN** Acuarela is added after Edges on a layer
- **THEN** the edges smear and dissipate like watercolour over the following frames

## ADDED Requirements

### Requirement: Effects are ISF files
Each effect SHALL be an ISF shader file (a JSON header and GLSL), with openVJ's own header keys for a stable id, display name, the Resolume effect types it replaces, and per-parameter units and wrapping.

#### Scenario: Varda shader
- **WHEN** an ISF file written for Varda, with its own `#version 450` and bindings, is loaded
- **THEN** it compiles and its parameters appear in the effect card

### Requirement: Multi-pass and feedback
Effects SHALL support several passes, sized render targets and persistent buffers that keep the previous frame's result.

#### Scenario: Feedback buffer
- **GIVEN** an effect with a persistent pass
- **WHEN** a frame renders
- **THEN** the pass can read what it wrote on the previous frame

### Requirement: 16-bit mixing
Mixing and effects SHALL run in 16-bit float, while output, preview and thumbnails are 8-bit.

#### Scenario: Smooth feedback
- **WHEN** a feedback effect fades a trail over many frames
- **THEN** it fades smoothly without 8-bit banding

### Requirement: Parameters in real units
Effect parameters SHALL show and save in their real units (degrees, %, px, named choices), saved by name so effect updates don't scramble saved compositions.

#### Scenario: Rotation in degrees
- **WHEN** the VJ sets a Transform rotation
- **THEN** the card shows degrees, and an angle mapped to a tempo shape wraps around instead of jumping
