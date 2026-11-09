# effects Specification

## Purpose

GPU effects stack in chains on layers, groups and the composition, like Resolume's effect chains.

## Requirements

### Requirement: Effect chains
Every layer, group and the composition SHALL have an effect chain. Each effect SHALL have Bypass, reorder, remove and a dry/wet mix.

#### Scenario: Reorder
- **WHEN** the VJ moves Blur above Edges in a layer's chain
- **THEN** the layer is blurred before edge detection

### Requirement: Built-in effects
openVJ SHALL ship Color, Invert, Transform, Mirror, Kaleido, Pixelate, RGB Split, Wave, Posterize, Blur, Edges and Acuarela, added from the browser's Effects tab.

#### Scenario: Add an effect
- **WHEN** the VJ picks Mirror in the Effects tab and clicks a layer
- **THEN** Mirror is appended to that layer's chain

#### Scenario: Acuarela
- **WHEN** Acuarela is added after Edges on a layer
- **THEN** the edges smear and dissipate like watercolour over the following frames

### Requirement: Mappable parameters
Every effect parameter and the mix SHALL be mappable like any other parameter (see parameter-modulation).

#### Scenario: Audio-mapped mix
- **WHEN** an effect's mix is mapped to Bass
- **THEN** the effect fades in with the bass level

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

### Requirement: Effect folders
openVJ SHALL load ISF effects from `~/Documents/openVJ/Effects` (created on first launch), from the standard ISF folders `~/Library/Graphics/ISF` and `/Library/Graphics/ISF` on macOS, and from `OPENVJ_EFFECTS_DIR` when set, including subfolders, like Resolume's Extra Effects folder. ISF files without an `inputImage` input are generators and SHALL NOT be listed as effects.

#### Scenario: Drop in a shader
- **WHEN** the VJ copies `Glitch.fs` into `~/Documents/openVJ/Effects` and starts openVJ
- **THEN** Glitch is listed in the Effects tab after the built-in effects and can be added to any chain

#### Scenario: VDMX shaders
- **WHEN** ISF shaders are installed in `~/Library/Graphics/ISF`
- **THEN** the filters among them appear in the Effects tab without any setup

### Requirement: Hot reload
openVJ SHALL notice new, changed and deleted effect files while running, within about a second, and update the Effects tab and every chain that uses them without a restart. Parameter values SHALL survive a reload.

#### Scenario: Edit a shader live
- **GIVEN** a folder effect is in a layer's chain
- **WHEN** the VJ saves a change to its `.fs` file
- **THEN** the layer shows the new shader on the next frames and the effect's parameter values are kept

### Requirement: Effect errors
An effect file that fails to parse or compile SHALL be listed in the Effects tab, dimmed, with the error, and SHALL NOT be addable. A chain that already holds it SHALL pass video through until the file is fixed.

#### Scenario: Broken shader
- **WHEN** a file in the effects folder has a GLSL error
- **THEN** the Effects tab shows it dimmed with the compiler message, and the rest of the effects work

### Requirement: Effect ids from folders
A folder effect's id SHALL be its `OPENVJ.ID` header key, or else its file name without `.fs`. A folder file SHALL NOT replace a built-in effect, and when two folder files share an id the one from the earlier folder wins; the other is listed as an error.

#### Scenario: Missing on another machine
- **WHEN** a composition that uses a folder effect opens on a machine without that file
- **THEN** the effect keeps its saved settings and passes video through, and starts working once the file is added
