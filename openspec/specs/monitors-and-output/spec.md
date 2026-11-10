# monitors-and-output Specification

## Purpose

The VJ sees what they are about to show and what is live, and sends the mix to a projector.

## Requirements

### Requirement: Preview and output monitors
The window SHALL show a Preview of the selected clip and the Output mix, and the grid SHALL show live thumbnails.

#### Scenario: Preview
- **WHEN** the VJ right-clicks a clip
- **THEN** Preview shows it without changing the output

### Requirement: Output window
A separate output window SHALL go fullscreen by double-click or F, and leave fullscreen with Esc.

#### Scenario: Projector
- **WHEN** the output window is on the projector and the VJ presses F
- **THEN** it fills that display

### Requirement: Syphon output on macOS
On macOS, openVJ SHALL publish its composition output as a Syphon server listed by the app name alone ("openVJ"), like Resolume's composition Syphon server. A "Syphon output" switch in Settings → Output SHALL turn it on and off, on by default. Frames SHALL stay on the GPU and SHALL only be published while a client is connected.

#### Scenario: Receive openVJ in another app
- **GIVEN** openVJ is running
- **WHEN** the VJ picks "openVJ" as a Syphon source in another app, such as MadMapper or OBS
- **THEN** that app shows openVJ's output mix, the right way up and at the composition size

#### Scenario: Switched off
- **WHEN** the VJ turns off Syphon output in Settings → Output
- **THEN** other apps no longer list openVJ as a Syphon source

#### Scenario: Nobody listening
- **WHEN** no app is connected to openVJ's Syphon server
- **THEN** openVJ does no extra work per frame for it
