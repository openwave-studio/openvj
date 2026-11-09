## ADDED Requirements

### Requirement: Syphon output on macOS
On macOS, openVJ SHALL publish its composition output as a Syphon server listed by the app name alone ("OpenVJ"), like Resolume's composition Syphon server. A "Syphon output" switch in Settings → Output SHALL turn it on and off, on by default. Frames SHALL stay on the GPU and SHALL only be published while a client is connected.

#### Scenario: Receive openVJ in another app
- **GIVEN** openVJ is running
- **WHEN** the VJ picks "OpenVJ" as a Syphon source in another app, such as MadMapper or OBS
- **THEN** that app shows openVJ's output mix, the right way up and at the composition size

#### Scenario: Switched off
- **WHEN** the VJ turns off Syphon output in Settings → Output
- **THEN** other apps no longer list openVJ as a Syphon source

#### Scenario: Nobody listening
- **WHEN** no app is connected to openVJ's Syphon server
- **THEN** openVJ does no extra work per frame for it
