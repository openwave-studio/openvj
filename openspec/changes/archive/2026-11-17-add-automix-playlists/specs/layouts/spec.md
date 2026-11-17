## MODIFIED Requirements

### Requirement: Player layout
The Player layout SHALL be a very simple automix player, modelled on openDJ's Automix and Synesthesia's automix playlist: a playlist of presets that plays one after another by itself, with a large output preview, a playlist picker, a transport row (Play/Stop, Previous, Next, Shuffle), and a numbered list of presets. Each row SHALL show a thumbnail, the preset's name and its source's name; the playing row SHALL be marked with the accent colour and show the time left.

#### Scenario: Leave it running
- **WHEN** the VJ picks a playlist and presses Play
- **THEN** openVJ plays each preset for its duration, then brings in the next with the chosen transition, without further input

#### Scenario: Same generator, several looks
- **WHEN** a playlist holds three presets of the same generator with different parameter values
- **THEN** each plays with its own values, mappings and effects, and each row shows the same source name under its own preset name

#### Scenario: Jump to a preset
- **WHEN** the VJ clicks row 5 while row 2 plays
- **THEN** preset 5 comes in with the current transition and the playlist continues from there

#### Scenario: Go back
- **WHEN** Shuffle is on and the VJ presses Previous
- **THEN** the preset that played before the current one comes back in

### Requirement: Playlist files
Playlists SHALL save to and open from their own files (`.ovjplaylist`, JSON), independent of compositions, so they can be prepared before a show. The file SHALL hold the presets in order, each preset's duration, LENGTH, STYLE, Shuffle and Repeat. Playlists in `~/Documents/openVJ/Playlists` SHALL be listed by name in the Player's picker with their preset count, and changes to them SHALL save without a Save step.

#### Scenario: Prepare for an event
- **WHEN** the VJ saves a playlist at home and opens it at the venue on another machine with the same generators
- **THEN** the same presets play in the same order with the same settings

#### Scenario: Switch playlist during a show
- **WHEN** Automix plays "Warm up" and the VJ picks "Peak time" in the picker
- **THEN** the first preset of "Peak time" comes in with the current transition

### Requirement: Play duration and transitions
Each preset SHALL play for a duration in bars (following BPM and Link), in seconds, or until the song changes; 32 bars by default. The transition into the next preset SHALL use LENGTH (TIME: 0 s cut, 2, 4, 8 s; BARS: 1, 2, 4 bars) and STYLE (Crossfade, Cut, Fade through black, and later others), as in openDJ's Automix pickers.

#### Scenario: Bar-synced change
- **WHEN** a preset is set to 16 bars and LENGTH is 2 bars with Crossfade at 120 BPM
- **THEN** after 16 bars the next preset crossfades in over 2 bars, starting on a bar line

## ADDED Requirements

### Requirement: Advance when the song changes
A preset set to "Song" SHALL play until openVJ detects a new song in the audio input: a gap of silence, a jump in the tempo source's BPM, or a lasting change in the sound's spectrum and energy. It SHALL play at least 16 bars and at most 5 minutes.

#### Scenario: Next track in a set with gaps
- **WHEN** a preset is set to Song and the music stops for a second before the next track
- **THEN** the next preset comes in as the new track starts

#### Scenario: Long breakdown
- **WHEN** a track drops to a quiet breakdown for 8 bars and comes back
- **THEN** the preset keeps playing

### Requirement: Animated thumbnails on hover
Each row's thumbnail SHALL be a still image, and SHALL play that preset live while the mouse is over it.

#### Scenario: Peek at a preset
- **WHEN** the VJ hovers row 4 while row 2 plays
- **THEN** row 4's thumbnail animates and the output is unchanged


### Requirement: Playlist edit mode
The Player SHALL show editing controls (duration, reorder, remove, Add, LENGTH, STYLE, Repeat) only in edit mode, toggled by "Edit playlist". Outside edit mode a click on a row SHALL only play it.

#### Scenario: Safe during a show
- **WHEN** edit mode is off and the VJ clicks a row
- **THEN** the row plays and the playlist is unchanged

### Requirement: Add to playlist from the library
The library and the Discover layout SHALL offer "Add to playlist" for presets and generators, adding them to the playlist shown in the Player.

#### Scenario: Queue from Discover
- **WHEN** the VJ keeps a look in Discover and chooses "Add to playlist"
- **THEN** the Player's playlist ends with that look, with its thumbnail
