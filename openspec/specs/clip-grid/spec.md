# clip-grid Specification

## Purpose

The clip grid is openVJ's main performance surface, modelled on Resolume's: rows are layers, columns are clip slots, and clicking a clip or column launches it.

## Requirements

### Requirement: Launch a clip
Clicking a clip SHALL launch it on its layer, replacing whatever that layer was playing, and select it.

#### Scenario: Click a clip
- **WHEN** the VJ clicks a clip in a layer's row
- **THEN** that clip starts playing on the layer and becomes the selected clip shown in Preview and the Clip tab

### Requirement: Select without launching
Right-clicking a clip or empty slot SHALL select it without launching it.

#### Scenario: Right-click a clip
- **WHEN** the VJ right-clicks a clip that is not playing
- **THEN** it becomes selected and shows in Preview and Properties, and the output does not change

### Requirement: Launch a column
Clicking a column header, or pressing 1 to 9, SHALL launch every clip in that column. Layers with an empty slot in that column SHALL be cleared, as in Resolume.

#### Scenario: Column with an empty slot
- **GIVEN** column 2 has clips on layers 1 and 3 and nothing on layer 2
- **WHEN** the VJ presses 2
- **THEN** layers 1 and 3 play their column-2 clips and layer 2 is cleared

### Requirement: Place sources from the browser
The browser SHALL offer Sources (generators, cameras, Syphon servers), Files and Effects tabs. Clicking an item picks it, and clicking a grid slot then places it; double-clicking an item places it into the selected slot.

#### Scenario: Place a file
- **WHEN** the VJ clicks a video in the Files tab and then an empty slot
- **THEN** a clip for that video appears in the slot

#### Scenario: Cancel a pick
- **WHEN** an item is picked and the VJ presses Esc
- **THEN** the pick is cancelled and no slot changes

### Requirement: Remove and clear
Del or Backspace SHALL remove the selected clip. Pressing Esc twice quickly with nothing picked SHALL clear all layers.

#### Scenario: Panic clear
- **WHEN** the VJ presses Esc twice quickly
- **THEN** every layer stops playing
