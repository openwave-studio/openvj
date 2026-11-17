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
The browser SHALL offer Sources (generators, cameras, Syphon servers), Files and Effects tabs. Dragging an item onto a grid slot SHALL place it there, and dragging it onto a layer SHALL place it in the layer's first empty slot; double-clicking an item places it into the selected slot. Clicking a slot SHALL never place a browser item.

#### Scenario: Place a file
- **WHEN** the VJ drags a video from the Files tab onto an empty slot
- **THEN** a clip for that video appears in the slot

#### Scenario: Cancel a pick
- **WHEN** the VJ drags an item and lets go outside any slot or layer
- **THEN** no slot changes

### Requirement: Remove and clear
Del or Backspace SHALL remove the selected clip. Pressing Esc twice quickly with nothing picked SHALL clear all layers.

#### Scenario: Panic clear
- **WHEN** the VJ presses Esc twice quickly
- **THEN** every layer stops playing
