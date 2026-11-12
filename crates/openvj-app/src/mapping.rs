//! The output mapping the output window shows: saved per machine, like Resolume's Advanced Output,
//! not in the composition, so a show file opens with the venue's mapping.

use std::path::{Path, PathBuf};

use openvj_core::{Contour, Mask, OutputSetup, Point, PointMode, ScreenLayer, Slice, Warp, rect_quad};

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
    /// The points the selected layer shows as handles at this stage: a slice's four corners
    /// (then, on the output stage, its warp grid points), or a screen mask's outline points
    /// (output stage only).
    pub fn points(&self, layer: &ScreenLayer) -> Vec<Point> {
        match (layer, self.stage) {
            (ScreenLayer::Slice(s), Stage::Input) => s.input.to_vec(),
            (ScreenLayer::Slice(s), Stage::Output) => {
                let mut pts = s.warp.dst.to_vec();
                pts.extend(grid_handles(&s.warp).filter_map(|g| s.warp.screen_point(g)));
                pts
            }
            (ScreenLayer::Mask(m), Stage::Output) => m.contours.first().map(|c| c.points.clone()).unwrap_or_default(),
            (ScreenLayer::Mask(_), Stage::Input) => Vec::new(),
        }
    }

    /// The selected layer of screen 1.
    fn layer_mut<'a>(&self, setup: &'a mut OutputSetup) -> Option<&'a mut ScreenLayer> {
        setup.screens.first_mut()?.layers.get_mut(self.selected?)
    }

    /// The selected slice of screen 1, if a slice is selected.
    pub fn slice_mut<'a>(&self, setup: &'a mut OutputSetup) -> Option<&'a mut Slice> {
        match self.layer_mut(setup)? {
            ScreenLayer::Slice(s) => Some(s),
            ScreenLayer::Mask(_) => None,
        }
    }

    /// Moves point `i` of the selected layer (numbered as [`Editor::points`] lists them) to `p`.
    fn set_point(&self, setup: &mut OutputSetup, i: usize, p: Point) -> bool {
        let stage = self.stage;
        match (self.layer_mut(setup), stage) {
            (Some(ScreenLayer::Slice(s)), Stage::Input) => s.input.get_mut(i).map(|q| *q = p).is_some(),
            (Some(ScreenLayer::Slice(s)), Stage::Output) if i < 4 => {
                s.warp.dst[i] = p;
                true
            }
            (Some(ScreenLayer::Slice(s)), Stage::Output) => {
                let g = grid_handles(&s.warp).nth(i - 4);
                g.is_some_and(|g| s.warp.set_screen_point(g, p))
            }
            (Some(ScreenLayer::Mask(m)), Stage::Output) => {
                m.contours.first_mut().and_then(|c| c.points.get_mut(i)).map(|q| *q = p).is_some()
            }
            _ => false,
        }
    }

    /// Moves the whole selected layer by `d`: on the output stage a slice moves its corner pin,
    /// and its grid moves with it.
    fn shift(&self, setup: &mut OutputSetup, d: Point) -> bool {
        let stage = self.stage;
        let quad = |q: &mut [Point]| q.iter_mut().for_each(|q| *q = [q[0] + d[0], q[1] + d[1]]);
        match (self.layer_mut(setup), stage) {
            (Some(ScreenLayer::Slice(s)), Stage::Input) => quad(&mut s.input),
            (Some(ScreenLayer::Slice(s)), Stage::Output) => quad(&mut s.warp.dst),
            (Some(ScreenLayer::Mask(m)), Stage::Output) => match m.contours.first_mut() {
                Some(c) => quad(&mut c.points),
                None => return false,
            },
            _ => return false,
        }
        true
    }

    fn selected_points(&self, setup: &OutputSetup) -> Vec<Point> {
        self.selected.and_then(|i| setup.screens.first()?.layers.get(i)).map(|l| self.points(l)).unwrap_or_default()
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
        let points = self.selected_points(setup);
        // Corners come first in the list, so they win a tie with a grid point under them.
        let nearest = (0..points.len()).min_by(|&a, &b| dist(points[a], p).total_cmp(&dist(points[b], p)));
        if let Some(i) = nearest.filter(|&i| dist(points[i], p) <= reach) {
            self.corner = i;
            self.drag = Some(Drag::Corner(i));
            return true;
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
        match drag {
            Drag::Corner(i) => self.set_point(setup, i, p),
            Drag::Move { last } => {
                self.drag = Some(Drag::Move { last: p });
                self.shift(setup, [p[0] - last[0], p[1] - last[1]])
            }
        }
    }

    /// Ends a drag. Returns whether the setup changed and should be saved.
    pub fn release(&mut self) -> bool {
        self.drag.take().is_some()
    }

    /// Moves the current point by `d` pixels.
    pub fn nudge(&mut self, setup: &mut OutputSetup, d: Point) -> bool {
        let Some(q) = self.selected_points(setup).get(self.corner).copied() else { return false };
        self.set_point(setup, self.corner, [q[0] + d[0], q[1] + d[1]])
    }

    /// Types one coordinate (`axis` 0 = x, 1 = y) of a point.
    pub fn set_corner(&mut self, setup: &mut OutputSetup, corner: usize, axis: usize, value: f32) -> bool {
        let Some(mut q) = self.selected_points(setup).get(corner).copied() else { return false };
        q[axis.min(1)] = value;
        self.corner = corner;
        self.set_point(setup, corner, q)
    }

    pub fn next_corner(&mut self, setup: &OutputSetup) {
        let n = self.selected_points(setup).len().max(1);
        self.corner = (self.corner + 1) % n;
    }

    /// Output: back to the un-pinned corners and a flat grid. Input: the whole composition.
    pub fn reset(&mut self, setup: &mut OutputSetup) -> bool {
        let [w, h] = setup.composition_size;
        let stage = self.stage;
        let Some(i) = self.selected else { return false };
        let Some(ScreenLayer::Slice(slice)) = setup.screens.first_mut().and_then(|s| s.layers.get_mut(i)) else {
            return false;
        };
        match stage {
            Stage::Output => slice.warp.reset(),
            Stage::Input => slice.input = rect_quad(0.0, 0.0, w as f32, h as f32),
        }
        true
    }

    /// Sets the selected slice's warp grid to `cols`×`rows` points, keeping its shape.
    pub fn set_grid(&mut self, setup: &mut OutputSetup, cols: u32, rows: u32) -> bool {
        let Some(slice) = self.slice_mut(setup) else { return false };
        let before = (slice.warp.cols, slice.warp.rows);
        slice.warp.resize(cols, rows);
        let changed = before != (slice.warp.cols, slice.warp.rows);
        if changed && self.corner >= 4 {
            self.corner = 0; // The grid point it pointed at may be gone.
        }
        changed
    }

    /// Switches the selected slice's grid between straight lines and a bezier net.
    pub fn set_point_mode(&mut self, setup: &mut OutputSetup, mode: PointMode) -> bool {
        let Some(slice) = self.slice_mut(setup) else { return false };
        let changed = slice.warp.mode != mode;
        slice.warp.mode = mode;
        changed
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

/// The warp grid points the editor shows as handles: all but the four corners, which the corner
/// pin moves. Handle `4 + k` is the `k`th of these.
pub fn grid_handles(warp: &Warp) -> impl Iterator<Item = usize> + '_ {
    (0..warp.points.len()).filter(|&i| !warp.is_corner(i))
}

/// The inner lines of a slice's warp grid on the screen, as open polylines: one per grid row and
/// column between the edges.
pub fn grid_lines(warp: &Warp) -> Vec<Vec<Point>> {
    const N: u32 = 24;
    let (cols, rows) = (warp.cols.max(2), warp.rows.max(2));
    let t = |i: u32| i as f32 / N as f32;
    let mut lines = Vec::new();
    for r in 1..rows - 1 {
        let v = r as f32 / (rows - 1) as f32;
        lines.push((0..=N).map(|i| warp.map(t(i), v)).collect());
    }
    for c in 1..cols - 1 {
        let u = c as f32 / (cols - 1) as f32;
        lines.push((0..=N).map(|i| warp.map(u, t(i))).collect());
    }
    lines
}

/// SVG path commands for open polylines.
pub fn polyline_commands(lines: &[Vec<Point>]) -> String {
    let mut s = String::new();
    for line in lines {
        for (i, p) in line.iter().enumerate() {
            s.push_str(if i == 0 { " M " } else { " L " });
            s.push_str(&format!("{:.1} {:.1}", p[0], p[1]));
        }
    }
    s.trim_start().to_owned()
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
    use openvj_core::Quad;

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
    fn bend_a_slice_with_its_grid() {
        let mut s = setup();
        let mut e = Editor { selected: Some(0), ..Default::default() };
        assert!(e.set_grid(&mut s, 3, 3));
        assert!(!e.set_grid(&mut s, 3, 3), "same size: nothing to do");
        let slice = |s: &OutputSetup| match &s.screens[0].layers[0] {
            ScreenLayer::Slice(sl) => sl.clone(),
            _ => unreachable!(),
        };
        // 4 corners + 5 grid points (a 3×3 grid without its corners).
        let pts = e.points(&s.screens[0].layers[0]);
        assert_eq!(pts.len(), 9);
        // Grab the middle grid point and pull it right: the middle of the slice follows.
        assert!(e.press(&mut s, [960.0, 540.0], 20.0));
        assert_eq!(e.corner, 4 + 2, "middle point of the grid");
        e.drag_to(&mut s, [1060.0, 540.0]);
        assert!(e.release());
        let mid = slice(&s).warp.map(0.5, 0.5);
        assert!((mid[0] - 1060.0).abs() < 0.1, "{mid:?}");
        // Nudging and typing work on grid points too.
        e.nudge(&mut s, [0.0, 10.0]);
        e.set_corner(&mut s, 6, 0, 1000.0);
        let mid = slice(&s).warp.map(0.5, 0.5);
        assert!((mid[0] - 1000.0).abs() < 0.1 && (mid[1] - 550.0).abs() < 0.1, "{mid:?}");
        // Moving the corner pin carries the grid with it.
        e.press(&mut s, [5.0, 5.0], 20.0);
        e.drag_to(&mut s, [100.0, 0.0]);
        e.release();
        assert!(slice(&s).warp.map(0.5, 0.5)[0] > 1000.0);
        // Bezier mode keeps the points; reset flattens the grid and keeps its size and mode.
        assert!(e.set_point_mode(&mut s, PointMode::Bezier));
        e.reset(&mut s);
        let w = slice(&s).warp;
        assert!(w.is_identity() && w.cols == 3 && w.mode == PointMode::Bezier);
    }

    #[test]
    fn path_commands_close_the_outline() {
        assert_eq!(path_commands(&[[0.0, 0.0], [10.0, 0.0], [10.0, 5.0]]), "M 0.0 0.0 L 10.0 0.0 L 10.0 5.0 Z");
    }
}
