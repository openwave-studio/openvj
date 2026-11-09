## ADDED Requirements

### Requirement: Mapped output
The output SHALL be drawn through the output setup, as in Resolume's Advanced Output: each enabled slice takes its input area from the composition and draws it on its screen through its warp, and screen and slice masks hide what they cover. Without a saved setup the screen shows the whole composition unwarped.

#### Scenario: Imported preset
- **GIVEN** a Resolume preset with two slices, one corner-pinned and one with a bezier warp, and an inverted mask
- **WHEN** the VJ imports it and plays a clip
- **THEN** the output window shows both slices warped as in Resolume, and black outside the mask

#### Scenario: No mapping
- **GIVEN** no output setup was ever saved
- **WHEN** a clip plays
- **THEN** the output shows the composition as before

### Requirement: Output mapping window
openVJ SHALL have an Output mapping window, modelled on Resolume's Advanced Output, listing screens, slices and masks, with an Input selection stage and an Output transformation stage.

#### Scenario: Open mapping
- **WHEN** the VJ clicks Mapping in the top bar
- **THEN** the mapping window opens on Output transformation with Screen 1's slices drawn over its content

### Requirement: Corner pin
In Output transformation the VJ SHALL pin a slice by dragging its four corners, with the content following in correct perspective, and SHALL be able to nudge a corner with the arrow keys (Shift for 10 px) or type its position in screen pixels.

#### Scenario: Pin to a wall
- **GIVEN** a slice filling the screen
- **WHEN** the VJ drags its top-right corner down and left
- **THEN** the output shows the slice as a perspective quad with that corner moved, live

#### Scenario: Nudge
- **GIVEN** a selected corner
- **WHEN** the VJ presses Shift+Right
- **THEN** the corner moves 10 screen pixels right

### Requirement: Input selection
In Input selection the VJ SHALL move and resize the rectangle of the composition each slice shows, as in Resolume's Input Selection.

#### Scenario: Crop the composition
- **WHEN** the VJ drags a slice's input rectangle to the left half of the composition
- **THEN** that slice shows only the left half, still pinned where it was

### Requirement: Edit slices and masks
The VJ SHALL add, rename, reorder, enable or disable, and delete slices and masks, edit a mask's points, and invert a mask.

#### Scenario: Mask out a door
- **GIVEN** a slice covering a wall with a door
- **WHEN** the VJ adds a mask, places its points around the door and turns Invert off
- **THEN** the door stays black on the output

### Requirement: Setup saved per machine
The output setup SHALL be saved in openVJ's settings, apart from compositions, and restored on launch, as Resolume keeps Advanced Output per machine. Resolume Advanced Output presets SHALL import from the mapping window.

#### Scenario: Same venue, new show
- **GIVEN** a mapped setup
- **WHEN** the VJ quits, relaunches and opens another composition
- **THEN** the output is mapped the same way

### Requirement: Test card
The mapping window SHALL offer a test card that replaces the composition on the output while it is on, so slices can be lined up without content.

#### Scenario: Line up
- **WHEN** the VJ turns on Test card
- **THEN** each slice shows a grid with its name until Test card is turned off
