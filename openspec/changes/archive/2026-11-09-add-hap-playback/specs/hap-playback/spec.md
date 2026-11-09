## ADDED Requirements

### Requirement: Decode HAP on the GPU
HAP movies in QuickTime files (HAP, HAP Alpha, HAP Q, HAP Q Alpha and HAP R, with Snappy or no second-stage compression, chunked or not) SHALL be read from the file and uploaded as compressed BC textures, as Vidvox's HAP spec describes, without starting an FFmpeg process.

#### Scenario: HAP Q clip
- **WHEN** a HAP Q .mov exported from Resolume Alley or AVF Batch Converter is launched
- **THEN** it plays without starting an FFmpeg process, with the same colours FFmpeg's HAP decoder gives

#### Scenario: HAP Alpha clip
- **WHEN** a HAP Alpha or HAP Q Alpha clip is launched on a layer above another clip
- **THEN** its transparent parts show the layer below

### Requirement: HAP fallback
If the GPU lacks BC texture support or a HAP frame fails to decode, the clip SHALL fall back to FFmpeg, as DXV does.

#### Scenario: No BC support
- **GIVEN** a GPU without BC compression
- **WHEN** a HAP clip is launched
- **THEN** it plays through FFmpeg instead
