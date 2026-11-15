## ADDED Requirements

### Requirement: Shadertoy import
openVJ SHALL import a Shadertoy shader from its link or ID, using the VJ's own Shadertoy API key from Settings, or from pasted code without a key. The import SHALL write an ISF file into `~/Documents/openVJ/Effects/Shadertoy/` with the shader's title, author, link and licence in its header, after which it plays like any folder generator. openVJ SHALL NOT bundle Shadertoy shaders, and Settings SHALL say that the import uses the Shadertoy.com API.

#### Scenario: Import by link
- **GIVEN** a Shadertoy API key in Settings
- **WHEN** the VJ pastes `https://www.shadertoy.com/view/XsXXDn` into Import from Shadertoy
- **THEN** the shader appears in the Sources tab under Shadertoy within about a second and can be placed in a slot

#### Scenario: Shader not public to the API
- **WHEN** the API doesn't return the shader
- **THEN** the dialog says the author hasn't made it available to the API and offers Paste code

#### Scenario: Paste code
- **WHEN** the VJ pastes a shader's Image code and its link without an API key
- **THEN** a generator with that code and credit is written and listed

#### Scenario: Unsupported channels
- **WHEN** an imported shader reads a keyboard or cubemap channel
- **THEN** the shader is still imported, those channels read black, and the dialog names them

#### Scenario: Audio-reactive Shadertoy shader
- **GIVEN** an imported shader that reads a music channel
- **WHEN** it plays with audio input on
- **THEN** it reacts to openVJ's audio input as it does to music on Shadertoy
