//! Imports Resolume's Advanced Output (screens, slices, masks and warps) into an [`OutputSetup`].
//!
//! Resolume keeps the live setup in `Preferences/AdvancedOutput.xml` (root `<ScreenSetup>`) and saved
//! presets in `Presets/Advanced Output/*.xml` (a `<ScreenSetup>` inside an `<XmlState>`). Both are read.
//!
//! The element names come from real files written by Arena 5.1 to 7.23:
//!
//! - `<Screen>` has `Params` (name, enabled, opacity, colour), an `<OutputDevice>` and `<layers>`.
//! - `<layers>` holds `<Slice>`, `<Polygon>` (a slice with `InputContour`/`OutputContour`) and `<Mask>`.
//! - A slice's `<Warper>` has a `<Homography>` (`src` → `dst` corner pin) and a `<BezierWarper>` grid
//!   whose points sit in the un-pinned `src` space. Its "Point Mode" is `PM_LINEAR` or `PM_BEZIER`.
//! - Shapes are `<Contour closed>` with `<points>` and a `<segments>` string, one letter per segment:
//!   `L` for a line and `C` for a cubic (which takes two extra control points).
//!
//! DMX screens (Lumiverse pixel mapping) are skipped and noted.

use std::path::Path;

use openvj_core::{
    ColorTrim, Contour, DeviceKind, Mask, OutputDevice, OutputSetup, Point, PointMode, Quad, Screen, ScreenLayer,
    SegmentKind, Slice, SliceShape, Warp, rect_quad,
};
use roxmltree::{Document, Node};

use super::{child, children};

/// An Advanced Output setup, plus what could not be carried over.
#[derive(Debug, Clone)]
pub struct ImportedOutput {
    pub setup: OutputSetup,
    /// The Resolume version that saved the file, such as "Resolume Arena 7.22.9".
    pub saved_by: Option<String>,
    /// One line per thing that was dropped or approximated.
    pub notes: Vec<String>,
}

/// Reads an Advanced Output preset or `AdvancedOutput.xml` file.
pub fn import_output_file(path: &Path) -> Result<ImportedOutput, String> {
    let xml = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut imported = import_output_str(&xml)?;
    if imported.setup.name.is_empty()
        && let Some(stem) = path.file_stem()
    {
        imported.setup.name = stem.to_string_lossy().into_owned();
    }
    Ok(imported)
}

/// Reads Advanced Output XML.
pub fn import_output_str(xml: &str) -> Result<ImportedOutput, String> {
    let doc = Document::parse(xml).map_err(|e| format!("not valid XML: {e}"))?;
    let root = doc.root_element();
    let (setup_node, name) = match root.tag_name().name() {
        "ScreenSetup" => (root, String::new()),
        "XmlState" => (
            child(root, "ScreenSetup").ok_or("this preset has no <ScreenSetup>")?,
            root.attribute("name").unwrap_or_default().to_owned(),
        ),
        other => return Err(format!("not a Resolume Advanced Output file (root is <{other}>)")),
    };

    let mut notes = Vec::new();
    let saved_by = root.descendants().find(|n| n.has_tag_name("versionInfo")).map(|v| {
        let a = |k| v.attribute(k).unwrap_or("?");
        format!("{} {}.{}.{}", a("name"), a("majorVersion"), a("minorVersion"), a("microVersion"))
    });

    let mut setup = OutputSetup { name, ..OutputSetup::default() };
    if let Some(size) = child(setup_node, "CurrentCompositionTextureSize") {
        setup.composition_size = [attr_u32(size, "width").unwrap_or(1920), attr_u32(size, "height").unwrap_or(1080)];
    } else if let Some(input) = setup_node.descendants().find(|n| n.has_tag_name("InputSize")) {
        // Arena 5 files keep the composition size under <sizing>.
        setup.composition_size = [attr_u32(input, "width").unwrap_or(1920), attr_u32(input, "height").unwrap_or(1080)];
    }
    if let Some(power) = child(setup_node, "SoftEdging").and_then(|s| param_f32(s, "Power")) {
        setup.soft_edge_power = power;
    }

    for node in child(setup_node, "screens").into_iter().flat_map(|s| s.children().filter(|n| n.is_element())) {
        match node.tag_name().name() {
            "Screen" => setup.screens.push(screen(node, &mut notes)),
            "DmxScreen" => notes.push(format!(
                "DMX screen \"{}\" skipped: openVJ has no DMX output yet",
                node.attribute("name").unwrap_or("?")
            )),
            other => notes.push(format!("Unknown screen type <{other}> skipped")),
        }
    }
    Ok(ImportedOutput { setup, saved_by, notes })
}

