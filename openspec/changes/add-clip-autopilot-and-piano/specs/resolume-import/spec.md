## ADDED Requirements

### Requirement: Import Autopilot and trigger style
Importing an .avc SHALL bring over each clip's Autopilot setting and trigger style.

#### Scenario: Autopilot set in Resolume
- **GIVEN** an .avc where a clip has Autopilot set to Next clip
- **WHEN** it is imported
- **THEN** that clip's Autopilot is Next clip
