# Proposal

## Why

Tempo modulation has four shapes today (Sine, Saw, Square, Pulse), always starting on the beat. Imaginando VS has 8 LFO shapes with phase, one-shot and retrigger, and Lumen has phase-linked LFOs. Resolume's parameter animation has a random direction and a phase too. This is the cheapest way to get more motion out of every generator and effect. It came out of the Lumen and VS comparison (2026-11-12).

## What Changes

- New tempo shapes in the **map ▾** menu: **Triangle**, **Ramp down**, **Random** (a new random value each cycle, held), and **Smooth random** (eases to a new random value each cycle). Random values come from a seeded hash of the cycle number, so every parameter mapped to Random over the same beats moves the same way. A per-mapping seed keeps two mappings from moving together.
- A **Phase** setting (0–100% of a cycle) on every tempo shape, so two parameters can move in opposite directions on the same beat. This is the same idea as Lumen's linked LFOs.
- A **Restart on launch** switch: the shape's cycle starts when the clip launches, not on the global beat. This is like VS's retrigger and Resolume's clip-position phase.
- Rates gain **32** and **64** beats, for slow drifts over a phrase.
- Saved compositions keep working: missing fields default to today's behaviour.

Out of scope: envelopes, in/out ranges, Hz rates and stacking (all in add-parameter-animation), and audio-band changes (the Synesthesia audio work).

## Capabilities

### Modified Capabilities
- `parameter-modulation`: more tempo shapes, phase, restart on launch, longer rates.

## Touches

- crates/openvj-core: `modulation.rs` (shapes, phase, seed, launch time), `composition.rs` (pass clip launch time into the modulation inputs only)
- crates/openvj-app: `ui/properties.slint` / `ui/controls.slint` (the map ▾ popup: phase field, restart switch), `src/app.rs` (wiring)
- Specs: parameter-modulation
- Shared hot spots: `composition.rs` (one field), `src/app.rs`

## Conflicts

- add-parameter-animation (not started) will later turn tempo shapes into envelope presets. These new shapes become presets too. Land this first; it's small and keeps the current model.
- PR #18 (Synthesis layout) lets macros take LFO drivers. Its macros should offer the same shapes once both merge. Merge #18 first.

## Open questions

- None.
