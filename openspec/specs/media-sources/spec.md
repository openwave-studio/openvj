# media-sources Specification

## Purpose

Clips play images, video files and built-in GPU generators.

## Requirements

### Requirement: Images
Clips SHALL play png, jpg, gif, webp and bmp images.

#### Scenario: Image clip
- **WHEN** a png is placed and launched
- **THEN** it shows on its layer for its duration

### Requirement: Video files
Clips SHALL play common video files (mov, mp4, mkv, webm, avi and others) decoded by FFmpeg, found on PATH, in /opt/homebrew/bin or in OPENVJ_FFMPEG_DIR. Video SHALL seek and play backwards for Bounce and Random.

#### Scenario: No FFmpeg
- **GIVEN** FFmpeg is not installed
- **WHEN** a non-DXV video is launched
- **THEN** loading that clip fails with an "is FFmpeg installed?" error, and images, generators and DXV still work

### Requirement: Built-in generators
openVJ SHALL ship GPU generators (Plasma, Tunnel, Bars, Rings, Noise, Kaleido, Gradient, Strobe, Checker), each with parameters and a speed, with Bars, Rings and Strobe locked to the BPM. New generators are planned for v0.4.

#### Scenario: Tempo-locked generator
- **WHEN** Strobe plays at 120 BPM
- **THEN** it flashes in time with the beat

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

### Requirement: Shadertoy import
openVJ SHALL import a Shadertoy shader from its link or ID, using the VJ's own Shadertoy API key from Settings, or from pasted code without a key. Settings → Media SHALL hold the key. The import SHALL write an ISF file into `~/Documents/openVJ/Effects/Shadertoy/` with the shader's title, author, link and licence in its header, after which it plays like any folder generator. openVJ SHALL NOT bundle Shadertoy shaders, and Settings SHALL say that the import uses the Shadertoy.com API.

#### Scenario: Import by link
- **GIVEN** a Shadertoy API key in Settings
- **WHEN** the VJ pastes `https://www.shadertoy.com/view/XsXXDn` into Import from Shadertoy
- **THEN** the shader appears in the Sources tab under Shadertoy within about a second and can be placed in a slot

#### Scenario: Shader not public to the API
- **WHEN** the API doesn't return the shader
- **THEN** the dialog says the author hasn't made it available to the API and offers Paste code

#### Scenario: Paste code
- **WHEN** the VJ pastes a shader's Image code and its link without an API key
- **THEN** a generator with that code and credit is written and listed, and its channels read black

#### Scenario: Import again
- **WHEN** the VJ imports a shader that was imported before
- **THEN** its file is replaced with the new version

#### Scenario: Unsupported channels
- **WHEN** an imported shader reads a keyboard or cubemap channel
- **THEN** the shader is still imported, those channels read black, and the dialog names them

#### Scenario: Audio-reactive Shadertoy shader
- **GIVEN** an imported shader that reads a music channel
- **WHEN** it plays with audio input on
- **THEN** it reacts to openVJ's audio input as it does to music on Shadertoy

### Requirement: Open on Shadertoy
A clip or effect from an imported Shadertoy shader SHALL show an "Open on Shadertoy" link in its panel that opens the shader's page in the browser.

#### Scenario: Find the original
- **WHEN** the VJ clicks Open on Shadertoy in the panel of an imported shader's clip
- **THEN** the shader's Shadertoy page opens in the default browser

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
