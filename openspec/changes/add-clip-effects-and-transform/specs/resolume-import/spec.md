# Spec Delta

## MODIFIED Requirements

### Requirement: What is imported
The import SHALL bring over clip media and names, camera and Syphon inputs, clip playback (play mode, BPM sync and beats, duration), column names, layer names, opacity, blend mode, bypass and transition time, layer groups with their effects, crossfader sides and position, BPM, the MIDI shortcut preset, clip and layer Transforms, and clip, layer, group and composition effects openVJ has an equivalent for, with their Bypass, mix and parameter values.

#### Scenario: Layer opacity
- **WHEN** a layer's VideoTrack Opacity differs from its blend mixer's stale copy
- **THEN** the VideoTrack value is used

#### Scenario: Clip Transform
- **WHEN** a clip in the .avc has Scale 140% and Position Y −120
- **THEN** the imported clip has the same Scale and Position Y in its Transform

#### Scenario: Effect settings
- **WHEN** a clip has Edge Detection followed by Acuarela with a raised Strength
- **THEN** the imported clip has Edges then Acuarela, with the same Strength

## ADDED Requirements

### Requirement: Effect parameters match Resolume's
Built-in effects with a Resolume counterpart SHALL use Resolume's parameter names, units and defaults where they can, and each ISF input MAY name the Resolume parameter it imports from. Compositions saved with older parameter names SHALL still load.

#### Scenario: Old composition
- **WHEN** a composition saved before a parameter was renamed is opened
- **THEN** the effect loads with its saved value under the new name
