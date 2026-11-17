## MODIFIED Requirements

### Requirement: Place sources from the browser
The browser SHALL offer Sources (generators, cameras, Syphon servers), Files and Effects tabs. Dragging an item onto a grid slot SHALL place it there, and dragging it onto a layer SHALL place it in the layer's first empty slot; double-clicking an item places it into the selected slot. Clicking a slot SHALL never place a browser item.

#### Scenario: Place a file
- **WHEN** the VJ drags a video from the Files tab onto an empty slot
- **THEN** a clip for that video appears in the slot

#### Scenario: Cancel a pick
- **WHEN** the VJ drags an item and lets go outside any slot or layer
- **THEN** no slot changes
