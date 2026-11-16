## ADDED Requirements

### Requirement: Discover layout
The layout menu SHALL offer a Discover layout, also reached with Cmd+4 (Ctrl+4 on Linux and Windows), for quickly reviewing library items and preparing presets. Discover is not for performing: nothing shown in it SHALL reach the output, Syphon or mapping, and whatever is playing there SHALL keep playing.

#### Scenario: Prep during a running show
- **WHEN** clips are playing to the output and the VJ switches to Discover and reviews items
- **THEN** the output keeps showing the same clips and never shows a Discover item

### Requirement: Review queue
Discover SHALL step through the library's Not reviewed items from the packs the VJ picks, in random or pack order, showing the current item large and live with audio, an "Up next" strip, how many remain, and how many were kept, saved as presets, skipped and marked unwanted in this session.

#### Scenario: Pick packs
- **WHEN** the VJ picks only MilkDrop in the From picker
- **THEN** only Not reviewed MilkDrop presets come up

### Requirement: Adjust and randomize
Discover SHALL show the current item's parameters as bars, with Randomize (R) setting every unlocked parameter to a random value within its range, a lock per parameter, Undo of the last change and Reset to the item's defaults.

#### Scenario: Randomize with a lock
- **WHEN** the VJ locks Hue shift and presses R
- **THEN** every other parameter changes and Hue shift keeps its value

### Requirement: Decide and move on
For the current item the VJ SHALL be able to set intensity (0–5 keys), tags and crates, then choose Keep (K), Save as preset (S), Skip (→) or Unwanted (X), after which the next item loads. Keep SHALL mark the original Kept without changing it. Save as preset SHALL keep the original and add a new preset with the current parameter values, intensity, tags and crates. Skip SHALL leave the item Not reviewed. Unwanted SHALL move it to the Unwanted crate.

#### Scenario: Save a variation
- **WHEN** the VJ randomizes a generator, likes the result and presses S
- **THEN** a new preset with those values appears in My library, the original generator is Kept with its default values, and the next item loads

#### Scenario: Skip for later
- **WHEN** the VJ presses → on an item
- **THEN** the next item loads and the skipped one is still Not reviewed
