# layer-mixing Specification

## Purpose

Layers are stacked and blended into the composition on the GPU, with Resolume-style layer groups, a crossfader and a master fader.

## Requirements

### Requirement: Layer controls
Each layer SHALL have an opacity fader, a blend mode (Normal, Add, Screen, Multiply, Overlay, Difference, Lighten, Darken, Subtract), Bypass, Solo, clear, and an effect chain.

#### Scenario: Solo
- **WHEN** the VJ solos layer 2
- **THEN** only layer 2 is mixed into the output until solo is turned off

### Requirement: Layer groups
Layers SHALL be assignable to a group. A group SHALL mix its layers on their own, then blend the result into the composition with its own opacity, blend mode, Bypass, Solo and effect chain, as Resolume's layer groups do.

#### Scenario: Group opacity
- **GIVEN** layers 1 and 2 are in a group
- **WHEN** the group's opacity is set to 50%
- **THEN** the mix of layers 1 and 2 appears at half opacity, without changing their own faders

### Requirement: Crossfader
Layers SHALL be assignable to side A or B, and a crossfader SHALL fade between the sides with either a linear curve or one that keeps both sides full in the middle. Double-clicking the crossfader SHALL recentre it.

#### Scenario: Fade to B
- **GIVEN** layer 1 is on A and layer 2 on B
- **WHEN** the crossfader moves fully to B
- **THEN** layer 1 is hidden and layer 2 shows at its own opacity

### Requirement: Master
A master fader and composition effect chain SHALL apply to the whole mix before output.

#### Scenario: Master down
- **WHEN** the master fader is at 0
- **THEN** the output is black
