# Proposal

## Why

With thousands of generators and presets, the maintainer wants the main library to hold only what a VJ has chosen, and a separate place to sort through the rest: "let's find some new generators" as a 30-minute task of testing, adjusting, adding to a crate and adding metadata. The maintainer (2026-11-16) set the roles of the layouts: **Discover** is a quick search-through-and-prep-presets layout, not for performance; **Synthesis** is for performing one visualization; **Mixer** is for combining layers and mixing more than moving one scene's parameters.

## What Changes

- A fourth layout, **Discover**, in the layout menu (Cmd/Ctrl+4), like openDJ's Prep layout.
- It works through the library's **Not reviewed** items, from the packs the VJ picks (e.g. Ghost Arcade + MilkDrop), in random or pack order, with a progress bar and this session's counts.
- The current item plays large, **live with audio**, on Discover's own preview. Nothing in Discover reaches the output, Syphon or mapping; the composition and the Player keep playing there.
- A parameter panel for the item's inputs (Resolume-style bars, as everywhere outside Synthesis), with **Randomize** (R) within each parameter's range, a **lock** per parameter that Randomize skips, **Undo** and **Reset**.
- Intensity 0–5 (number keys), tags and crates, as in the library.
- Four decisions, each loading the next item: **Keep** (K) marks the original kept, unchanged; **Save as preset** (S) keeps the original and adds a new preset with the current values; **Skip** (→) leaves it Not reviewed for later; **Unwanted** (X) moves it to the Unwanted crate.
- An "Up next" strip of the following items' thumbnails.

Out of scope: performing from Discover, MIDI mapping of its controls, comparing two items side by side, smart suggestions of what to review.

## Capabilities

### New Capabilities
- None.

### Modified Capabilities
- `layouts`: adds the Discover layout.

## Impact

UI and a small preview renderer reuse; all data goes through the library from `add-library-browser`.

## Touches

- crates/openvj-app: `ui/layouts.slint` (Discover screen, menu entry), `ui/main.slint` (selector entry), `src/app.rs` (Discover state, keys), new `src/discover.rs` (queue, randomize, undo)
- crates/openvj-render: render one item to Discover's preview beside the composition (same path as the Player's private layers)
- Specs: `layouts`
- Shared hot spots: `ui/main.slint`, `src/app.rs`, `crates/openvj-render/src/lib.rs`

## Conflicts

- **add-library-browser**: Discover needs its statuses, tags, crates and presets. Merge that first (at least its task group 1).
- **add-layout-selector** / **PR #14 (Player)**: same layout menu and `layouts.slint`; the `layouts` spec is created there. Merge #14 first and archive add-layout-selector before archiving this change.

## Open questions

1. Menu order: Discover first like openDJ's Prep (Discover, Mixer, Player, Synthesis, which renumbers the shortcuts) or added last as Cmd+4 (proposed, keeps today's shortcuts)?
2. Should Discover also review **Kept** items again (e.g. a "My library" source in its From picker) to re-tag and make presets, or only Not reviewed ones? Proposed: both, Not reviewed by default.
