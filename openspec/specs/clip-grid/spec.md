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

### Requirement: Move and copy clips by dragging
The VJ SHALL be able to drag a clip onto another slot to move it there; when that slot holds a clip, the two clips SHALL swap. Holding Ctrl while dragging SHALL copy the clip, with its parameters and effects, instead. Dropping a clip on a layer's strip SHALL move it to that layer's first empty slot. This follows Resolume, where clips are dragged between slots.

#### Scenario: Swap two clips
- **WHEN** the VJ drags Kaleido onto the slot holding Bars
- **THEN** Kaleido is where Bars was and Bars is where Kaleido was

#### Scenario: Copy a clip
- **WHEN** the VJ drags Rings onto an empty slot while holding Ctrl
- **THEN** both slots hold Rings with the same settings

### Requirement: Files from the Finder
The VJ SHALL be able to drop image and video files from the Finder onto a slot: the first file into that slot and the next ones into the columns to its right. Dropped on a layer, they SHALL start at the layer's first empty slot. Files openVJ can't play SHALL be skipped and counted in the status bar. While files hover, the target under the cursor SHALL light up as for a drag inside openVJ.

#### Scenario: Drop two videos
- **WHEN** the VJ drops two videos from the Finder onto layer 1 column 1
- **THEN** the first video is in column 1, the second in column 2, and both load
