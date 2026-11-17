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

## ADDED Requirements

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
