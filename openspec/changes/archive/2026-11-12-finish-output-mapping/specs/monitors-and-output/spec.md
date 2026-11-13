# Spec Delta

## MODIFIED Requirements

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
