# Design

## Player playback beside the composition

**Choice:** the Player plays its playlist on two layers of its own (A and B, for the transition), rendered by the engine next to the composition, not inside it. While the Player is running, the output, Syphon and mapping show the Player; the composition stays exactly as it was and keeps its clips.

**Why:** a playlist is meant to be prepared and loaded separately from a composition. Writing presets into the composition's layers would change what gets saved with it, and stopping the Player would have to undo that. Two private layers keep both clean, and reuse the same clip, parameter and effect code.

**Consequences:**
- Pressing Automix makes the Player live. In Mixer and Synthesis an "AUTOMIX ■" chip shows in the top bar; clicking it stops Automix. Launching a clip in Mixer while the Player is live asks nothing: the Player stops and the composition takes the output back (mirrors how openDJ's Automix yields to a manual action).
- Switching layout never starts or stops the Player.

## Presets and the playlist file

A preset reuses the composition's saved types: one clip source (generator, ISF file, or media file), its parameter values and mappings, and its effect chain. The playlist file (`.ovjplaylist`, JSON, versioned like composition files) holds presets inline, so a playlist is one self-contained file. Generators are referred to by name and ISF path, like compositions do; a missing one shows as missing and is skipped.

A preset is a saved clip (the composition's own `Clip` type), so it carries source, parameter values, mappings, speed, Transform and effects with no new format. The first version accepts generators (built-in, ISF, MilkDrop); videos and images need the app's media loading to follow the Player's clips and come later.

A separate preset library (presets saved one by one and browsed) is left for later, if needed.

## Reactivity default

The maintainer was unsure whether the Player should add its own audio reactivity. Default: no hidden modulation. Each preset carries its own audio and tempo mappings (as Synesthesia scenes do), and the existing Reactivity control scales them all. This keeps a preset looking the same in the Player as where it was made. An "auto-react" option for presets without mappings can be added later if playlists feel static.

## Timing

Durations count bars from the shared tempo (BPM or Link), so changes land on bar lines. LENGTH and STYLE copy openDJ's Automix pickers: LENGTH has TIME (0 s cut, 2, 4, 8 s) and BARS (1, 2, 4) groups; STYLE starts with Crossfade, Cut and Fade through black.

## Synthesis macros

**Choice:** 8 macros per visual (clip), like Ableton racks and Synesthesia's controls. They live on the clip rather than the layer so a look keeps its macros when it becomes a preset or goes into a playlist. A macro is a value 0..1 with a list of targets; each target is one generator or effect parameter with a from/to range. The macro sets its targets directly, so a parameter targeted by a macro shows the macro's colour and can't also take its own driver (keeps the one-driver-per-parameter rule). The macro itself takes one driver, using add-parameter-animation's model.

**Why 8:** fits one row and matches most MIDI controllers' knob banks (Circuit Tracks has 8 macro knobs).

**Sections:** SOURCE lists the generator's ISF inputs in file order; FX one section per effect; OUTPUT opacity, blend and transform. A generator can later ship a sidecar layout (which inputs go where, section names) for a custom panel; until then every generator uses the standard one.