fn screen(node: Node, notes: &mut Vec<String>) -> Screen {
    let name = param_str(node, "Name").or(node.attribute("name")).unwrap_or("Screen").to_owned();
    let device = child(node, "OutputDevice").and_then(|d| d.children().find(|n| n.is_element()));
    let device = match device {
        Some(d) => output_device(d),
        None => {
            OutputDevice { kind: DeviceKind::Virtual, name: name.clone(), id: String::new(), width: 1920, height: 1080 }
        }
    };
    let mut screen = Screen {
        id: attr_u64(node, "uniqueId").unwrap_or(0),
        enabled: param_bool(node, "Enabled").unwrap_or(true),
        opacity: param_f32(node, "Opacity").unwrap_or(1.0),
        color: color(node),
        device,
        layers: Vec::new(),
        name,
    };
    if let DeviceKind::Other(kind) = &screen.device.kind {
        notes.push(format!("Screen \"{}\": output device {kind} imported as virtual", screen.name));
    }
    for layer in child(node, "layers").into_iter().flat_map(|l| l.children().filter(|n| n.is_element())) {
        match layer.tag_name().name() {
            "Slice" | "Polygon" => screen.layers.push(ScreenLayer::Slice(slice(layer, &screen.name, notes))),
            "Mask" => screen.layers.push(ScreenLayer::Mask(mask(layer, "Mask"))),
            other => notes.push(format!("Screen \"{}\": layer type <{other}> skipped", screen.name)),
        }
    }
    let guides = child(node, "guides").into_iter().flat_map(|g| children(g, "ScreenGuide"));
    if guides.flat_map(|g| g.descendants()).any(|n| n.has_tag_name("ParamPixels") && n.attribute("fileName").is_some())
    {
        notes.push(format!("Screen \"{}\": output guide image not imported", screen.name));
    }
    screen
}

fn output_device(node: Node) -> OutputDevice {
    let kind = match node.tag_name().name() {
        "OutputDeviceDisplay" => DeviceKind::Display,
        "OutputDeviceVirtual" => DeviceKind::Virtual,
        "OutputDeviceNDI" => DeviceKind::Ndi,
        "OutputDeviceSpout" => DeviceKind::Spout,
        "OutputDeviceSyphon" => DeviceKind::Syphon,
        other => DeviceKind::Other(other.trim_start_matches("OutputDevice").to_owned()),
    };
    // Virtual outputs also keep their size as Width/Height params, which win when they differ.
    let width = param_f32(node, "Width").map(|w| w as u32).or(attr_u32(node, "width")).unwrap_or(1920);
    let height = param_f32(node, "Height").map(|h| h as u32).or(attr_u32(node, "height")).unwrap_or(1080);
    OutputDevice {
        kind,
        name: node.attribute("name").unwrap_or_default().to_owned(),
        id: node.attribute("deviceId").unwrap_or_default().to_owned(),
        width,
        height,
    }
}

fn slice(node: Node, screen: &str, notes: &mut Vec<String>) -> Slice {
    let name = param_str(node, "Name").unwrap_or("Slice").to_owned();
    let input = quad(child(node, "InputRect")).unwrap_or_else(|| rect_quad(0.0, 0.0, 1920.0, 1080.0));
    let output = quad(child(node, "OutputRect")).unwrap_or(input);
    let shape = match (child(node, "InputContour"), child(node, "OutputContour")) {
        (Some(i), Some(o)) => Some(SliceShape { input: contour(i), output: contour(o) }),
        (Some(i), None) => Some(SliceShape { input: contour(i), output: contour(i) }),
        _ => None,
    };
    let warp = child(node, "Warper").map(|w| warp(w, output)).unwrap_or_else(|| Warp::identity(output, 4, 4));
    if !(param_f32(node, "BRed").unwrap_or(0.0) == 0.0
        && param_f32(node, "BGreen").unwrap_or(0.0) == 0.0
        && param_f32(node, "BBlue").unwrap_or(0.0) == 0.0)
    {
        notes.push(format!("Slice \"{name}\" on \"{screen}\": black level compensation not imported"));
    }
    Slice {
        id: attr_u64(node, "uniqueId").unwrap_or(0),
        enabled: param_bool(node, "Enabled").unwrap_or(true),
        source: param_str(node, "Input Source").unwrap_or("0:1").to_owned(),
        input,
        output,
        shape,
        warp,
        mask: child(node, "SliceMask").map(|m| mask(m, "mask")),
        color: color(node),
        soft_edge: param_bool(node, "SoftEdgeEnable").unwrap_or(false),
        flip: param_f32(node, "Flip").map(|f| f as u8).unwrap_or(0),
        black_bg: param_bool(node, "Black BG").unwrap_or(false),
        is_key: param_bool(node, "Is Key").unwrap_or(false),
        name,
    }
}

