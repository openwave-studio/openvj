## 1. Analysis
- [ ] 1.1 Hits, presence and clocks per band and overall; auto-gained overall level
- [ ] 1.2 256-bin log spectrum and waveform
- [ ] 1.3 Tests: a kick reads as a hit, a steady tone doesn't; clocks run faster when loud

## 2. Modulation
- [ ] 2.1 ModInputs carries the new values and Reactivity; beat events from the beat position
- [ ] 2.2 New ModSource entries (Level, Hits, band hits) in map ▾
- [ ] 2.3 Reactivity on the composition, saved, shown in the Composition tab

## 3. Shaders
- [ ] 3.1 Globals and Synesthesia name shim in isf.rs
- [ ] 3.2 Spectrum texture; Ghost Arcade sampleFFT/sampleWaveform read it
- [ ] 3.3 GPU test: a shader using syn_* names compiles and reads the values

## 4. Wrap up
- [ ] 4.1 README "What works", archive the change, validate
