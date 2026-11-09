# output-mapping Specification

## Purpose

Projection mapping follows Resolume's Advanced Output: screens, slices, masks and warps, imported from Resolume presets. Only the model and import exist so far; there is no mapping UI or rendering yet.

## Requirements

### Requirement: Advanced Output model
The output setup SHALL model screens with slices (input and output rectangles or polygons), masks with bezier shapes, and warps: corner-pin, linear grid and bezier grid, matching Resolume's Slice Warper (bezier grid then homography).

#### Scenario: Bezier grid
- **GIVEN** a slice with a 4×4 PM_BEZIER warp grid
- **WHEN** it is evaluated
- **THEN** the grid acts as one bicubic bezier patch, as in Resolume

### Requirement: Import Advanced Output presets
Resolume Advanced Output presets and AdvancedOutput.xml (Arena 5 to 7) SHALL import into the model.

#### Scenario: Inverted masks
- **GIVEN** a screen with several masks with Invert on (Resolume's default)
- **WHEN** it is imported
- **THEN** the masks show their insides, combined as a union

### Requirement: Inspect a preset
`cargo run -p openvj-resolume --example output_info -- preset.xml` SHALL print what a preset imports as.

#### Scenario: CLI inspect
- **WHEN** a developer runs output_info on a preset
- **THEN** it lists screens, slices, warps and masks
