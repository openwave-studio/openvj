# output-mapping Specification

## Purpose

Projection mapping follows Resolume's Advanced Output: screens, slices, masks, warps and soft edges, imported from Resolume presets, drawn on one output window per screen and edited in the Output mapping window (screens and displays, corner pin, warp grids, input selection, masks, colour, soft edge).

## Requirements

### Requirement: Advanced Output model
The output setup SHALL model screens with slices (input and output rectangles or polygons), masks with bezier shapes, and warps: corner-pin, linear grid and bezier grid, matching Resolume's Slice Warper (bezier grid then homography).

#### Scenario: Bezier grid
- **GIVEN** a slice with a 4×4 PM_BEZIER warp grid
- **WHEN** it is evaluated
- **THEN** the grid acts as one bicubic bezier patch, as in Resolume

### Requirement: Import Advanced Output presets
Resolume Advanced Output presets and AdvancedOutput.xml (Arena 5 to 7) SHALL import into the model.

#### Scenario: Inverted masks
- **GIVEN** a screen with several masks with Invert on (Resolume's default)
- **WHEN** it is imported
- **THEN** the masks show their insides, combined as a union

### Requirement: Inspect a preset
`cargo run -p openvj-resolume --example output_info -- preset.xml` SHALL print what a preset imports as.

#### Scenario: CLI inspect
- **WHEN** a developer runs output_info on a preset
- **THEN** it lists screens, slices, warps and masks

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
openVJ SHALL have an Output mapping window, modelled on Resolume's Advanced Output, listing screens and the selected screen's slices and masks, with an Input selection stage and an Output transformation stage.

#### Scenario: Open mapping
- **WHEN** the VJ clicks Mapping in the top bar
- **THEN** the mapping window opens on Output transformation with the first screen selected and its slices drawn over its content

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

### Requirement: Several screens
The mapping window SHALL list every screen of the output setup and SHALL let the VJ add, rename, enable or disable, remove and pick a screen to edit, and choose the display each screen goes to, like Resolume's screen list and Output Device menu. Imported display screens SHALL go to the connected display with the same name.

#### Scenario: Second projector
- **WHEN** the VJ adds Screen 2 and sets it to a second display
- **THEN** a second output window opens fullscreen there, showing Screen 2's slices

#### Scenario: Imported two-screen preset
- **GIVEN** a Resolume preset with two display screens
- **WHEN** the VJ imports it
- **THEN** both screens are listed and each can be selected and edited

### Requirement: Edit warp points
In Output transformation the VJ SHALL set a slice's warp grid size (columns and rows, 2 to 32), switch the points between linear and bezier, drag any point, and in bezier mode drag its handles, with arrow-key nudging as for corners, like Resolume's warp points. The four corners SHALL keep acting as the perspective corner pin. Edited warps SHALL save in the same form Resolume files use, so an imported warp edits without changing until it is touched.

#### Scenario: Curve along a column
- **GIVEN** a slice on a curved column
- **WHEN** the VJ sets a 3×5 grid and drags the middle column of points sideways
- **THEN** the output bends smoothly through the moved points, live

#### Scenario: Imported bezier warp
- **GIVEN** an imported slice with a 4×4 bezier warp
- **WHEN** the VJ drags one handle
- **THEN** only the patch around that handle changes

### Requirement: Soft edge
Where the input areas of two enabled slices with Soft Edge on overlap, each SHALL fade across the overlap so the sum stays even, using the setup's Power, Luminance, Gamma and Red/Green/Blue gamma, as Resolume Arena's soft edge does. The mapping window SHALL have a Soft Edge switch per slice and the setup's soft-edge settings, and these SHALL import from Resolume files.

#### Scenario: Two blended projectors
- **GIVEN** Slice 1 taking the left 55% of the composition on Screen 1 and Slice 2 the right 55% on Screen 2, both with Soft Edge on
- **WHEN** a white clip plays
- **THEN** each projector fades out across the shared 10% and the overlap on the wall looks as bright as the rest

#### Scenario: Soft edge off
- **WHEN** Soft Edge is off on a slice
- **THEN** it draws hard edges even where it overlaps another slice

### Requirement: Outlines on the output
While the mapping window is open, an Outlines button SHALL draw every slice's outline and the selected point on the output windows, as Resolume does while editing Advanced Output. Closing the mapping window SHALL hide them.

#### Scenario: Line up on the wall
- **GIVEN** the mapping window is open with Outlines on
- **WHEN** the VJ drags a warp point
- **THEN** the projector shows the slice outlines and the point moving, on top of the content

#### Scenario: Show starts
- **WHEN** the VJ closes the mapping window
- **THEN** no outlines show on the output

### Requirement: Colour controls
The mapping window SHALL show Opacity, Brightness, Contrast and Red, Green and Blue for the selected screen, and Brightness, Contrast and Red, Green and Blue for the selected slice, as in Resolume's screen and slice settings, changing the output live.

#### Scenario: Match two projectors
- **GIVEN** the right projector looks bluer than the left
- **WHEN** the VJ lowers Blue on Screen 2
- **THEN** Screen 2's output loses blue, live, and the setting is saved
