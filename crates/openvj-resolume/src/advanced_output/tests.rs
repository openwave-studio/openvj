use super::*;

const TWO_SCREENS: &str = include_str!("../../tests/fixtures/TwoScreens.xml");
const DISPLAY_AND_SPOUT: &str = include_str!("../../tests/fixtures/DisplayAndSpout.xml");
const MASKED_SHAPES: &str = include_str!("../../tests/fixtures/MaskedShapes.xml");
const BEZIER_BULBS: &str = include_str!("../../tests/fixtures/BezierBulbs.xml");
const CURVED_MASK: &str = include_str!("../../tests/fixtures/CurvedMask.xml");

/// A preferences-style file with a corner-pinned and grid-warped slice, a polygon slice, a slice
/// mask and a screen mask. Element shapes are copied from real Arena files; the values are made up.
const MAPPED: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<ScreenSetup name="ScreenSetup">
  <versionInfo name="Resolume Arena" majorVersion="7" minorVersion="23" microVersion="2" revision="51094"/>
  <Params name="ScreenSetupParams"/>
  <CurrentCompositionTextureSize width="1920" height="1080"/>
  <screens>
    <Screen name="Hexagon" uniqueId="10">
      <Params name="Params">
        <Param name="Name" T="STRING" default="" value="Hexagon"/>
        <Param name="Enabled" T="BOOL" default="1" value="1"/>
      </Params>
      <Params name="Output">
        <ParamRange name="Opacity" T="DOUBLE" default="1" value="0.75"/>
        <ParamRange name="Brightness" T="DOUBLE" default="0" value="0.1"/>
        <ParamRange name="Blue" T="DOUBLE" default="0" value="-0.2"/>
      </Params>
      <layers>
        <Slice uniqueId="11">
          <Params name="Common">
            <Param name="Name" T="STRING" default="Layer" value="Wall"/>
            <Param name="Enabled" T="BOOL" default="1" value="1"/>
          </Params>
          <Params name="Input">
            <ParamChoice name="Input Source" default="0:1" value="0:1" storeChoices="0"/>
            <Param name="SoftEdgeEnable" T="BOOL" default="0" value="1"/>
          </Params>
          <Params name="Output">
            <Param name="Flip" T="UINT8" default="0" value="2"/>
            <ParamRange name="BRed" altName="Red" T="DOUBLE" default="0" value="0.1"/>
          </Params>
          <InputRect orientation="0"><v x="0" y="0"/><v x="1920" y="0"/><v x="1920" y="1080"/><v x="0" y="1080"/></InputRect>
          <OutputRect orientation="0"><v x="-3.7" y="-1.9"/><v x="1617.6" y="-1.9"/><v x="1617.6" y="1061.4"/><v x="-3.7" y="1061.4"/></OutputRect>
          <SliceMask>
            <Params name="Input Mask">
              <Param name="Name" T="STRING" default="mask" value="mask"/>
              <Param name="Enabled" T="BOOL" default="1" value="1"/>
              <Param name="Invert" T="BOOL" default="1" value="1"/>
            </Params>
            <ShapeObject>
              <Params name="Shape"><ParamChoice name="Point Mode" default="PM_LINEAR" value="PM_BEZIER" storeChoices="0"/></Params>
              <Rect orientation="0"><v x="420" y="0"/><v x="1500" y="0"/><v x="1500" y="1080"/><v x="420" y="1080"/></Rect>
              <Shape>
                <Contour closed="1">
                  <points>
                    <v x="960" y="0"/><v x="1258.2" y="0"/><v x="1500" y="241.8"/>
                    <v x="1500" y="540"/><v x="1500" y="838.2"/><v x="1258.2" y="1080"/>
                    <v x="960" y="1080"/><v x="661.8" y="1080"/><v x="420" y="838.2"/>
                    <v x="420" y="540"/><v x="420" y="241.8"/><v x="661.8" y="0"/>
                  </points>
                  <segments>CCCC</segments>
                </Contour>
              </Shape>
            </ShapeObject>
          </SliceMask>
          <Warper>
            <Params name="Warper">
              <ParamChoice name="Point Mode" default="PM_LINEAR" value="PM_BEZIER" storeChoices="0"/>
            </Params>
            <BezierWarper controlWidth="2" controlHeight="2">
              <vertices><v x="0" y="0"/><v x="1920" y="0"/><v x="0" y="1080"/><v x="1920" y="1080"/></vertices>
            </BezierWarper>
            <Homography>
              <src><v x="0" y="0"/><v x="1920" y="0"/><v x="1920" y="1080"/><v x="0" y="1080"/></src>
              <dst><v x="98.3" y="-1.9"/><v x="1617.6" y="7.4"/><v x="1584.2" y="1061.4"/><v x="-3.7" y="966.8"/></dst>
            </Homography>
          </Warper>
        </Slice>
        <Polygon uniqueId="12" IsVirgin="0">
          <Params name="Common"><Param name="Name" T="STRING" default="Layer" value="Head"/></Params>
          <InputRect orientation="0"><v x="0" y="0"/><v x="100" y="0"/><v x="100" y="100"/><v x="0" y="100"/></InputRect>
          <OutputRect orientation="0"><v x="0" y="0"/><v x="100" y="0"/><v x="100" y="100"/><v x="0" y="100"/></OutputRect>
          <InputContour closed="1"><points><v x="0" y="0"/><v x="100" y="0"/><v x="50" y="100"/></points><segments>LLL</segments></InputContour>
          <OutputContour closed="1"><points><v x="10" y="0"/><v x="110" y="0"/><v x="60" y="100"/></points><segments>LLL</segments></OutputContour>
        </Polygon>
        <Mask uniqueId="13">
          <Params name="Common">
            <Param name="Name" T="STRING" default="Layer" value="Mask 1"/>
            <Param name="Enabled" T="BOOL" default="1" value="1"/>
          </Params>
          <Params name="Output"><Param name="Invert" T="BOOL" default="1" value="0"/></Params>
          <ShapeObject>
            <Shape><Contour closed="1"><points><v x="0" y="0"/><v x="1280" y="0"/><v x="640" y="720"/></points><segments>LLL</segments></Contour></Shape>
          </ShapeObject>
        </Mask>
      </layers>
      <OutputDevice>
        <OutputDeviceNDI name="Hexagon" deviceId="NDIHexagon" idHash="1" width="1280" height="720"/>
      </OutputDevice>
    </Screen>
    <DmxScreen name="Lumiverse 1" uniqueId="20" LumiverseId="1"/>
  </screens>
  <SoftEdging>
    <Params name="Soft Edge"><ParamRange name="Power" T="DOUBLE" default="2" value="2.5"/></Params>
  </SoftEdging>
