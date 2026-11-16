## ADDED Requirements

### Requirement: Shadertoy names
openVJ SHALL accept Shadertoy code in ISF files: a `mainImage` function instead of `main`, and Shadertoy's `iTime`, `iTimeDelta`, `iFrame`, `iResolution`, `iDate`, `iMouse`, `iChannelTime`, `iChannelResolution` and `iSampleRate` uniforms. A sound channel SHALL read openVJ's audio as Shadertoy's 512×2 spectrum and waveform texture. Shadertoy buffers SHALL run as persistent passes in Shadertoy's order.

#### Scenario: Hand-copied shader
- **WHEN** the VJ saves a Shadertoy Image tab's code as `Clouds.glsl` in the Effects folder
- **THEN** it is listed as a generator and plays

#### Scenario: Feedback buffer
- **GIVEN** an imported shader whose Buffer A reads itself
- **WHEN** it plays
- **THEN** Buffer A sees its previous frame, as on Shadertoy

### Requirement: Shadertoy shaders as effects
A Shadertoy shader whose first channel is a video or webcam input SHALL import as an effect, with that channel fed by the clip, layer or group it is applied to.

#### Scenario: Shadertoy filter
- **WHEN** the VJ imports a shader that distorts its webcam channel
- **THEN** it is listed in the Effects tab and distorts the layer it is added to
