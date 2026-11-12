## MODIFIED Requirements

### Requirement: Map a parameter
Every normalised parameter (clip parameters, layer opacity, master, effect parameters and mix) SHALL have a map ▾ button that maps it to an audio band level (Bass, Low mid, High mid, Treble), the overall Level, audio hits (Bass hits, Low mid hits, High mid hits, Treble hits, Hits) or a tempo shape (Sine, Saw, Square, Pulse over ¼ to 16 beats), with a depth from −100% to +100%. Audio sources are scaled by the composition's Reactivity.

#### Scenario: Tempo shape
- **WHEN** layer opacity is mapped to Sine over 4 beats with 50% depth
- **THEN** opacity swings by up to half its range once every 4 beats

#### Scenario: Flash on kicks
- **WHEN** an effect's mix is mapped to Bass hits with 100% depth from 0
- **THEN** the effect flashes in on each kick and fades out between them
