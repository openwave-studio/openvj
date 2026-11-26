## ADDED Requirements

### Requirement: Connect to openDJ
With the openDJ switch on, openVJ SHALL find openDJ on this computer or the local network through Bonjour, subscribe to its beat, phrase, track and cue events over OSC, and show whether it is connected. It SHALL reconnect by itself when openDJ restarts. With the switch off, no port SHALL be open.

#### Scenario: openDJ on the same Mac
- **GIVEN** openDJ runs with OSC on and the openDJ switch is on in openVJ
- **WHEN** a deck plays
- **THEN** openVJ shows openDJ as connected, with the current phrase and bar

#### Scenario: openDJ restarts
- **WHEN** openDJ quits and opens again
- **THEN** openVJ connects again without the VJ doing anything

### Requirement: Beats and phrases
openVJ SHALL keep the master deck's beat in the bar, bar in the phrase and current phrase (name and length in bars) from openDJ's `/beat` and `/phrase` events. Tempo and beat phase SHALL keep coming from Ableton Link or MIDI clock.

#### Scenario: A drop starts
- **WHEN** openDJ sends `/phrase "Drop" 16`
- **THEN** openVJ shows Drop and counts its 16 bars

### Requirement: Track and key
openVJ SHALL keep the title, artist, BPM and key of the track on the master deck, from `/deck/N/track`.

#### Scenario: New track
- **WHEN** a track in 8A loads and becomes the master deck
- **THEN** openVJ shows its title and key

### Requirement: Phrase triggers
In MIDI map mode, a phrase name (Intro, Build, Drop, Breakdown, Chorus, Verse, Outro) or a hot cue SHALL be learnable like a pad, to launch a column or clip or to act as Autopilot's next, as Resolume users map openDJ's `/phrase` to clip triggers.

#### Scenario: Drop launches a column
- **GIVEN** Drop is learned to column 3
- **WHEN** openDJ starts a Drop phrase
- **THEN** column 3 launches on that downbeat
