# Proposal

## Why

Today a parameter can only wobble around its base value with an audio band or one of four tempo shapes. Resolume sets animate parameters with timelines in beats or seconds, envelopes, FFT, dashboard links and in/out ranges, and those animations should survive an .avc import. The maintainer also wants to start playing parameters from a MIDI controller with knobs and faders.

## What Changes

- Each parameter gets at most one driver (Resolume's model; stacking drivers comes later): Timeline in seconds, Timeline in beats, Audio FFT, Dashboard dial, Clip position, or Crossfader.
- A driver produces a phase 0..1, shaped by an optional envelope, then mapped into an in/out range.
- Today's mappings migrate: base + depth × signal becomes the range [base, base + depth]; Sine, Saw, Square and Pulse become envelope presets.
- Animation settings unfold under each parameter, Resolume-style.
- Dashboard dials come with MIDI mapping, before envelopes.
- .avc parameter animations import.

Out of scope: stacking several drivers on one parameter.

## Capabilities

### New Capabilities

### Modified Capabilities
- `parameter-modulation`: one driver per parameter with envelope and range, replacing audio band / tempo shape + depth.

## Impact

Not started. Builds on add-isf-effect-engine's typed parameters.

## Touches

- crates/openvj-core: `modulation.rs`, parameter types, `composition.rs` (saved format, with migration)
- crates/openvj-resolume: `lib.rs` (PhaseSource*, ModifierEnvelope, ValueRange)
- crates/openvj-app: `app.rs`, `ui/properties.slint`, `controls.slint`
- Specs: parameter-modulation, resolume-import, midi-control
- Shared hot spots: composition.rs, app.rs, resolume lib.rs

## Conflicts

- add-isf-effect-engine (PR #9): merge it first.
- restyle-ui-like-opendj (PR #10): the unfolding parameter settings should use its controls; merge it first.

## Open questions

- Resolume's play direction and play mode codes, and envelope curve codes, are undecoded. The maintainer is making a small test .avc to decode them.
- Which controller the maintainer will use for dashboard dials (none chosen yet).
