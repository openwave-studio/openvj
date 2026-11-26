## MODIFIED Requirements

### Requirement: Layer controls
Each layer SHALL have an opacity fader, a blend mode, Bypass, Solo, clear, and an effect chain. The blend modes SHALL be Normal, Add, Screen, Multiply, Overlay, Difference, Lighten, Darken, Subtract, Soft Light, Hard Light, Dodge, Burn, Exclusion, Luma Key, Mask, Mask Inverted, Stencil and Silhouette, named as in Resolume and grouped in the menu as Mix, Light and Key.

#### Scenario: Solo
- **WHEN** the VJ solos layer 2
- **THEN** only layer 2 is mixed into the output until solo is turned off

#### Scenario: Pick a blend mode
- **WHEN** the VJ opens layer 2's blend menu
- **THEN** the modes are listed under Mix, Light and Key, and picking Soft Light applies it on the next frame

## ADDED Requirements

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
