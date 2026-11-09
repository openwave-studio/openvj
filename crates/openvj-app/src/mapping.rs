//! The output mapping the output window shows: saved per machine, like Resolume's Advanced Output,
//! not in the composition, so a show file opens with the venue's mapping.

use std::path::{Path, PathBuf};

use openvj_core::{Contour, Mask, OutputSetup, Point, Quad, ScreenLayer, Slice, rect_quad};

#[derive(Default)]
pub struct Mapping {
    /// `None`: no mapping, the output window shows the whole composition.
    pub setup: Option<OutputSetup>,
    pub test_card: bool,
    pub editor: Editor,
}

impl Mapping {
    /// The mapping saved on this machine, if any.
    pub fn load() -> Self {
        let setup = settings_file().and_then(|p| std::fs::read(p).ok()).and_then(|b| {
            serde_json::from_slice::<OutputSetup>(&b)
                .inspect_err(|e| eprintln!("openVJ: ignoring the saved output mapping: {e}"))
                .ok()
        });
        Self { setup, ..Self::default() }
    }

    /// Imports a Resolume Advanced Output preset and saves it. Returns a status line.
    pub fn import(&mut self, path: &Path) -> Result<String, String> {
        let imported = openvj_resolume::import_output_file(path)?;
        for note in &imported.notes {
            eprintln!("openVJ: output mapping: {note}");
        }
        let msg = format!("Mapping \"{}\" imported: {}.", imported.setup.name, summary(&imported.setup));
        self.setup = Some(imported.setup);
        self.editor.selected = None;
        self.save();
        Ok(msg)
    }

    /// Back to no mapping.
    pub fn clear(&mut self) {
        self.setup = None;
        self.test_card = false;
        self.editor.selected = None;
        self.save();
    }

    /// One line for the settings page.
    pub fn status(&self) -> String {
        match &self.setup {
            None => "None: the output window shows the whole composition.".into(),
            Some(s) => format!("{}: {}.", if s.name.is_empty() { "Mapping" } else { &s.name }, summary(s)),
        }
    }

    /// Writes the setup to openVJ's settings folder.
    pub fn save(&self) {
        let Some(path) = settings_file() else { return };
        let result = match &self.setup {
            None => std::fs::remove_file(&path).or_else(|e| match e.kind() {
                std::io::ErrorKind::NotFound => Ok(()),
                _ => Err(e),
            }),
            Some(setup) => path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|_| std::fs::write(&path, serde_json::to_vec_pretty(setup).unwrap_or_default())),
        };
        if let Err(e) = result {
            eprintln!("openVJ: could not save the output mapping to {}: {e}", path.display());
        }
    }
}

fn summary(s: &OutputSetup) -> String {
    let screens = s.screens.len();
    let (mut slices, mut masks) = (0, 0);
    for layer in s.screens.iter().flat_map(|sc| &sc.layers) {
        match layer {
            ScreenLayer::Slice(_) => slices += 1,
            ScreenLayer::Mask(_) => masks += 1,
        }
    }
    let n = |k: usize, one: &str| format!("{k} {one}{}", if k == 1 { "" } else { "s" });
    let mut text = format!("{}, {}, {}", n(screens, "screen"), n(slices, "slice"), n(masks, "mask"));
    if screens > 1 {
        text.push_str(" (the output window shows the first screen)");
    }
    text
}

/// `output-mapping.json` in openVJ's settings folder.
fn settings_file() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let dir = if cfg!(target_os = "macos") {
        home?.join("Library/Application Support/openVJ")
    } else if cfg!(windows) {
        PathBuf::from(std::env::var_os("APPDATA")?).join("openVJ")
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home.map(|h| h.join(".config")))?
            .join("openvj")
    };
    Some(dir.join("output-mapping.json"))
}

// ---------------------------------------------------------------- editing

/// The mapping window's two stages, as in Resolume's Advanced Output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stage {
    /// What each slice takes from the composition.
    Input,
    /// Where each slice goes on the screen.
    #[default]
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Drag {
    Corner(usize),
    Move { last: Point },
}

/// What the mapping window is editing: always screen 1 of the setup for now.
#[derive(Debug, Clone, Copy, Default)]
pub struct Editor {
    pub stage: Stage,
    /// Index into screen 1's layers.
    pub selected: Option<usize>,
    /// The corner the arrow keys move, 0 to 3 clockwise from top left.
    pub corner: usize,
    drag: Option<Drag>,
}

