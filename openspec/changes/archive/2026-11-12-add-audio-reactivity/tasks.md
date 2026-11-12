## 1. Analysis
- [x] 1.1 Hits, presence and clocks per band and overall; auto-gained overall level
- [x] 1.2 256-bin log spectrum and waveform
- [x] 1.3 Tests: a kick reads as a hit, a steady tone doesn't; clocks run faster when loud

## 2. Modulation
- [x] 2.1 ModInputs carries the new values and Reactivity; beat events from the beat position
- [x] 2.2 New ModSource entries (Level, Hits, band hits) in map ▾
- [x] 2.3 Reactivity on the composition, saved, shown in the Composition tab

## 3. Shaders
- [x] 3.1 Globals and Synesthesia name shim in isf.rs
- [x] 3.2 Spectrum texture; Ghost Arcade sampleFFT/sampleWaveform read it
- [x] 3.3 GPU test: a shader using syn_* names compiles and reads the values

## 4. Wrap up
- [x] 4.1 README "What works", archive the change, validate
