## MODIFIED Requirements

### Requirement: Click previews, never outputs
Clicking an item in the library SHALL play it on the Preview monitor only. Dragging it onto a clip slot SHALL put it into that slot; double-clicking it or pressing Enter SHALL put it into the selected slot. Clicking an item SHALL NOT arm it for the next slot click. The output SHALL change only when a slot is launched, or when the slot an item is dropped on is already playing.

#### Scenario: Audition during a show
- **WHEN** clips are playing to the output and the VJ clicks a library tile
- **THEN** the Preview monitor shows that item and the output is unchanged

#### Scenario: Click a tile, then a slot
- **WHEN** the VJ clicks a library tile and then clicks a filled slot
- **THEN** the slot is selected or launched as usual and its clip is not replaced

## ADDED Requirements

### Requirement: Drag and drop
The VJ SHALL be able to drag a generator, MilkDrop preset or saved preset from the library grid, list or expanded library onto a clip slot to place it there, or onto a layer to place it in the layer's first empty slot. An effect SHALL be draggable onto a filled clip slot (clip effects), a layer (layer effects) or an effect chain in the properties panel. This follows Resolume's browser, where sources and effects are dragged onto clips and layers.

#### Scenario: Drag a generator onto a slot
- **WHEN** the VJ drags the Bars tile onto layer 1 column 6
- **THEN** Bars is in that slot, selected, and the status bar says "Placed Bars in Layer 1 · column 6"

#### Scenario: Drag an effect onto a layer
- **WHEN** the VJ drags an effect from the Effects tab onto layer 3's strip
- **THEN** the effect is added to the end of layer 3's effect chain and the Layer tab shows it

#### Scenario: Layer is full
- **WHEN** the VJ drops a generator on a layer with no empty slot
- **THEN** nothing changes and the status bar says to drop it on a slot instead

### Requirement: Drop hints
While dragging, the target under the cursor SHALL light up and say what a drop does, and targets that can't take the item SHALL not light up. The status bar SHALL say where the item can go.

#### Scenario: Hover a slot
- **WHEN** the VJ drags Bars over an empty slot
- **THEN** the slot lights up in the accent and reads "Place: Bars"

### Requirement: Drag out of the expanded library
Dragging an item out of the expanded library SHALL uncover the clip grid for the drop and bring the library back afterwards.

#### Scenario: Drag from the expanded library
- **GIVEN** the library is expanded over the clip grid
- **WHEN** the VJ starts dragging a tile
- **THEN** the clip grid shows for the drop, and the expanded library is back once the item is dropped or the drag is cancelled
