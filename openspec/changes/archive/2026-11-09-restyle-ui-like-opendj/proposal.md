# Proposal

## Why

openVJ should look like a sibling of openDJ (the maintainer's DJ app), so the two read as one family, while keeping the Resolume-style elements a VJ relies on.

## What Changes

- Colours, type and sizes move to theme tokens matched to openDJ: near-black panels, flat dark controls, Inter Tabular, Apple system colours for states.
- One accent, cyan #4fe6da, replaces openDJ's lilac. Playing clips light in the accent.
- Buttons, tabs, faders and menus follow openDJ; parameters stay Resolume-style bars and the clip grid keeps spaced cells.
- A Settings window like openDJ's.

Out of scope: layout changes beyond restyling, new features.

## Capabilities

### New Capabilities
- `visual-style`: how openVJ looks and where its styling lives.

### Modified Capabilities

## Impact

Implemented in PR #10 (open). Archive this change in that PR, or right after it merges.

## Touches

- crates/openvj-app/ui: `theme.slint`, `controls.slint`, `main.slint`, `properties.slint`, new `settings.slint`, `fonts/`
- Specs: visual-style (new)
- Shared hot spots: theme.slint, main.slint

## Conflicts

- Any change that adds UI (MIDI, decks, live inputs, FX cards) must use the theme tokens once this lands. Merge this early so later UI work builds on it.

## Open questions

None. The maintainer chose the accent, playing colour, parameter bars and grid spacing on 2026-11-09.
