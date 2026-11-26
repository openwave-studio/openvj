## ADDED Requirements

### Requirement: Envelope driver
A parameter SHALL be mappable to an Envelope with attack, decay, sustain and release (in milliseconds or beats), fired by a trigger: a MIDI note, the beat (every beat, every downbeat, or every 2 to 16 bars), an audio hit, or, with openDJ connected, a phrase start or hot cue. A new trigger SHALL restart the envelope from its current value.

#### Scenario: Pad punches the zoom
- **GIVEN** Scale is mapped to an Envelope on note C1 with attack 0, decay 200 ms, sustain 0, depth +50%
- **WHEN** the VJ hits the C1 pad hard
- **THEN** Scale jumps up by about 50% and falls back within 200 ms

#### Scenario: Flash on every downbeat
- **GIVEN** Brightness is mapped to an Envelope on every downbeat with decay 1/2 beat
- **WHEN** the music plays at 128 BPM
- **THEN** the image flashes on beat 1 of each bar and fades by beat 1.5

### Requirement: Velocity and gate
For a MIDI note trigger, the note's velocity SHALL scale the peak, and the sustain level SHALL hold while the note is held, releasing on note off.

#### Scenario: Held note
- **GIVEN** an Envelope on note D1 with sustain 60%
- **WHEN** the VJ holds D1 for two bars, then lets go
- **THEN** the parameter stays at 60% of its depth while held and falls over the release time after
