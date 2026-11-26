# layer-mixing Specification

## Purpose

Layers are stacked and blended into the composition on the GPU, with Resolume-style layer groups, a crossfader and a master fader.

## Requirements

### Requirement: Layer controls
Each layer SHALL have an opacity fader, a blend mode, Bypass, Solo, clear, and an effect chain. The blend modes SHALL be Normal, Add, Screen, Multiply, Overlay, Difference, Lighten, Darken, Subtract, Soft Light, Hard Light, Dodge, Burn, Exclusion, Luma Key, Mask, Mask Inverted, Stencil and Silhouette, named as in Resolume and grouped in the menu as Mix, Light and Key.

#### Scenario: Solo
- **WHEN** the VJ solos layer 2
- **THEN** only layer 2 is mixed into the output until solo is turned off

#### Scenario: Pick a blend mode
- **WHEN** the VJ opens layer 2's blend menu
- **THEN** the modes are listed under Mix, Light and Key, and picking Soft Light applies it on the next frame

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

### Requirement: Keying and masking blends
Luma Key SHALL make the dark parts of the layer transparent. Mask SHALL show what is below the layer in proportion to the layer's brightness (black hides it), and Mask Inverted the reverse. Stencil SHALL show what is below only where the layer is opaque, and Silhouette SHALL cut what is below out where the layer is opaque. "Below" SHALL be the layers under it in its group, or the composition mixed so far for a layer outside a group, as in Resolume.

#### Scenario: Logo on black
- **GIVEN** layer 2 plays a white logo on black, set to Luma Key
- **WHEN** layer 1 plays a video
- **THEN** the logo shows over the video and the black around it is transparent

#### Scenario: Mask with a shape
- **GIVEN** layer 1 plays a video and layer 2, set to Mask, plays a white circle on black
- **THEN** the output shows the video inside the circle and black outside it

#### Scenario: Mask inside a group
- **GIVEN** layers 2 and 3 are in a group and layer 3 is set to Stencil
- **THEN** layer 3 cuts only layer 2, and layer 1 below the group is untouched
