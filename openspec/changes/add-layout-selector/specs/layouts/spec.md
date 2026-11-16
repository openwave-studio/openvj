## ADDED Requirements

### Requirement: Layout selector
The top bar SHALL show, at its left, a layout selector like openDJ's: a button with the current layout's icon and name that opens a menu titled LAYOUT, listing Mixer, Player and Synthesis with an icon each and a check mark in the cyan accent on the current one.

#### Scenario: Switch layout from the menu
- **WHEN** the VJ opens the layout menu and picks Synthesis
- **THEN** the window shows the Synthesis layout and the button reads Synthesis

#### Scenario: Switch layout from the keyboard
- **WHEN** the VJ presses Cmd+2 (Ctrl+2 on Linux and Windows)
- **THEN** the window shows the Player layout

### Requirement: Switching keeps the show running
Switching layout SHALL change only what the main window shows. Output, Syphon, mapping, audio input, tempo, Link, MIDI and the composition SHALL keep running unchanged.

#### Scenario: Switch during a show
- **WHEN** clips are playing to the output and the VJ switches from Mixer to Player and back
- **THEN** the output never drops a frame to black and the same clips are still playing

### Requirement: Remembered layout
openVJ SHALL remember the last layout used and open in it next time. A first launch opens in Mixer.

#### Scenario: Reopen the app
- **WHEN** the VJ quits in Synthesis and opens openVJ again
- **THEN** it opens in Synthesis

### Requirement: Mixer layout
The Mixer layout SHALL be the full mixing screen: clip grid with deck tabs, monitors, master and crossfader, properties dock and browser.

#### Scenario: Default layout
- **WHEN** openVJ opens for the first time
- **THEN** it shows the Mixer layout, the same screen as before layouts existed

### Requirement: Player layout
The Player layout SHALL be a very simple automix player, modelled on openDJ's Automix: a playlist of presets that plays one after another by itself, with a large output preview, play/stop and next controls, LENGTH and STYLE pickers, and Shuffle and Repeat.

#### Scenario: Leave it running
- **WHEN** the VJ loads a playlist and presses Automix
- **THEN** openVJ plays each preset for its duration, then brings in the next with the chosen transition, without further input

#### Scenario: Same generator, several looks
- **WHEN** a playlist holds three presets of the same generator with different parameter values
- **THEN** each plays with its own values, mappings and effects

### Requirement: Presets
A preset SHALL capture a generator or clip with its parameter values, audio and tempo mappings, and effects. Adding a layer to the playlist SHALL capture what that layer is playing as a preset.

#### Scenario: Add the current look
- **WHEN** the VJ chooses "Add to playlist" on a layer playing a generator with two effects
- **THEN** the playlist gets a preset with that generator, its current parameter values, mappings and both effects

### Requirement: Play duration and transitions
Each preset SHALL play for a duration in bars (following BPM and Link) or seconds, 32 bars by default. The transition into the next preset SHALL use LENGTH (TIME: 0 s cut, 2, 4, 8 s; BARS: 1, 2, 4 bars) and STYLE (Crossfade, Cut, Fade through black, and later others), as in openDJ's Automix pickers.

#### Scenario: Bar-synced change
- **WHEN** a preset is set to 16 bars and LENGTH is 2 bars with Crossfade at 120 BPM
- **THEN** after 16 bars the next preset crossfades in over 2 bars, starting on a bar line

### Requirement: Playlist files
Playlists SHALL save to and open from their own files (`.ovjplaylist`, JSON), independent of compositions, so they can be prepared before a show. The file SHALL hold the presets in order, each preset's duration, LENGTH, STYLE, Shuffle and Repeat.

#### Scenario: Prepare for an event
- **WHEN** the VJ saves a playlist at home and opens it at the venue on another machine with the same generators
- **THEN** the same presets play in the same order with the same settings

### Requirement: Player keeps reactivity simple
Each preset SHALL react to the music through its own audio and tempo mappings, and the existing Reactivity control SHALL scale all of them at once. The Player SHALL NOT add hidden modulation of its own.

#### Scenario: Turn reactivity down
- **WHEN** the VJ lowers Reactivity while the Player runs
- **THEN** every preset reacts less to the music

### Requirement: Synthesis layout
The Synthesis layout SHALL be a zoomed-in view of one layer, laid out like a hardware synthesizer: a strip of layer thumbnails to pick the layer, a large preview, a row of 8 macros, and sections that are the same for every generator: SOURCE (the generator's parameters), one section per effect, and OUTPUT (opacity, blend mode, transform). The clip grid SHALL be hidden.

Macros and section parameters in Synthesis SHALL be standardized knobs; elsewhere parameters stay bars.

#### Scenario: Focus a layer
- **WHEN** the VJ clicks the second layer's thumbnail in Synthesis
- **THEN** the panel shows that layer's generator, effects and macros

### Requirement: Macros
Each layer SHALL have 8 macros. A macro SHALL move any number of the layer's generator and effect parameters, each over its own range, and SHALL be playable with the mouse and mappable to MIDI. A macro SHALL take one driver (LFO, audio, tempo) like any parameter. Macros SHALL save with the composition and inside presets.

#### Scenario: One knob, many parameters
- **WHEN** macro 1 targets the generator's Zoom from 1 to 4 and a Blur effect's amount from 0 to 0.6, and the VJ turns it halfway
- **THEN** Zoom is 2.5 and Blur is 0.3

#### Scenario: Animate a macro
- **WHEN** the VJ sets macro 1's driver to a tempo sine over 4 bars
- **THEN** every parameter it targets moves together with the beat

### Requirement: Presets from Synthesis
Synthesis SHALL offer "Save preset" and "Add to playlist" for the focused layer, capturing its generator, parameter values, mappings, macros and effects.

#### Scenario: Shape a look, then queue it
- **WHEN** the VJ shapes a look in Synthesis and chooses "Add to playlist"
- **THEN** the Player's playlist gets that look as a new preset