</ScreenSetup>
"#;

fn close(a: Point, b: Point) -> bool {
    (a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01
}

#[test]
fn reads_a_real_arena_7_preset() {
    let ImportedOutput { setup, saved_by, notes } = import_output_str(TWO_SCREENS).unwrap();
    assert_eq!(setup.name, "experiencia TD");
    assert_eq!(saved_by.as_deref(), Some("Resolume Arena 7.13.2"));
    assert_eq!(setup.composition_size, [1920, 1080]);
    assert_eq!(setup.screens.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["LEFT", "RIGHT"]);
    assert!(notes.is_empty(), "{notes:?}");

    let left = &setup.screens[0];
    assert_eq!((left.device.kind.clone(), left.size()), (DeviceKind::Virtual, [1920, 1080]));
    assert!(left.enabled && left.opacity == 1.0 && left.color.is_neutral());
    let slice = left.slices().next().unwrap();
    assert_eq!(slice.name, "Slice 1");
    assert_eq!(slice.source, "0:1");
    assert_eq!(slice.input, rect_quad(0.0, 0.0, 960.0, 1080.0), "the left half of the composition");
    assert_eq!(slice.output, rect_quad(29.0, 0.0, 1920.0, 1080.0));

    // Both slices are hand-warped: a 7×7 grid on the left, a 25×13 grid plus a corner pin on the right.
    let right = setup.screens[1].slices().next().unwrap();
    assert_eq!(right.input, rect_quad(960.0, 0.0, 1920.0, 1080.0));
    assert_eq!((slice.warp.cols, slice.warp.rows, right.warp.cols, right.warp.rows), (7, 7, 25, 13));
    assert!(!slice.warp.is_identity() && !right.warp.is_identity());
}

/// Resolume saves each slice's OutputRect as the bounding box of its warped surface. Checking our warp
/// against that confirms the grid points sit in the un-pinned space, the corner pin comes after, and
/// `PM_BEZIER` grids are bezier control nets, as real Arena 7.15 presets showed. BezierBulbs.xml keeps
/// that check: read as plain grid points its nets would overshoot the rect by 30 px; as a bezier
/// surface they land within a pixel.
#[test]
fn warped_surface_fills_resolumes_output_rect() {
    for xml in [TWO_SCREENS, DISPLAY_AND_SPOUT, MASKED_SHAPES, BEZIER_BULBS, CURVED_MASK] {
        for screen in import_output_str(xml).unwrap().setup.screens {
            for slice in screen.slices() {
                let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
                let n = 96;
                for r in 0..=n {
                    for c in 0..=n {
                        let p = slice.warp.map(c as f32 / n as f32, r as f32 / n as f32);
                        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
                        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
                    }
                }
                let near = |a: Point, b: Point| (a[0] - b[0]).abs() < 1.0 && (a[1] - b[1]).abs() < 1.0;
                assert!(
                    near(lo, slice.output[0]) && near(hi, slice.output[2]),
                    "{} / {}: {lo:?} {hi:?} vs {:?}",
                    screen.name,
                    slice.name,
                    slice.output
                );
            }
        }
    }
}

#[test]
fn inverted_screen_masks_show_their_union() {
    let ImportedOutput { setup, notes, .. } = import_output_str(MASKED_SHAPES).unwrap();
    assert!(notes.is_empty(), "{notes:?}");
    let screen = &setup.screens[0];
    assert_eq!((screen.device.kind.clone(), screen.size()), (DeviceKind::Display, [1920, 1080]));
    let masks: Vec<_> = screen.masks().map(|m| (m.name.as_str(), m.invert, m.contours[0].points.len())).collect();
    assert_eq!(masks, [("Left", true, 3), ("Hexagon", true, 6), ("Right", true, 3)]);
    assert!(screen.shows([960.0, 540.0]), "middle of the hexagon");
    assert!(screen.shows([300.0, 950.0]), "inside the left triangle, outside the hexagon");
    assert!(!screen.shows([580.0, 950.0]), "the black gap between the triangle and the hexagon");
    assert!(!screen.shows([10.0, 10.0]), "outside every shape");
}

#[test]
fn bezier_warps_and_masks_use_bezier_shapes() {
    let bulbs = import_output_str(BEZIER_BULBS).unwrap().setup;
    let slices: Vec<_> = bulbs.screens[0].slices().collect();
    assert_eq!(slices.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Left", "Middle", "Right"]);
    assert!(slices.iter().all(|s| s.warp.mode == PointMode::Bezier && (s.warp.cols, s.warp.rows) == (4, 4)));
    assert_eq!(slices[1].flip, 2, "the middle bulb is flipped vertically");

    let curved = import_output_str(CURVED_MASK).unwrap().setup;
    let screen = &curved.screens[0];
    let mask = screen.masks().next().unwrap();
    assert_eq!(mask.contours[0].segments, [SegmentKind::Cubic; 6]);
    assert!(screen.shows([960.0, 300.0]) && !screen.shows([960.0, 1060.0]));
}

#[test]
fn reads_display_and_spout_outputs() {
    let setup = import_output_str(DISPLAY_AND_SPOUT).unwrap().setup;
    let kinds: Vec<_> = setup.screens.iter().map(|s| s.device.kind.clone()).collect();
    assert_eq!(kinds, [DeviceKind::Display, DeviceKind::Spout, DeviceKind::Spout]);
    assert_eq!(setup.screens[0].device.name, "Display 1");
    assert!(setup.screens.iter().all(|s| s.slices().all(|sl| sl.warp.is_identity())));
}

#[test]
fn reads_warps_polygons_and_masks() {
    let ImportedOutput { setup, notes, .. } = import_output_str(MAPPED).unwrap();
    assert_eq!(setup.soft_edge_power, 2.5);
    assert_eq!(setup.screens.len(), 1, "the DMX screen is skipped");
    assert!(notes.iter().any(|n| n.contains("DMX screen \"Lumiverse 1\"")));
    assert!(notes.iter().any(|n| n.contains("black level")));

    let screen = &setup.screens[0];
    assert_eq!((screen.device.kind.clone(), screen.size()), (DeviceKind::Ndi, [1280, 720]));
    assert_eq!(screen.opacity, 0.75);
    assert_eq!((screen.color.brightness, screen.color.blue), (0.1, -0.2));
    assert_eq!(screen.layers.len(), 3);

    let wall = screen.slices().next().unwrap();
    assert!(wall.soft_edge);
    assert_eq!(wall.flip, 2);
    assert_eq!(wall.warp.mode, PointMode::Bezier);
    assert_eq!((wall.warp.cols, wall.warp.rows), (2, 2));
    assert!(close(wall.warp.map(1.0, 0.0), [1617.6, 7.4]), "corner pin applied after the grid");
    let mask = wall.mask.as_ref().unwrap();
    assert!(mask.invert && mask.contours[0].segments == [SegmentKind::Cubic; 4]);
    assert!(mask.shows([960.0, 540.0]) && !mask.shows([430.0, 10.0]), "inverted: only the circle shows");

    let head = screen.slices().nth(1).unwrap();
    assert_eq!(head.name, "Head");
    let shape = head.shape.as_ref().unwrap();
    assert_eq!(shape.output.points[0], [10.0, 0.0]);
    assert!(head.warp.is_identity());

    let cut = screen.masks().next().unwrap();
    assert_eq!((cut.name.as_str(), cut.invert), ("Mask 1", false));
    assert!(!cut.shows([640.0, 200.0]) && cut.shows([100.0, 700.0]));
    assert!(matches!(screen.layers[2], ScreenLayer::Mask(_)), "masks keep their place in the layer order");
}

#[test]
fn rejects_files_that_are_not_advanced_output() {
    assert!(import_output_str("<Composition/>").unwrap_err().contains("not a Resolume Advanced Output"));
    assert!(import_output_str("not xml").is_err());
}
