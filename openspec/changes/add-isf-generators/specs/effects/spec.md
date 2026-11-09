# Spec Delta

## ADDED Requirements

### Requirement: Ghost Arcade ISF names
openVJ SHALL accept ISF files written for Ghost Arcade: its audio values (`audioLevel`, `audioBass`, `audioMid`, `audioHigh`, `audioBeat`, `audioBeatPhase`, `audioBPM`, `audioSpectralCentroid`), its `sampleFFT` and `sampleWaveform` functions, `renderSize`, and files that redeclare built-in uniforms or set a precision.

#### Scenario: Ghost Arcade effect
- **WHEN** an ISF filter that reads `audioBass` is added to a layer
- **THEN** it compiles and follows the bass band of the audio input
