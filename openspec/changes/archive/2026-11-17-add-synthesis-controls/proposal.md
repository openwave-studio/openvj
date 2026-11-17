# Proposal

## Why

The Synthesis layout (PR #18) has a preview, 8 macros and one knob section per source and effect. The maintainer wants it to play like Synesthesia's control page (screenshot, 2026-11-17): group tools to lock, undo, reset and randomize; a fixed set of "meta" look controls every visual has; and a media overlay with its own transport, transform, colour and an image browser. Everything keeps openVJ's own look (openDJ style, one cyan accent) and its performance rules.

## What Changes

- **Two rows, as in Synesthesia.** SCENE CONTROLS holds the visual's own sections (SOURCE and one per effect); META CONTROLS holds the look every visual shares. MEDIA sits below.
- **Group tools.** Every Synthesis section (SOURCE, each FX, META, MEDIA's transform and colour) gets **lock**, **undo**, **default** and **random** buttons. Each row of sections (SCENE and META) also gets the four as row-wide buttons: random and default skip locked sections, undo steps back the last row-wide change. Same behaviour and seeded random as the Discover layout's Randomize, Undo and Reset.
- **META controls**, the same for every visual:
  - **Master** (the visual's intensity, fades it to black)
  - **Color**: Invert, Brightness, Gamma, Contrast, Hue, Saturation
  - **Mirror**: Vertical, Horizontal
  - **Limit colors**: on/off, Low colour, High colour (maps the image's brightness onto a two-colour gradient)
  - **General**: Alpha channel (alpha from brightness, so Syphon receivers can key it)

  Built as one hidden fixed effect, `look`, the same way the fixed Transform works: one shader pass, skipped when every value is at its default, so a visual that doesn't use META costs nothing. Its parameters are ordinary parameters, so they can take a driver (audio, tempo) and macros can move them.
- **MEDIA overlay**: an image, video or camera drawn over the visual with a **blend mode** (the layer blend modes) and an **amount** knob. The overlay is a small clip, so it reuses clip playback and the video decoder:
  - transport: play, previous, next (in the browser's current folder), stop; **loop mode** (the clip play modes), a **scrub bar**, **playback speed**
  - **transform**: Scale, Fit or Fill, Vertical flip, Horizontal flip
  - **colour**: Invert, Contrast, Gamma, Hue, Saturation (the same `look` effect, showing only these)
- **Media browser** in the MEDIA section: chips for All, Live sources (cameras), and each media folder the VJ adds; a thumbnail grid of images and videos; **refresh**, **add folder** and **lock** (while locked, clicks in the browser don't change the overlay, to avoid accidents during a show). Clicking a tile sets the overlay. It reuses the library's grid and thumbnail cache and the file browser's media listing; folders are saved in preferences.
- Knobs only, as Synthesis already decided; on/off controls are small switches; colours use a swatch with a colour picker popup.

Out of scope: Synesthesia's row of preset slots above the scene controls (presets exist; a slot row can follow), an XY pad for 2D inputs, per-generator custom sections, renaming macros, MIDI learn on the new controls (they become mappable when add-midi-learn grows clip targets), video audio, and the overlay in the Mixer's clip properties (it shows in Synthesis only for now; the data is shared, so the Mixer can follow).

## Capabilities

### Modified Capabilities
- `layouts`: group tools, META controls, MEDIA overlay and media browser in Synthesis.

## Touches

- crates/openvj-core: new `effects/layer_look.fs` (hidden), new `overlay.rs`, `effect.rs` (register it), `composition.rs` (Layer gets `look` and `overlay`)
- crates/openvj-render: `src/lib.rs` (look pass, overlay render and blend)
- crates/openvj-app: `src/app.rs` (Synthesis callbacks), new `src/synth_controls.rs`, new `src/media_browser.rs`, `src/prefs.rs` (media folders), `ui/synthesis.slint`, `ui/main.slint`, `ui/state.slint` (new scopes)
- Specs: layouts
- Shared hot spots: `composition.rs`, `app.rs`, `render/src/lib.rs`. Kept to small additions.

## Conflicts

- The library drag-and-drop thread is changing the library browser now. This change only reuses the grid tile component and the thumbnail cache; merge drag-and-drop first, then rebase.
- add-midi-learn (open): no file overlap beyond `app.rs`. Either order.
- add-parameter-animation: none, META parameters use the existing driver model.

## Open questions

Decided (the maintainer, 2026-11-17): META and the media overlay live on the **layer**, so they stay when the VJ switches visuals, as in Synesthesia. Macros (per visual) don't move them.

1. Master: a separate intensity knob in META, or the layer's existing Opacity moved into META? Built with the default: the layer's Opacity, shown as Master, so there is one control, not two. Easy to change if the maintainer prefers a separate knob.
