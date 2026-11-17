## ADDED Requirements

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
