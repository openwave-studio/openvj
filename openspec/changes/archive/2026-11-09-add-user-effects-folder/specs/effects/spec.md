# Spec Delta

## ADDED Requirements

### Requirement: Effect folders
openVJ SHALL load ISF effects from `~/Documents/openVJ/Effects` (created on first launch), from the standard ISF folders `~/Library/Graphics/ISF` and `/Library/Graphics/ISF` on macOS, and from `OPENVJ_EFFECTS_DIR` when set, including subfolders, like Resolume's Extra Effects folder. ISF files without an `inputImage` input are generators and SHALL NOT be listed as effects.

#### Scenario: Drop in a shader
- **WHEN** the VJ copies `Glitch.fs` into `~/Documents/openVJ/Effects` and starts openVJ
- **THEN** Glitch is listed in the Effects tab after the built-in effects and can be added to any chain

#### Scenario: VDMX shaders
- **WHEN** ISF shaders are installed in `~/Library/Graphics/ISF`
- **THEN** the filters among them appear in the Effects tab without any setup

### Requirement: Hot reload
openVJ SHALL notice new, changed and deleted effect files while running, within about a second, and update the Effects tab and every chain that uses them without a restart. Parameter values SHALL survive a reload.

#### Scenario: Edit a shader live
- **GIVEN** a folder effect is in a layer's chain
- **WHEN** the VJ saves a change to its `.fs` file
- **THEN** the layer shows the new shader on the next frames and the effect's parameter values are kept

### Requirement: Effect errors
An effect file that fails to parse or compile SHALL be listed in the Effects tab, dimmed, with the error, and SHALL NOT be addable. A chain that already holds it SHALL pass video through until the file is fixed.

#### Scenario: Broken shader
- **WHEN** a file in the effects folder has a GLSL error
- **THEN** the Effects tab shows it dimmed with the compiler message, and the rest of the effects work

### Requirement: Effect ids from folders
A folder effect's id SHALL be its `OPENVJ.ID` header key, or else its file name without `.fs`. A folder file SHALL NOT replace a built-in effect, and when two folder files share an id the one from the earlier folder wins; the other is listed as an error.

#### Scenario: Missing on another machine
- **WHEN** a composition that uses a folder effect opens on a machine without that file
- **THEN** the effect keeps its saved settings and passes video through, and starts working once the file is added
