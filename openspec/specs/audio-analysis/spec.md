# audio-analysis Specification

## Purpose

Audio input is analysed into frequency bands that drive audio-reactive parameters.

## Requirements

### Requirement: Audio bands
When AUDIO is on, openVJ SHALL analyse the default or chosen input device into four auto-gained bands (Bass, Low mid, High mid, Treble) with adjustable release, shown in the toolbar and the Composition tab.

#### Scenario: Choose input
- **WHEN** the VJ picks another input device and turns AUDIO on
- **THEN** the band meters follow that device

### Requirement: Hits, presence and audio clocks
For each band and for the whole signal, openVJ SHALL derive a level, hits (a 0–1 spike on transients such as kicks and snares that falls back quickly), presence (a slow 0–1 value that stays high while the band is strong in the song) and an audio clock (seconds that run faster while the band is loud), as Synesthesia does.

#### Scenario: Kick drum
- **WHEN** a bass hit follows a quiet passage
- **THEN** Bass hits jumps near 1 and falls back within a fraction of a second, while a steady bass tone leaves it near 0

### Requirement: Spectrum
openVJ SHALL keep a 256-bin, log-spaced, auto-gained spectrum and the latest waveform for shaders.

#### Scenario: Spectrum shader
- **WHEN** a Ghost Arcade generator that calls `sampleFFT` plays with AUDIO on
- **THEN** it reads the frequency at the position it asks for, not one of four bands

### Requirement: Reactivity
The Composition tab SHALL have a Reactivity control, in place of the former Gain, (0–200%, default 100%), saved with the composition, that scales every audio-driven value (audio mappings, audio values and spectrum in shaders, and the speed of the audio clocks), like Synesthesia's Reactivity. Values stay capped at full scale.

#### Scenario: Calm everything
- **WHEN** the VJ turns Reactivity down to 50%
- **THEN** every audio-mapped parameter and audio-reactive shader moves half as much
