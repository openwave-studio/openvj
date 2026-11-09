# Spec Delta

## ADDED Requirements

### Requirement: ISF generators
openVJ SHALL play ISF files without an `inputImage` input as generator clips, listed in the browser's Sources tab after the built-in generators and grouped by their ISF categories, like the Sources list in Resolume. A generator's ISF inputs SHALL show as clip parameters with units, ranges and modulation, and SHALL be saved by name with the composition.

#### Scenario: Place a generator
- **WHEN** the VJ double-clicks an ISF generator in the Sources tab
- **THEN** a clip with that generator is placed in the selected slot, plays when launched, and its inputs show as parameters in the Clip panel

#### Scenario: Audio-reactive generator
- **GIVEN** an ISF generator that reads the bass level
- **WHEN** audio input is on and music plays
- **THEN** the generator reacts to the bass

#### Scenario: Missing generator
- **WHEN** a composition uses an ISF generator that isn't installed
- **THEN** the clip keeps its settings, shows as missing, and plays once the file is added

### Requirement: Generator folders
openVJ SHALL load ISF generators from the same folders as effects, with the same hot reload and error display.

#### Scenario: Drop in a generator
- **WHEN** the VJ copies a generator `.fs` file into `~/Documents/openVJ/Effects`
- **THEN** it appears in the Sources tab within about a second

### Requirement: Bundled Ghost Arcade shaders
openVJ SHALL ship Ghost Arcade's licence-clean ISF shaders with their AGPL-3.0 licence and credits, so its generators and effects are available on a fresh install. Shaders derived from Shadertoy, glslsandbox or other non-commercial sources SHALL NOT be bundled.

#### Scenario: Fresh install
- **WHEN** openVJ starts for the first time
- **THEN** the Ghost Arcade generators are listed in the Sources tab and its filters in the Effects tab
