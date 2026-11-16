## ADDED Requirements

### Requirement: MilkDrop presets
openVJ SHALL play MilkDrop `.milk` presets as generator clips. A MilkDrop clip SHALL react to the audio input, follow the clip's speed, and have a Sensitivity parameter with modulation. openVJ SHALL NOT bundle MilkDrop presets.

#### Scenario: Audio-reactive preset
- **GIVEN** a MilkDrop clip playing
- **WHEN** audio input is on and music plays
- **THEN** the preset reacts as it does in MilkDrop, more strongly with a higher Sensitivity

#### Scenario: Broken preset
- **WHEN** a preset fails to load
- **THEN** the clip shows black and the clip panel shows the error

### Requirement: MilkDrop folder
openVJ SHALL load MilkDrop presets from `~/Documents/openVJ/MilkDrop` (subfolders included) and `OPENVJ_MILKDROP_DIR`, rescanned about once a second, and list them in the browser's MilkDrop tab with their subfolder and an Open folder button. Clips SHALL save the preset by its path inside the MilkDrop folder, or the full path for a file elsewhere.

#### Scenario: Place a preset
- **GIVEN** a `.milk` file in `~/Documents/openVJ/MilkDrop/Geiss`
- **WHEN** the VJ double-clicks it in the MilkDrop tab
- **THEN** a clip with that preset is placed in the selected slot and plays when launched

#### Scenario: Missing preset
- **WHEN** a composition uses a MilkDrop preset that isn't in the folder
- **THEN** the clip keeps its settings, shows as missing, and plays once the file is added
