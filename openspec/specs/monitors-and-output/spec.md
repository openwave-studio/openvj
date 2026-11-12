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
openVJ SHALL open one output window per enabled screen of the output setup (one window when there is no setup), each titled with its screen's name. A window SHALL go fullscreen by double-click or F, and leave fullscreen with Esc. When a screen is set to a display, its window SHALL go fullscreen on that display, like a Resolume screen's Output Device.

#### Scenario: Projector
- **WHEN** the output window is on the projector and the VJ presses F
- **THEN** it fills that display

#### Scenario: Two projectors
- **GIVEN** an output setup with Screen 1 set to the left projector and Screen 2 set to the right one, both connected
- **WHEN** openVJ starts
- **THEN** each projector shows its screen fullscreen

#### Scenario: Display missing
- **GIVEN** a screen set to a display that isn't connected
- **WHEN** openVJ starts
- **THEN** that screen opens as a normal window the VJ can drag to any display

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
