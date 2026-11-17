## ADDED Requirements

### Requirement: Synthesis group tools
Each Synthesis section SHALL have lock, undo, default and random buttons, and each row of sections SHALL have the same four for the whole row, like Synesthesia's scene and meta controls. Random SHALL set every parameter of the section to a random value within its range; row-wide random and default SHALL skip locked sections; undo SHALL step back through the section's (or row's) last changes, including knob drags.

#### Scenario: Randomize the scene, keep the colour
- **WHEN** the VJ locks the FX · Color section and presses the SCENE row's random
- **THEN** the source and every other effect section change, and FX · Color keeps its values

#### Scenario: Undo a random
- **WHEN** the VJ presses random on SOURCE and then undo on SOURCE
- **THEN** SOURCE is back to the values it had before

### Requirement: Scene and meta controls
Synthesis SHALL split its controls into SCENE CONTROLS (the visual's own source and effect sections) and META CONTROLS, which belong to the layer and stay when the VJ switches the visual the layer plays, as in Synesthesia.

#### Scenario: Look survives a switch
- **WHEN** the VJ sets Hue to 90° in META and then launches another visual on the same layer
- **THEN** the new visual shows with the same hue shift

### Requirement: Synthesis META controls
META SHALL hold: Master; Color (Invert, Brightness, Gamma, Contrast, Hue, Saturation); Mirror (Vertical, Horizontal); Limit colors (on/off, Low colour, High colour); and General (Alpha channel). At their defaults they SHALL leave the picture unchanged and cost no extra GPU pass. Each META control SHALL take a driver like any parameter.

#### Scenario: Two-colour look
- **WHEN** the VJ turns Limit colors on with Low dark blue and High orange
- **THEN** dark parts of the visual turn dark blue, bright parts orange, and tones in between blend

#### Scenario: Alpha for Syphon
- **WHEN** the VJ turns Alpha channel on and a Syphon receiver reads openVJ
- **THEN** black areas are transparent in the receiver

### Requirement: Synthesis media overlay
Synthesis SHALL offer a MEDIA section, belonging to the layer like META, that draws an image, video or camera over the visual with a blend mode (the layer blend modes) and an amount. It SHALL have play, previous, next and stop, a loop mode (the clip play modes), a scrub bar and playback speed for videos; a transform with Scale, Fit or Fill, and vertical and horizontal flip; and colour controls Invert, Contrast, Gamma, Hue and Saturation.

#### Scenario: Logo over a generator
- **WHEN** the VJ picks a PNG logo in the media browser with blend Add and amount 0.8
- **THEN** the logo shows over the visual in the preview and the output

#### Scenario: Next video
- **WHEN** a video from a folder is the overlay and the VJ presses next
- **THEN** the next video in that folder plays as the overlay

### Requirement: Media browser
The MEDIA section SHALL include a browser with chips for All, Live sources and each media folder the VJ added, a grid of thumbnails, refresh, add folder and lock. Clicking a tile SHALL set the overlay; while lock is on, clicks SHALL NOT change it. Added folders SHALL be remembered between sessions.

#### Scenario: Add a folder
- **WHEN** the VJ presses add folder and picks ~/VJ/images
- **THEN** an "images" chip appears and shows that folder's images and videos as thumbnails, also after a restart
