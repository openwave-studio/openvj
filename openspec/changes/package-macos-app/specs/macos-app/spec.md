## ADDED Requirements

### Requirement: macOS app bundle
openVJ SHALL be packaged as `openVJ.app` for Apple Silicon Macs, starting from Finder or the Dock like any app, with Syphon built in.

#### Scenario: Open from Finder
- **WHEN** the VJ double-clicks openVJ.app
- **THEN** openVJ opens, and Syphon servers appear in the Sources tab

### Requirement: FFmpeg built in
The app SHALL include FFmpeg, so video files (including transparent WebM and HAP) play without installing anything, and SHALL carry FFmpeg's licence and where to get its source.

#### Scenario: Fresh Mac
- **GIVEN** a Mac without Homebrew or FFmpeg
- **WHEN** the VJ drops an .mp4 or .mov into a clip slot
- **THEN** it plays

### Requirement: Camera and microphone permission in openVJ's name
The app SHALL ask for camera and microphone access itself, explaining why, instead of inheriting the permission of the terminal it was started from.

#### Scenario: First camera
- **WHEN** the VJ launches a camera clip for the first time
- **THEN** macOS asks whether openVJ may use the camera, and once allowed the camera plays

#### Scenario: Audio input
- **WHEN** the VJ picks an audio input for the first time
- **THEN** macOS asks whether openVJ may use the microphone

### Requirement: Signed builds from CI
Each version tag SHALL produce a signed (at least ad hoc) zipped app attached to a draft GitHub release, and the same build SHALL be available on demand.

#### Scenario: Tagging a release
- **GIVEN** the workspace version is 0.1.0
- **WHEN** the maintainer pushes the tag `v0.1.0`
- **THEN** a draft release with `openVJ-0.1.0-macos-arm64.zip` appears on GitHub

### Requirement: Notarized releases
When the Developer ID secrets are configured, manual and tagged builds SHALL be signed with Developer ID and notarized, so a downloaded copy opens without a Gatekeeper warning.

#### Scenario: Downloading a release
- **GIVEN** the Developer ID secrets are set
- **WHEN** the maintainer downloads and opens the zip from a release
- **THEN** openVJ opens without "Open Anyway"
