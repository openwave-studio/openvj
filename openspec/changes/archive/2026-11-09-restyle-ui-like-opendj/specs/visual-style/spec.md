# Spec Delta

## Purpose

openVJ looks like a sibling of openDJ, with one cyan accent, keeping Resolume-style elements only where the maintainer chose them.

## ADDED Requirements

### Requirement: openDJ look
The UI SHALL use openDJ's palette, Inter Tabular type, flat dark controls with thin borders, and openDJ-style buttons, tabs, faders, menus and Settings window.

#### Scenario: New control
- **WHEN** a feature adds a button or fader
- **THEN** it uses the shared openDJ-style control, not a one-off look

### Requirement: One cyan accent
The only accent colour SHALL be cyan #4fe6da, used for the logo, active and selected things, the output and playing clips.

#### Scenario: Playing clip
- **WHEN** a clip is playing
- **THEN** its cell lights in the cyan accent, not green

### Requirement: Resolume-style where chosen
Parameters SHALL stay Resolume-style horizontal bars (no knobs), and the clip grid SHALL keep spaced cells. Other Resolume-style elements need the maintainer's approval, shown as a mockup first.

#### Scenario: Effect parameter
- **WHEN** an effect card shows a parameter
- **THEN** it is a horizontal value bar

### Requirement: Theme tokens
Every colour and size SHALL come from the shared theme, so the app can be restyled in one place.

#### Scenario: Change the accent
- **WHEN** the accent token changes
- **THEN** every accent-coloured element follows
