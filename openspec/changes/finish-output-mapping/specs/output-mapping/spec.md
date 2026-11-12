# Spec Delta

## ADDED Requirements

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

### Requirement: Colour controls
The mapping window SHALL show Opacity, Brightness, Contrast and Red, Green and Blue for the selected screen, and Brightness, Contrast and Red, Green and Blue for the selected slice, as in Resolume's screen and slice settings, changing the output live.

#### Scenario: Match two projectors
- **GIVEN** the right projector looks bluer than the left
- **WHEN** the VJ lowers Blue on Screen 2
- **THEN** Screen 2's output loses blue, live, and the setting is saved

## MODIFIED Requirements

### Requirement: Output mapping window
openVJ SHALL have an Output mapping window, modelled on Resolume's Advanced Output, listing screens and the selected screen's slices and masks, with an Input selection stage and an Output transformation stage.

#### Scenario: Open mapping
- **WHEN** the VJ clicks Mapping in the top bar
- **THEN** the mapping window opens on Output transformation with the first screen selected and its slices drawn over its content
