## ADDED Requirements

### Requirement: Import Layer and Group Routers
Importing an .avc SHALL turn Layer Router and Group Router clips into Layer Router clips with the same input.

#### Scenario: Router on layer 1
- **GIVEN** an .avc with a Layer Router clip whose input is layer 1
- **WHEN** it is imported
- **THEN** the clip is a Layer Router with Input layer 1
