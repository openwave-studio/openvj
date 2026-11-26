## ADDED Requirements

### Requirement: Keying blends and effects import
Importing an .avc SHALL map Resolume's Soft Light, Hard Light, Dodge, Burn, Exclusion, Luma Key, Mask, Mask Inverted, Stencil and Silhouette blend modes, and its Luma Key, Chroma Key, Trails, Strobe, Threshold, Tile, Slide, Bloom, Vignette and Infinite Zoom effects with the settings that map, instead of reporting them as dropped.

#### Scenario: Masked layer
- **GIVEN** an .avc with a layer set to Mask
- **WHEN** it is opened
- **THEN** the layer's blend mode is Mask and the status bar doesn't list it as skipped
