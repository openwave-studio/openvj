# audio-analysis Specification

## Purpose

Audio input is analysed into frequency bands that drive audio-reactive parameters.

## Requirements

### Requirement: Audio bands
When AUDIO is on, openVJ SHALL analyse the default or chosen input device into four auto-gained bands (Bass, Low mid, High mid, Treble) with adjustable gain and release, shown in the toolbar and the Composition tab.

#### Scenario: Choose input
- **WHEN** the VJ picks another input device and turns AUDIO on
- **THEN** the band meters follow that device
