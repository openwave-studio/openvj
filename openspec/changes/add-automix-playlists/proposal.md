## Why

Seba shared Synesthesia's automix playlist (2026-11-17): a picker of named playlists with a scene count, play / previous / next / shuffle, a Playlist Editor, and a numbered list where each row shows a thumbnail, the scene name and the preset name. openVJ's Player layout (PR #14) already plays a playlist of presets by itself with bar-synced transitions, but it is one playlist at a time opened through a file dialog, rows are text only, there is no Previous, and you can't jump to a row. It is built for editing, not for glancing at during a show.

This change makes the Player feel like that: pick a playlist by name, see what is coming with thumbnails, jump around, and edit in a separate mode. It extends the Player and its `.ovjplaylist` format; it does not add a second playlist system. In openVJ's own style (openDJ look, cyan accent), not Synesthesia's.

## What changes

- **Playlists folder and picker.** Playlists live in `~/Documents/openVJ/Playlists` (next to `Presets`). The Player's header gets a dropdown of every playlist there, by name, with "N presets" beside it, plus New. Switching while Automix runs brings the first preset of the new list in with the current transition. Open/Save dialogs stay for files elsewhere. Changes save on their own (convention over configuration).
- **Transport.** Play/Stop, Previous, Next, Shuffle in one row. Previous goes back to the preset that played before (history, so it also works with Shuffle).
- **Rows like a show list.** Number, thumbnail, the look's name and below it the source (generator) name. The current row is marked in cyan with a thin progress bar showing time left. Clicking a row brings it in with the current transition.
- **Thumbnails.** Still, taken from the library's thumbnail cache (PR #19) or captured once from the output when a preset is added. Hovering a row plays that preset live in its thumbnail (one small preview at a time, through the library's audition path), so the Player costs nothing extra unless the mouse is on a row.
- **Advance per song.** Besides bars and seconds, a preset can play "Until the song changes". openVJ detects a song change from the audio: a gap of silence, a tempo jump on the tempo source (Link, MIDI clock, tap), or a lasting change in the sound's character (spectrum and energy over the last ~30 s). A minimum time (16 bars) stops it changing on every breakdown, and a maximum (5 min) keeps it moving if no change is found. Next stays MIDI-mappable for a sure hit.
- **Let it run.** The existing full-screen output works from the Player; a "Full screen" button sits in its transport row.
- **Edit mode.** An "Edit playlist" toggle inside the Player shows the editing controls that are inline today (duration, reorder, remove, Add, LENGTH, STYLE, Repeat). Out of edit mode the list is for playing only, so a stray click can't change it.
- **Add from the library.** Library and Discover get "Add to playlist" for presets and generators, alongside the existing Mixer and Synthesis entries.

Out of scope: videos and images in presets (already deferred), advance on drops or phrases inside a song, Synesthesia `.synScene` import, sharing playlists between machines beyond the file.

## Touches

- `crates/openvj-audio/src/lib.rs` (song-change detector, additive)
- `crates/openvj-core/src/playlist.rs` (Span gains "Song", history for Previous, jump to index, thumbnail path per preset; format stays version 1, new fields optional)
- `crates/openvj-app/src/player.rs`, `crates/openvj-app/ui/layouts.slint` (Player UI)
- `crates/openvj-app/src/library.rs`, `crates/openvj-app/ui/library.slint`, `crates/openvj-app/ui/discover.slint` (Add to playlist)
- Shared hot spots: `crates/openvj-app/src/app.rs` and `crates/openvj-app/ui/main.slint` (callback wiring only)
- Spec: `layouts` (Player requirements)
- README "What works"

## Conflicts

No open PRs right now. Parallel threads: library drag and drop (`library.slint`, `library.rs`) and Synthesis controls plus a media image browser (`synthesis.slint`, maybe `layouts.slint`/`main.slint`). Suggested order: whichever lands first; this change keeps its `library.slint` edit to one menu item and its `main.slint` edit to wiring, so rebasing is small. No other change in openspec/changes/ touches the Player.

## Decisions (Seba, 2026-11-17)

1. Edit mode is a toggle inside the Player; go full screen and let it run.
2. Advance options: at the end of each song, or bars, or seconds.
3. Still thumbnails that animate on hover.
4. Switching playlists while playing transitions in.

## Open questions for Seba

1. Song-change detection is a heuristic. Default chosen: silence gap OR tempo jump OR lasting change in sound, within 16 bars to 5 min. Does that fit how you play (continuous DJ mixes with long blends)?