pub const CORNER_NAMES: [&str; 4] = ["Top left", "Top right", "Bottom right", "Bottom left"];

impl Editor {
    /// The quad being edited for a slice at this stage.
    pub fn quad(&self, slice: &Slice) -> Quad {
        match self.stage {
            Stage::Input => slice.input,
            Stage::Output => slice.warp.dst,
        }
    }

    /// The points the selected layer shows as handles at this stage: a slice's four corners, or a
    /// screen mask's outline points (output stage only).
    pub fn points(&self, layer: &ScreenLayer) -> Vec<Point> {
        match (layer, self.stage) {
            (ScreenLayer::Slice(s), _) => self.quad(s).to_vec(),
            (ScreenLayer::Mask(m), Stage::Output) => m.contours.first().map(|c| c.points.clone()).unwrap_or_default(),
            (ScreenLayer::Mask(_), Stage::Input) => Vec::new(),
        }
    }

    fn points_mut<'a>(&self, setup: &'a mut OutputSetup) -> Option<&'a mut [Point]> {
        let i = self.selected?;
        match (setup.screens.first_mut()?.layers.get_mut(i)?, self.stage) {
            (ScreenLayer::Slice(s), Stage::Input) => Some(&mut s.input[..]),
            (ScreenLayer::Slice(s), Stage::Output) => Some(&mut s.warp.dst[..]),
            (ScreenLayer::Mask(m), Stage::Output) => m.contours.first_mut().map(|c| &mut c.points[..]),
            (ScreenLayer::Mask(_), Stage::Input) => None,
        }
    }

    /// The outline of a layer on the canvas at this stage (empty for masks on the input stage).
    pub fn layer_outline(&self, layer: &ScreenLayer) -> Vec<Vec<Point>> {
        match (layer, self.stage) {
            (ScreenLayer::Slice(s), _) => vec![self.outline(s)],
            (ScreenLayer::Mask(m), Stage::Output) => m.contours.iter().map(|c| c.flatten(16)).collect(),
            (ScreenLayer::Mask(_), Stage::Input) => Vec::new(),
        }
    }

    /// The outline of a slice on the canvas at this stage.
    pub fn outline(&self, slice: &Slice) -> Vec<Point> {
        match self.stage {
            Stage::Input => slice.input.to_vec(),
            Stage::Output => {
                const N: u32 = 16;
                let mut pts = Vec::with_capacity(4 * N as usize);
                let t = |i: u32| i as f32 / N as f32;
                pts.extend((0..N).map(|i| slice.warp.map(t(i), 0.0)));
                pts.extend((0..N).map(|i| slice.warp.map(1.0, t(i))));
                pts.extend((0..N).map(|i| slice.warp.map(1.0 - t(i), 1.0)));
                pts.extend((0..N).map(|i| slice.warp.map(0.0, 1.0 - t(i))));
                pts
            }
        }
    }

    /// A press on the canvas at `p` (canvas pixels): grabs a point of the selected layer, or picks
    /// and grabs the topmost layer under the pointer. `reach` is how close counts as on a point.
    /// Returns whether anything changed.
    pub fn press(&mut self, setup: &mut OutputSetup, p: Point, reach: f32) -> bool {
        self.drag = None;
        if let Some(points) = self.points_mut(setup) {
            let nearest = (0..points.len()).min_by(|&a, &b| dist(points[a], p).total_cmp(&dist(points[b], p)));
            if let Some(i) = nearest.filter(|&i| dist(points[i], p) <= reach) {
                self.corner = i;
                self.drag = Some(Drag::Corner(i));
                return true;
            }
        }
        let Some(screen) = setup.screens.first() else { return false };
        let enabled = |l: &ScreenLayer| match l {
            ScreenLayer::Slice(s) => s.enabled,
            ScreenLayer::Mask(m) => m.enabled,
        };
        let hit = screen.layers.iter().enumerate().rev().find_map(|(i, l)| {
            let outlines = self.layer_outline(l);
            (enabled(l) && outlines.iter().filter(|o| inside(o, p)).count() % 2 == 1).then_some(i)
        });
        if let Some(i) = hit {
            if self.selected != Some(i) {
                self.corner = 0;
            }
            self.selected = Some(i);
            self.drag = Some(Drag::Move { last: p });
            return true;
        }
        false
    }

    /// The pointer moved to `p` while pressed.
    pub fn drag_to(&mut self, setup: &mut OutputSetup, p: Point) -> bool {
        let Some(drag) = self.drag else { return false };
        let Some(points) = self.points_mut(setup) else { return false };
        match drag {
            Drag::Corner(i) => {
                if let Some(q) = points.get_mut(i) {
                    *q = p;
                }
            }
            Drag::Move { last } => {
                let d = [p[0] - last[0], p[1] - last[1]];
                points.iter_mut().for_each(|q| *q = [q[0] + d[0], q[1] + d[1]]);
                self.drag = Some(Drag::Move { last: p });
            }
        }
        true
    }

    /// Ends a drag. Returns whether the setup changed and should be saved.
    pub fn release(&mut self) -> bool {
        self.drag.take().is_some()
    }

    /// Moves the current point by `d` pixels.
    pub fn nudge(&mut self, setup: &mut OutputSetup, d: Point) -> bool {
        let corner = self.corner;
        let Some(q) = self.points_mut(setup).and_then(|p| p.get_mut(corner)) else { return false };
        *q = [q[0] + d[0], q[1] + d[1]];
        true
    }

    /// Types one coordinate (`axis` 0 = x, 1 = y) of a point.
    pub fn set_corner(&mut self, setup: &mut OutputSetup, corner: usize, axis: usize, value: f32) -> bool {
        let Some(q) = self.points_mut(setup).and_then(|p| p.get_mut(corner)) else { return false };
        q[axis.min(1)] = value;
        self.corner = corner;
        true
    }

    pub fn next_corner(&mut self, setup: &OutputSetup) {
        let n =
            self.selected.and_then(|i| setup.screens.first()?.layers.get(i)).map_or(4, |l| self.points(l).len()).max(1);
        self.corner = (self.corner + 1) % n;
    }

    /// Output: back to the un-pinned corners. Input: the whole composition.
    pub fn reset(&mut self, setup: &mut OutputSetup) -> bool {
        let [w, h] = setup.composition_size;
        let stage = self.stage;
        let Some(i) = self.selected else { return false };
        let Some(ScreenLayer::Slice(slice)) = setup.screens.first_mut().and_then(|s| s.layers.get_mut(i)) else {
            return false;
        };
        match stage {
            Stage::Output => slice.warp.dst = slice.warp.src,
            Stage::Input => slice.input = rect_quad(0.0, 0.0, w as f32, h as f32),
        }
        true
    }

    fn next_id(screen: &openvj_core::Screen) -> u64 {
        screen
            .layers
            .iter()
            .map(|l| match l {
                ScreenLayer::Slice(s) => s.id,
                ScreenLayer::Mask(m) => m.id,
            })
            .max()
            .unwrap_or(0)
            + 1
    }

    /// Adds a slice showing the whole composition over the whole screen, and selects it.
    pub fn add_slice(&mut self, setup: &mut OutputSetup) -> bool {
        let [cw, ch] = setup.composition_size;
        let Some(screen) = setup.screens.first_mut() else { return false };
        let id = Self::next_id(screen);
        let n = screen.slices().count() + 1;
        let [sw, sh] = screen.size();
        let slice = Slice::new(
            id,
            format!("Slice {n}"),
            rect_quad(0.0, 0.0, cw as f32, ch as f32),
            rect_quad(0.0, 0.0, sw as f32, sh as f32),
        );
        screen.layers.push(ScreenLayer::Slice(slice));
        self.selected = Some(screen.layers.len() - 1);
        self.corner = 0;
        true
    }

    /// Adds a rectangular mask over the middle of the screen, inverted as Resolume's are by default
    /// (it shows its inside), and selects it on the output stage.
    pub fn add_mask(&mut self, setup: &mut OutputSetup) -> bool {
        let Some(screen) = setup.screens.first_mut() else { return false };
        let id = Self::next_id(screen);
        let n = screen.masks().count() + 1;
        let [w, h] = screen.size().map(|v| v as f32);
        let rect = rect_quad(w * 0.25, h * 0.25, w * 0.75, h * 0.75);
        screen.layers.push(ScreenLayer::Mask(Mask {
            id,
            name: format!("Mask {n}"),
            enabled: true,
            invert: true,
            contours: vec![Contour::polygon(rect.to_vec())],
        }));
        self.selected = Some(screen.layers.len() - 1);
        self.stage = Stage::Output;
        self.corner = 0;
        true
    }

    /// Inserts a point halfway between the selected mask's current point and the next one.
    pub fn add_mask_point(&mut self, setup: &mut OutputSetup) -> bool {
        let Some(i) = self.selected else { return false };
        let Some(ScreenLayer::Mask(mask)) = setup.screens.first_mut().and_then(|s| s.layers.get_mut(i)) else {
            return false;
        };
        let Some(c) = mask.contours.first_mut() else { return false };
        if c.points.is_empty() {
            return false;
        }
        // Flatten curves to lines so the new point sits on a straight segment.
        let mut points = c.flatten(1);
        let at = self.corner.min(points.len() - 1);
        let (a, b) = (points[at], points[(at + 1) % points.len()]);
        points.insert(at + 1, [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]);
        *c = Contour::polygon(points);
        self.corner = at + 1;
        true
    }

    /// Flips the selected mask between showing its inside (Resolume's Invert, the default) and
    /// hiding it.
    pub fn toggle_invert(&mut self, setup: &mut OutputSetup) -> bool {
        let Some(i) = self.selected else { return false };
        match setup.screens.first_mut().and_then(|s| s.layers.get_mut(i)) {
            Some(ScreenLayer::Mask(m)) => {
                m.invert = !m.invert;
                true
            }
            _ => false,
        }
    }

    /// Turns the selected slice or mask on or off.
    pub fn toggle_enabled(&mut self, setup: &mut OutputSetup, i: usize) -> bool {
        match setup.screens.first_mut().and_then(|s| s.layers.get_mut(i)) {
            Some(ScreenLayer::Slice(s)) => s.enabled = !s.enabled,
            Some(ScreenLayer::Mask(m)) => m.enabled = !m.enabled,
            None => return false,
        }
        true
    }

    pub fn rename_selected(&mut self, setup: &mut OutputSetup, name: &str) -> bool {
        let name = name.trim();
        let Some(i) = self.selected.filter(|_| !name.is_empty()) else { return false };
        match setup.screens.first_mut().and_then(|s| s.layers.get_mut(i)) {
            Some(ScreenLayer::Slice(s)) => s.name = name.into(),
            Some(ScreenLayer::Mask(m)) => m.name = name.into(),
            None => return false,
        }
        true
    }

    /// Moves the selected layer up (-1) or down (+1) the list; later layers draw on top.
    pub fn move_selected(&mut self, setup: &mut OutputSetup, delta: i32) -> bool {
        let (Some(i), Some(screen)) = (self.selected, setup.screens.first_mut()) else { return false };
        let j = i as i64 + delta as i64;
        if j < 0 || j as usize >= screen.layers.len() {
            return false;
        }
        screen.layers.swap(i, j as usize);
        self.selected = Some(j as usize);
        true
    }

    pub fn remove_selected(&mut self, setup: &mut OutputSetup) -> bool {
        let (Some(i), Some(screen)) = (self.selected, setup.screens.first_mut()) else { return false };
        if i >= screen.layers.len() {
            return false;
        }
        screen.layers.remove(i);
        self.selected = None;
        true
    }
}

