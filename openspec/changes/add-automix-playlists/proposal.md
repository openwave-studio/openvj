## Why

Seba shared Synesthesia's automix playlist (2026-11-17): a picker of named playlists with a scene count, play / previous / next / shuffle, a Playlist Editor, and a numbered list where each row shows a thumbnail, the scene name and the preset name. openVJ's Player layout (PR #14) already plays a playlist of presets by itself with bar-synced transitions, but it is one playlist at a time opened through a file dialog, rows are text only, there is no Previous, and you can't jump to a row. It is built for editing, not for glancing at during a show.

This change makes the Player feel like that: pick a playlist by name, see what is coming with thumbnails, jump around, and edit in a separate mode. It extends the Player and its `.ovjplaylist` format; it does not add a second playlist system. In openVJ's own style (openDJ look, cyan accent), not Synesthesia's.

## What changes

- **Playlists folder and picker.** Playlists live in `~/Documents/openVJ/Playlists` (next to `Presets`). The Player's header gets a dropdown of every playlist there, by name, with "N presets" beside it, plus New. Switching while Automix runs brings the first preset of the new list in with the current transition. Open/Save dialogs stay for files elsewhere. Changes save on their own (convention over configuration).
- **Transport.** Play/Stop, Previous, Next, Shuffle in one row. Previous goes back to the preset that played before (history, so it also works with Shuffle).
- **Rows like a show list.** Number, thumbnail, the look's name and below it the source (generator) name. The current row is marked in cyan with a thin progress bar showing time left. Clicking a row brings it in with the current transition.
- **Thumbnails.** Static, taken from the library's thumbnail cache (PR #19) or captured once from the output when a preset is added, saved next to the playlist. No live previews, so the Player costs nothing extra per frame.
- **Edit mode.** An "Edit playlist" toggle shows the editing controls that are inline today (duration, reorder, remove, Add, LENGTH, STYLE, Repeat). Out of edit mode the list is for playing only, so a stray click can't change it.
- **Add from the library.** Library and Discover get "Add to playlist" for presets and generators, alongside the existing Mixer and Synthesis entries.

Out of scope: videos and images in presets (already deferred), audio-triggered advance (drops, phrases), live row previews, Synesthesia `.synScene` import, sharing playlists between machines beyond the file.

## Touches

- `crates/openvj-core/src/playlist.rs` (history for Previous, jump to index, thumbnail path per preset; format stays version 1, new fields optional)
- `crates/openvj-app/src/player.rs`, `crates/openvj-app/ui/layouts.slint` (Player UI)
- `crates/openvj-app/src/library.rs`, `crates/openvj-app/ui/library.slint`, `crates/openvj-app/ui/discover.slint` (Add to playlist)
- Shared hot spots: `crates/openvj-app/src/app.rs` and `crates/openvj-app/ui/main.slint` (callback wiring only)
- Spec: `layouts` (Player requirements)
- README "What works"

## Conflicts

No open PRs right now. Parallel threads: library drag and drop (`library.slint`, `library.rs`) and Synthesis controls plus a media image browser (`synthesis.slint`, maybe `layouts.slint`/`main.slint`). Suggested order: whichever lands first; this change keeps its `library.slint` edit to one menu item and its `main.slint` edit to wiring, so rebasing is small. No other change in openspec/changes/ touches the Player.

## Open questions for Seba

1. Edit mode: a toggle in the Player (proposed), or a separate Playlist Editor window like Synesthesia?
2. Advance: keep bars/seconds per preset (proposed), or also allow advancing on the music (e.g. after a drop or a quiet section)?
3. Thumbnails: static (proposed, cheapest) or small live previews of the next one or two presets?
4. Switching playlists while playing: transition in (proposed), or only switch when stopped?
