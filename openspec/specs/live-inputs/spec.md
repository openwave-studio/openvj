# live-inputs Specification

## Purpose

Cameras and Syphon servers are clip sources, so live video from capture devices and other apps (Synesthesia, TouchDesigner, VDMX) mixes like any clip.

## Requirements

### Requirement: Cameras
Webcams, capture cards and virtual cameras SHALL be listed as sources, picking the best format up to the composition size at 24 fps or more.

#### Scenario: Camera clip
- **WHEN** the VJ places and launches a camera
- **THEN** its live image plays on the layer

### Requirement: Syphon on macOS
On macOS, running Syphon servers SHALL appear as sources on their own, with Syphon.framework built in.

#### Scenario: New server
- **WHEN** Synesthesia starts publishing a Syphon server
- **THEN** it appears in the Sources tab without a refresh

### Requirement: Connect on demand and recover
A live input SHALL connect when its clip is launched or selected, disconnect a few seconds after it stops, show why if its device or server is gone, and reconnect on its own when it returns.

#### Scenario: Camera unplugged
- **GIVEN** a camera clip is playing
- **WHEN** the camera is unplugged and plugged back in
- **THEN** the clip shows it is disconnected, then resumes

### Requirement: Unsupported live sources are reported
Live sources openVJ doesn't support, such as NDI, SHALL be skipped and reported on import. NDI is not planned unless the maintainer asks for it.

#### Scenario: NDI in an .avc
- **WHEN** an .avc with an NDI clip is imported
- **THEN** the clip is skipped and named in the import report
