## ADDED Requirements

### Requirement: Synesthesia audio names
openVJ SHALL accept Synesthesia's audio, beat and coordinate uniform names in ISF files (`syn_BassLevel`, `syn_BassHits`, `syn_BassTime`, `syn_OnBeat`, `syn_BPMSin4`, `syn_Spectrum`, `_uvc` and the rest of the family), with Synesthesia's Mid, MidHigh and High bands mapped onto openVJ's Low mid, High mid and Treble. The full list is in the README.

#### Scenario: Shader written for Synesthesia
- **WHEN** an ISF generator scales its rings by `syn_BassHits` and spins with `syn_BassTime`
- **THEN** it compiles in openVJ and the rings jump on kicks and spin faster while the bass is loud

### Requirement: Real spectrum for Ghost Arcade
Ghost Arcade's `sampleFFT` and `sampleWaveform` SHALL read the real spectrum and waveform.

#### Scenario: Waveform shader
- **WHEN** a Ghost Arcade generator that draws `sampleWaveform` plays with AUDIO on
- **THEN** it draws the incoming waveform instead of a flat line
