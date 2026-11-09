# Proposal

## Why

The maintainer mixes openVJ with other Syphon apps on the same Mac (Synesthesia, MadMapper, OBS, a second VJ app). openVJ can already receive Syphon; it can't send. Resolume publishes its composition as a Syphon server ("Arena - Composition"), and VJs expect the same so they can map, record or stream the mix in another app.

## What Changes

- On macOS, openVJ publishes its composition output as a Syphon server listed as just **"OpenVJ"** (the maintainer's choice; Resolume lists "Arena - Composition").
- A **Syphon output** switch in Settings → Output, on by default. Frames are published only while at least one client is connected, so it costs nothing when nobody listens.
- Frames stay on the GPU: the final mix is converted to BGRA in one extra fullscreen pass and handed to Syphon's Metal server on openVJ's own Metal command queue. No CPU readback, no extra thread.
- The server follows the composition size and is published the right way up (Metal orientation, `flipped: NO`).

Out of scope: per-layer and per-screen Syphon outputs (in Resolume these come from Advanced Output screens whose device is Syphon; they belong with output mapping, not now per the maintainer), a custom server name, remembering the switch between launches (no setting is saved yet), Spout and NDI.

## Capabilities

### New Capabilities

### Modified Capabilities
- `monitors-and-output`: the composition is also sent to other apps over Syphon.

## Impact

macOS only, behind the existing `syphon` feature. Linux and Windows builds are unchanged.

## Touches

- `crates/openvj-render/src/lib.rs` (shared hot spot): an optional BGRA copy of the output, made only when asked for.
- `crates/openvj-app/src/app.rs` (shared hot spot): create the server and publish after each frame, a few lines in `frame()`, plus the switch.
- `crates/openvj-app/ui/settings.slint`, `crates/openvj-app/ui/main.slint` (shared hot spot): the Syphon output row on the Output page.
- New `crates/openvj-app/src/syphon_out.rs`; `crates/openvj-app/Cargo.toml` (syphon-core, objc2, objc2-metal on macOS).
- README.md (shared hot spot): "What works".
- Specs: `monitors-and-output`.

## Conflicts

Open PR #18 (perf) edits `Engine::render` and `App::frame` too, in nearby lines. Either order works; whichever merges second rebases (small, mechanical). No other change in `openspec/changes/` touches these files. The output mapping work will later add Syphon as a screen device; it can reuse `syphon_out.rs`.

## Open questions

Answered by the maintainer (2026-11-09): list it as just the app name; add an on/off switch, on by default; no per-layer outputs for now.
