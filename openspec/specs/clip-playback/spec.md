# clip-playback Specification

## Purpose

Clip playback follows Resolume's clip transport: each clip has a play mode and plays at its own speed or synced to the beat.

## Requirements

### Requirement: Play modes
Each clip SHALL have a play mode: Loop, Bounce, Random, Play once & clear, or Play once & hold.

#### Scenario: Play once and clear
- **GIVEN** a clip set to Play once & clear
- **WHEN** it reaches its end
- **THEN** its layer is cleared

#### Scenario: Bounce
- **GIVEN** a video clip set to Bounce
- **WHEN** it reaches its end
- **THEN** it plays backwards to the start, then forwards again

### Requirement: Beat sync
A clip SHALL either play at its own speed (Timeline) or in beat sync, where one pass lasts a set number of beats from ¼ to 256 at the current tempo, following Ableton Link when it is on. This matches Resolume's BPM Sync transport.

#### Scenario: Tempo change
- **GIVEN** a clip synced to 16 beats
- **WHEN** the tempo changes from 120 to 140 BPM
- **THEN** one pass of the clip still lasts 16 beats

### Requirement: Durations for stills and generators
Stills and generators SHALL have a duration so that play modes that need an end work for them.

#### Scenario: Still with play once
- **GIVEN** an image clip with a 4 s duration set to Play once & hold
- **WHEN** it is launched
- **THEN** it holds after 4 s

### Requirement: Layer transitions
Each layer SHALL have a transition time, and launching a clip on a playing layer SHALL crossfade from the old clip over that time.

#### Scenario: Crossfade
- **GIVEN** a layer with a 1 s transition playing clip A
- **WHEN** clip B is launched on it
- **THEN** A fades into B over 1 s

### Requirement: Frame-accurate clock
Clip time SHALL come from one composition clock, so speed changes, retriggers and crossfades stay frame-accurate, and video drops frames rather than drifting.

#### Scenario: Slow decode
- **WHEN** video decoding falls behind
- **THEN** frames are skipped and the clip stays in time
