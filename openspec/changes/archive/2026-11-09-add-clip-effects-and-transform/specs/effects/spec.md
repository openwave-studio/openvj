# Spec Delta

## MODIFIED Requirements

### Requirement: Effect chains
Every clip, layer, group and the composition SHALL have an effect chain. Each effect SHALL have Bypass, reorder, remove and a dry/wet mix. A clip's chain SHALL run before the clip reaches its layer, and travel with the clip when it is copied or moved.

#### Scenario: Reorder
- **WHEN** the VJ moves Blur above Edges in a layer's chain
- **THEN** the layer is blurred before edge detection

#### Scenario: Clip effect
- **GIVEN** a layer whose clip A has Mirror and clip B has no effects
- **WHEN** the VJ launches A and then B
- **THEN** A plays mirrored and B plays unprocessed, and during the transition each is processed on its own before the crossfade

## ADDED Requirements

### Requirement: Clip and layer Transform
Every clip and layer SHALL have a fixed Transform at the top of its card with Position X/Y in pixels, Scale and Scale W/H in %, Rotation X/Y/Z in degrees and Anchor X/Y, applied before that clip's or layer's effects. A Transform left at its defaults SHALL cost no render pass.

#### Scenario: Zoomed clip
- **WHEN** the VJ sets a clip's Scale to 150%
- **THEN** that clip plays zoomed in while other clips on the layer stay full-frame

#### Scenario: Tilt
- **WHEN** the VJ sets a layer's Rotation X to 30°
- **THEN** the layer tilts back in perspective
