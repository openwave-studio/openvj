# Proposal

## Why

The maintainer's main setup is openDJ playing the music, with its Session (an Ableton-style clip grid) on a channel, and openVJ running the visuals on the same Mac. Tempo and bars already line up: both join Ableton Link with 4-beat bars. What openVJ can't see is the music's structure. openDJ already publishes it over OSC: `/beat` (beat in the bar, bar in the phrase, BPM), `/phrase` (Intro, Build, Drop, Breakdown, Chorus, Verse, Outro, length in bars), `/deck/N/track` (title, artist, BPM, key) and `/deck/N/cue`, and it announces itself over Bonjour as "openDJ". Research: the "openDJ and openVJ integration" doc (2026-11-25).

## What Changes

- **OSC input** in openVJ, off by default. An **openDJ** switch in Settings finds openDJ on this Mac or the network (Bonjour), subscribes to its beat, phrase, track and cue events, and shows the connection, the phrase and the track in the top bar's tempo area. Nothing needs setting in openDJ beyond turning its OSC on.
- **Phrase-aware Player**: two more lengths, **Phrase** (until openDJ starts a new phrase) and **Track** (until openDJ loads and plays a new track). Transitions start so the new preset is fully in on the phrase's downbeat. With openDJ connected, **Song** uses openDJ's track changes instead of guessing from the audio.
- **Phrase triggers in Mixer**: in MIDI map mode, a phrase name (Drop, Build…) can be learned like a pad and launch a column or a clip, or act as Autopilot's "next". This is the Resolume workflow openDJ's own guide suggests (map `/phrase` to a clip trigger).
- **Drivers from openDJ** in every parameter's map menu: **Phrase position** (0 to 1 across the current phrase, so a Build ramps into the Drop) and **Track key** (the key's place on the Camelot wheel, 0 to 1, to drive a hue).
- **Session MIDI**: openDJ Session tracks that send MIDI to a virtual port (the IAC Driver on macOS) already reach openVJ's MIDI input and learn. The guide gets a section on setting it up; openVJ needs no code for it.

Out of scope: sending OSC, controlling openVJ from OSC (Resolume's OSC addresses, later), output recording, and anything that needs openDJ changes. Those are asked for in the openDJ project: Session scene and clip events, the next phrase's name and bars until it, master and stem levels, and a master-deck-change event.

## Capabilities

### New Capabilities
- `opendj-sync`: connecting to openDJ and receiving its beat, phrase, track and cue events.

### Modified Capabilities
- `layouts`: Player lengths Phrase and Track.
- `parameter-modulation`: Phrase position and Track key drivers.

## Touches

- New `crates/openvj-app/src/osc.rs` (receiver thread, Bonjour browse, subscribe), using the `rosc` crate and `mdns-sd` (both MIT/Apache).
- crates/openvj-core: `modulation.rs` (two sources), player model (two lengths), `midi.rs` (phrase as a learnable source)
- crates/openvj-app: `src/app.rs`, `ui/main.slint` (tempo area), settings UI, `ui/layouts.slint` (Player length menu)
- Specs: new opendj-sync; layouts, parameter-modulation, midi-control (phrase learn)
- Shared hot spots: `src/app.rs`, `ui/main.slint`, `composition.rs` (saved phrase mappings)
- README "What works", user docs on the openDJ setup

## Conflicts

- add-gated-envelopes (proposed alongside) adds envelope drivers whose triggers include openDJ's downbeat, phrase and cue. Land this first, or land envelopes first with those triggers hidden until this merges.
- add-parameter-animation (not started) reshapes drivers. The two new drivers are plain sources and migrate like the audio bands.
- add-midi-learn (open, export blocked) owns map mode. Phrase learn reuses it.
- No open PRs right now.

## Decisions (the maintainer, 2026-11-26)

- openVJ connects to openDJ by itself once the switch has been turned on, and remembers it.
- Phrase triggers come only from learned mappings, as in Resolume; no fixed phrase-to-column rule.
- The track's key is only made available (shown, and as a driver) for now, to start making visuals key-aware.
- OSC is the link, being the standard visuals and lighting apps already speak.