fn warp(node: Node, output: Quad) -> Warp {
    let homography = child(node, "Homography");
    let src = homography.and_then(|h| quad(child(h, "src"))).unwrap_or(output);
    let dst = homography.and_then(|h| quad(child(h, "dst"))).unwrap_or(src);
    let mut warp = Warp::identity(src, 4, 4);
    warp.dst = dst;
    if let Some(grid) = child(node, "BezierWarper") {
        let cols = attr_u32(grid, "controlWidth").unwrap_or(4);
        let rows = attr_u32(grid, "controlHeight").unwrap_or(4);
        let points = child(grid, "vertices").map(points).unwrap_or_default();
        if cols >= 2 && rows >= 2 && points.len() == (cols * rows) as usize {
            (warp.cols, warp.rows, warp.points) = (cols, rows, points);
        }
    }
    if param_str(node, "Point Mode") == Some("PM_BEZIER") {
        warp.mode = PointMode::Bezier;
    }
    warp
}

/// A screen `<Mask>` or a slice's `<SliceMask>`.
fn mask(node: Node, default_name: &str) -> Mask {
    let shape = child(node, "ShapeObject").and_then(|s| child(s, "Shape"));
    Mask {
        id: attr_u64(node, "uniqueId").unwrap_or(0),
        name: param_str(node, "Name").unwrap_or(default_name).to_owned(),
        enabled: param_bool(node, "Enabled").unwrap_or(true),
        invert: param_bool(node, "Invert").unwrap_or(true),
        contours: shape.into_iter().flat_map(|s| children(s, "Contour")).map(contour).collect(),
    }
}

fn contour(node: Node) -> Contour {
    let points = child(node, "points").map(points).unwrap_or_default();
    let letters = child(node, "segments").and_then(|s| s.text()).unwrap_or_default();
    let mut segments: Vec<SegmentKind> = letters
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| if c.eq_ignore_ascii_case(&'C') { SegmentKind::Cubic } else { SegmentKind::Line })
        .collect();
    if segments.is_empty() {
        segments = vec![SegmentKind::Line; points.len()];
    }
    Contour { closed: node.attribute("closed").is_none_or(|c| c == "1"), points, segments }
}

fn quad(node: Option<Node>) -> Option<Quad> {
    let p = points(node?);
    (p.len() == 4).then(|| [p[0], p[1], p[2], p[3]])
}

fn points(node: Node) -> Vec<Point> {
    children(node, "v")
        .filter_map(|v| Some([v.attribute("x")?.trim().parse().ok()?, v.attribute("y")?.trim().parse().ok()?]))
        .collect()
}

fn color(node: Node) -> ColorTrim {
    let get = |name| param_f32(node, name).unwrap_or(0.0);
    ColorTrim {
        brightness: get("Brightness"),
        contrast: get("Contrast"),
        red: get("Red"),
        green: get("Green"),
        blue: get("Blue"),
    }
}

// Advanced Output elements split their parameters over several `<Params>` groups ("Common",
// "Input", "Output"...), so these look through all of them.

fn param_str<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    let p = children(node, "Params")
        .flat_map(|g| g.children().filter(|n| n.is_element()))
        .find(|p| p.attribute("name") == Some(name))?;
    p.attribute("value").or_else(|| p.attribute("default"))
}

fn param_f32(node: Node, name: &str) -> Option<f32> {
    param_str(node, name)?.trim().parse().ok()
}

fn param_bool(node: Node, name: &str) -> Option<bool> {
    param_str(node, name).map(|v| v.trim() == "1" || v.eq_ignore_ascii_case("true"))
}

fn attr_u32(node: Node, attr: &str) -> Option<u32> {
    node.attribute(attr)?.trim().parse::<f32>().ok().map(|v| v.round() as u32)
}

fn attr_u64(node: Node, attr: &str) -> Option<u64> {
    node.attribute(attr)?.trim().parse().ok()
}

#[cfg(test)]
mod tests;
