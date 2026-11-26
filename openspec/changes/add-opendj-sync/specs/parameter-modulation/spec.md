## ADDED Requirements

### Requirement: Drivers from openDJ
With openDJ connected, the map menu SHALL offer **Phrase position** (0 at the start of the current phrase to 1 at its end) and **Track key** (the master track's key on the Camelot wheel, 1A to 12B, as 0 to 1).

#### Scenario: Build ramps into the drop
- **GIVEN** a zoom parameter mapped to Phrase position with depth +100%
- **WHEN** a 16-bar Build plays
- **THEN** the zoom rises across the 16 bars and starts again with the next phrase