fn dist(a: Point, b: Point) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Even-odd point in polygon.
fn inside(poly: &[Point], p: Point) -> bool {
    let mut inside = false;
    let mut j = poly.len().wrapping_sub(1);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// SVG path commands for a closed polygon.
pub fn path_commands(poly: &[Point]) -> String {
    let mut s = String::new();
    for (i, p) in poly.iter().enumerate() {
        s.push_str(if i == 0 { "M " } else { " L " });
        s.push_str(&format!("{:.1} {:.1}", p[0], p[1]));
    }
    if !poly.is_empty() {
        s.push_str(" Z");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> OutputSetup {
        OutputSetup::passthrough(1920, 1080)
    }

    fn dst(s: &OutputSetup) -> Quad {
        match &s.screens[0].layers[0] {
            ScreenLayer::Slice(sl) => sl.warp.dst,
            _ => unreachable!(),
        }
    }

    #[test]
    fn drag_a_corner_to_pin_it() {
        let mut s = setup();
        let mut e = Editor::default();
        // A press inside picks the slice; a second press on its corner grabs the corner.
        assert!(e.press(&mut s, [900.0, 500.0], 20.0));
        assert_eq!(e.selected, Some(0));
        e.release();
        assert!(e.press(&mut s, [1915.0, 6.0], 20.0));
        assert_eq!(e.corner, 1);
        e.drag_to(&mut s, [1700.0, 120.0]);
        assert!(e.release());
        assert_eq!(dst(&s)[1], [1700.0, 120.0]);
        // The warp now maps the slice's top right there.
        let ScreenLayer::Slice(sl) = &s.screens[0].layers[0] else { unreachable!() };
        let p = sl.warp.map(1.0, 0.0);
        assert!((p[0] - 1700.0).abs() < 0.1 && (p[1] - 120.0).abs() < 0.1);
    }

    #[test]
    fn nudge_type_and_reset() {
        let mut s = setup();
        let mut e = Editor { selected: Some(0), ..Default::default() };
        e.next_corner(&s);
        e.next_corner(&s);
        e.nudge(&mut s, [-10.0, 0.0]);
        assert_eq!(dst(&s)[2], [1910.0, 1080.0]);
        e.set_corner(&mut s, 3, 1, 900.0);
        assert_eq!(dst(&s)[3], [0.0, 900.0]);
        e.reset(&mut s);
        assert_eq!(dst(&s), rect_quad(0.0, 0.0, 1920.0, 1080.0));
    }

    #[test]
    fn move_an_input_rectangle() {
        let mut s = setup();
        let mut e = Editor { stage: Stage::Input, selected: Some(0), ..Default::default() };
        e.reset(&mut s);
        // Drag from the middle: the whole input moves.
        assert!(e.press(&mut s, [960.0, 540.0], 20.0));
        e.drag_to(&mut s, [1060.0, 540.0]);
        e.release();
        let ScreenLayer::Slice(sl) = &s.screens[0].layers[0] else { unreachable!() };
        assert_eq!(sl.input[0], [100.0, 0.0]);
        assert_eq!(sl.warp.dst, rect_quad(0.0, 0.0, 1920.0, 1080.0), "output untouched");
    }

    #[test]
    fn add_and_remove_slices() {
        let mut s = setup();
        let mut e = Editor::default();
        e.add_slice(&mut s);
        assert_eq!(e.selected, Some(1));
        assert_eq!(s.screens[0].slices().count(), 2);
        assert_eq!(s.screens[0].slices().nth(1).unwrap().name, "Slice 2");
        e.remove_selected(&mut s);
        assert_eq!(s.screens[0].slices().count(), 1);
        assert_eq!(e.selected, None);
    }

    #[test]
    fn edit_a_mask() {
        let mut s = setup();
        let mut e = Editor::default();
        assert!(e.add_mask(&mut s));
        assert_eq!(e.selected, Some(1));
        let screen = &s.screens[0];
        assert!(screen.shows([960.0, 540.0]) && !screen.shows([10.0, 10.0]), "inverted: shows its inside");
        // Drag its top-left point to the screen corner.
        assert!(e.press(&mut s, [482.0, 272.0], 20.0));
        e.drag_to(&mut s, [0.0, 0.0]);
        e.release();
        assert!(s.screens[0].shows([100.0, 100.0]));
        // Add a point after it, then invert: the mask now hides its inside.
        assert!(e.add_mask_point(&mut s));
        let ScreenLayer::Mask(m) = &s.screens[0].layers[1] else { unreachable!() };
        assert_eq!(m.contours[0].points.len(), 5);
        assert_eq!(m.contours[0].points[1], [720.0, 135.0]);
        e.toggle_invert(&mut s);
        assert!(!s.screens[0].shows([960.0, 540.0]) && s.screens[0].shows([1900.0, 1060.0]));
        e.toggle_enabled(&mut s, 1);
        assert!(s.screens[0].shows([960.0, 540.0]));
    }

    #[test]
    fn rename_and_reorder() {
        let mut s = setup();
        let mut e = Editor::default();
        e.add_mask(&mut s);
        assert!(e.rename_selected(&mut s, " Door "));
        assert!(e.move_selected(&mut s, -1));
        assert_eq!(e.selected, Some(0));
        let ScreenLayer::Mask(m) = &s.screens[0].layers[0] else { panic!("mask moved first") };
        assert_eq!(m.name, "Door");
        assert!(!e.move_selected(&mut s, -1));
    }

    #[test]
    fn path_commands_close_the_outline() {
        assert_eq!(path_commands(&[[0.0, 0.0], [10.0, 0.0], [10.0, 5.0]]), "M 0.0 0.0 L 10.0 0.0 L 10.0 5.0 Z");
    }
}
