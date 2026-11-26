# osc-control Specification

## Purpose

OSC apps (TouchOSC, Lemur, Max, show control) drive openVJ with Resolume's OSC addresses, see its state through OSC output, and can map their own addresses in map mode.

## Requirements

### Requirement: OSC input and output settings
Settings SHALL have an OSC input switch with a UDP port (default 7000) and an OSC output switch with a host and port (default this computer, 7001), all off by default, matching Resolume's defaults. The Composition tab SHALL show the last OSC message received.

#### Scenario: Turn on OSC input
- **WHEN** the VJ turns on OSC input on port 7000
- **THEN** openVJ listens on UDP 7000 and announces itself over Bonjour as an OSC service

### Requirement: Resolume OSC addresses
OSC input SHALL accept Resolume's addresses for the controls openVJ has: master, crossfader, tempo, tap and resync, column and clip connect, deck select, disconnect all, and each layer's clear, opacity, bypass, solo and select, also in the selected-layer form. Faders SHALL take 0–1 floats and buttons 1 for press and 0 for release, as in Resolume.

#### Scenario: Launch a clip
- **WHEN** openVJ receives `/composition/layers/2/clips/3/connect 1`
- **THEN** the clip in layer 2, column 3 of the open deck launches

#### Scenario: Selected layer
- **GIVEN** layer 2 is selected
- **WHEN** openVJ receives `/composition/selectedlayer/video/opacity 0.5`
- **THEN** layer 2's opacity is 50%

### Requirement: Effects over OSC
Effect parameters, mix (`opacity`) and bypass on layers and the composition SHALL be reachable at Resolume-style addresses, naming the effect and parameter by their names in lowercase without spaces, or by the Resolume names they import from. Group controls follow when they become MIDI-mappable.

#### Scenario: Effect parameter
- **GIVEN** layer 1 has a Blur effect
- **WHEN** openVJ receives `/composition/layers/1/video/effects/blur/effect/radius 0.5`
- **THEN** the Blur's radius is set to the middle of its range

#### Scenario: Unknown address
- **WHEN** openVJ receives an address it doesn't support
- **THEN** nothing changes and the message still shows as the last one received

### Requirement: Query and output
An address sent with `?` as its argument SHALL be answered with its current value to the sender. With OSC output on, openVJ SHALL send each supported address when its value changes, including clip connect state when a clip starts or stops playing.

#### Scenario: Fader feedback
- **GIVEN** OSC output is on
- **WHEN** the VJ drags layer 1's opacity to 25% with the mouse
- **THEN** openVJ sends `/composition/layers/1/video/opacity 0.25` to the output host

### Requirement: OSC learn
In map mode, an incoming OSC address SHALL be learnable onto any mappable control, the same way as a MIDI note or CC, with the same range and button behaviour. Learned OSC mappings SHALL be saved with the composition.

#### Scenario: TouchOSC fader
- **GIVEN** map mode is on and the master fader is clicked
- **WHEN** TouchOSC sends `/1/fader1 0.7`
- **THEN** `/1/fader1` moves the master from then on, after map mode is left too
