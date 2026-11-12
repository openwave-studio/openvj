# Proposal

## Why

openVJ's audio reactivity is four band levels plus the beat. Levels follow loudness, so visuals swell with the music but don't punch on kicks and snares, and there is no way to calm or boost every audio mapping at once. Synesthesia, which the maintainer runs alongside openVJ, gives its shaders about 40 audio and beat signals under well-known names. Matching that (from the Synesthesia comparison, items 1 to 4, approved by the maintainer on 2026-11-12) makes the 345 bundled generators react better and lets shaders written for Synesthesia compile in openVJ.

## What Changes

- **Richer analysis**, from the FFT openVJ already runs: per band and overall *hits* (spikes on transients), *presence* (slow, high while a band is strong in the song) and *audio clocks* (run faster while a band is loud); an auto-gained overall level; beat events from the tempo clock (on beat, toggle on beat, random on beat).
- **New map ▾ sources**: Level, Hits, and Bass / Low mid / High mid / Treble hits, next to today's band levels and tempo shapes.
- **Synesthesia names in ISF files**: `syn_Level`, `syn_BassLevel`… `syn_*Hits`, `syn_*Presence`, `syn_*Time`, `syn_OnBeat`, `syn_ToggleOnBeat`, `syn_RandomOnBeat`, `syn_BeatTime`, `syn_BPM`, `syn_BPMConfidence`, `syn_BPMSin/2/4/8`, `syn_BPMTri/2/4/8`, `syn_Spectrum`, `_uv`, `_uvc`, `_xy`, `FRAMECOUNT`. Synesthesia's Mid, MidHigh and High map onto Low mid, High mid and Treble.
- **Spectrum texture**: 256 log-spaced FFT bins and the waveform, uploaded once per frame. Ghost Arcade's `sampleFFT` and `sampleWaveform` read it instead of stepping through the four bands and returning 0.
- **Reactivity**: one composition-level knob (0–200%, default 100%) in the Composition tab's audio section, saved with the composition, scaling every audio-driven value (mappings and shaders) at once, like Synesthesia's Reactivity.

Out of scope: importing Synesthesia scenes (.synScene), Synesthesia's helper functions (`_rotate`, `_noise`…), `syn_LevelTrail`, `syn_Intensity`, `syn_FadeInOut`, `syn_Media`, automatic BPM detection, and a per-parameter custom frequency range with threshold/attack/release (that belongs to add-parameter-animation's Audio FFT driver).

## Capabilities

### Modified Capabilities
- `audio-analysis`: hits, presence, clocks, overall level, spectrum, Reactivity.
- `parameter-modulation`: new audio sources in map ▾.
- `effects`: Synesthesia names and a real spectrum in ISF files.

## Touches

- crates/openvj-audio: `lib.rs` (analysis)
- crates/openvj-core: `modulation.rs` (ModInputs, ModSource), `composition.rs` (reactivity field, serde default)
- crates/openvj-render: `isf.rs` (globals, name shim, spectrum binding), `lib.rs` (uniform values, spectrum texture)
- crates/openvj-app: `app.rs` (wire analysis and Reactivity), `ui/properties.slint` (Reactivity row)
- Specs: audio-analysis, parameter-modulation, effects; README "What works"
- Shared hot spots: composition.rs (one field), app.rs, render lib.rs

## Conflicts

- add-parameter-animation (not started): it replaces ModSource with drivers. The new sources here become choices of its Audio FFT driver (Level / Hits); merge this first, then migrate them with the rest.
- No open PRs.

## Open questions

- None blocking. Reactivity is not MIDI-mappable yet (Resolume has no equivalent target to import); it can join MIDI learn later.
