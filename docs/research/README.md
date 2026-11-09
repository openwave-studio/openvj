# Early research (October 2026)

Notes written before any code existed, when openVJ was planned as an **OBS Studio plugin**. The project then moved to a standalone Rust app (wgpu + Slint). The reasons: its own frame clock and outputs, compute shaders, BC4/BC7 textures for every HAP variant, a custom performance UI, and no OBS plugin ABI churn. openVJ will send its output into OBS over Syphon, Spout or NDI instead of running inside it.

The research still applies in large part:

- [01 – OBS as a host](01-obs-as-host.md): what OBS offers. Mostly relevant now to the future OBS bridge.
- [02 – Key libraries](02-libraries.md): HAP, ISF, MIDI/OSC/Link and audio analysis carry over. The Rust equivalents are in the main README.
- [03 – UI](03-ui.md): the Resolume-inspired layout the alpha implements.
- [04 – Architecture](04-architecture.md): data model, threading rules, render pipeline and roadmap. The OBS-specific parts don't apply.
