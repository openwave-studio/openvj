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
