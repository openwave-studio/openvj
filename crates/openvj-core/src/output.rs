//! Output mapping: how the composition reaches projectors and screens.
//!
//! The model follows Resolume's Advanced Output on purpose, so a Resolume setup imports one to one
//! and Resolume users find what they expect:
//!
//! - An [`OutputSetup`] holds one [`Screen`] per physical (or virtual) output.
//! - A screen draws a list of [`ScreenLayer`]s. A [`Slice`] takes a quad of the composition (its
//!   *input*) and draws it on the screen through a [`Warp`]. A [`Mask`] blacks out part of the screen.
//! - A [`Warp`] is a control grid (linear or smooth) followed by a perspective corner pin, exactly as
//!   Resolume stores it: the grid points live in the un-pinned output space and the homography maps
//!   that space onto the pinned corners.
//!
//! All coordinates are pixels: inputs in composition pixels, outputs in screen pixels. Quads are
//! listed clockwise from the top-left corner of the unrotated rectangle, the way Resolume writes
//! them, so a rotated input is just the same four corners in a different order.
//!
//! Everything here is pure CPU math so it can be tested headless. The renderer draws the meshes that
//! [`Slice::mesh`] produces.

use serde::{Deserialize, Serialize};

/// A 2D point in pixels.
pub type Point = [f32; 2];

/// Four corners, clockwise from top-left: top-left, top-right, bottom-right, bottom-left.
pub type Quad = [Point; 4];

/// Builds the quad of an axis-aligned rectangle.
pub fn rect_quad(x0: f32, y0: f32, x1: f32, y1: f32) -> Quad {
    [[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
}

/// Every output of a show, the equivalent of one Resolume Advanced Output preset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputSetup {
    pub name: String,
    /// The composition size the slices' input quads were laid out for.
    pub composition_size: [u32; 2],
    pub screens: Vec<Screen>,
    /// Exponent of the soft-edge falloff where slices overlap (Resolume's "Power", default 2).
    pub soft_edge_power: f32,
}

impl Default for OutputSetup {
    fn default() -> Self {
        Self { name: String::new(), composition_size: [1920, 1080], screens: Vec::new(), soft_edge_power: 2.0 }
    }
}

impl OutputSetup {
    /// A single screen showing the whole composition unwarped: what you get with no mapping at all.
    pub fn passthrough(width: u32, height: u32) -> Self {
        let full = rect_quad(0.0, 0.0, width as f32, height as f32);
        let slice = Slice::new(1, "Slice 1", full, full);
        let screen = Screen {
            id: 1,
            name: "Screen 1".into(),
            enabled: true,
            device: OutputDevice {
                kind: DeviceKind::Virtual,
                name: "Screen 1".into(),
                id: String::new(),
                width,
                height,
            },
            opacity: 1.0,
            color: ColorTrim::default(),
            layers: vec![ScreenLayer::Slice(slice)],
        };
        Self { composition_size: [width, height], screens: vec![screen], ..Self::default() }
    }

    /// Scales every input quad so the setup fits a composition of a different size.
    pub fn rescale_inputs(&mut self, width: u32, height: u32) {
        let [ow, oh] = self.composition_size;
        if ow == 0 || oh == 0 {
            return;
        }
        let (sx, sy) = (width as f32 / ow as f32, height as f32 / oh as f32);
        let scale = |p: &mut Point| *p = [p[0] * sx, p[1] * sy];
        for screen in &mut self.screens {
            for layer in &mut screen.layers {
                if let ScreenLayer::Slice(s) = layer {
                    s.input.iter_mut().for_each(scale);
                    if let Some(shape) = &mut s.shape {
                        shape.input.points_mut().for_each(scale);
                    }
                }
            }
        }
        self.composition_size = [width, height];
    }
}

/// One output, such as a projector, a monitor or a virtual (network) output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Screen {
    pub id: u64,
    pub name: String,
    pub enabled: bool,
    pub device: OutputDevice,
    pub opacity: f32,
    pub color: ColorTrim,
    /// Drawn in order, so later layers draw over earlier ones.
    pub layers: Vec<ScreenLayer>,
}

impl Screen {
    pub fn size(&self) -> [u32; 2] {
        [self.device.width, self.device.height]
    }

    pub fn slices(&self) -> impl Iterator<Item = &Slice> {
        self.layers.iter().filter_map(|l| match l {
            ScreenLayer::Slice(s) => Some(s),
            ScreenLayer::Mask(_) => None,
        })
    }

    /// Whether the screen's masks let the screen pixel `p` through.
    ///
    /// Inverted masks (Resolume's default) together form the visible area: a pixel shows when it is
    /// inside any of them. A non-inverted mask blacks out its inside on top of that. So a hexagon and
    /// two triangles, all inverted, show the content in all three shapes and black everywhere else,
    /// as in Resolume.
    pub fn shows(&self, p: Point) -> bool {
        let mut any_inverted = false;
        let mut in_inverted = false;
        for m in self.masks().filter(|m| m.enabled) {
            let inside = m.contains(p);
            if m.invert {
                any_inverted = true;
                in_inverted |= inside;
            } else if inside {
                return false;
            }
        }
        !any_inverted || in_inverted
    }

    pub fn masks(&self) -> impl Iterator<Item = &Mask> {
        self.layers.iter().filter_map(|l| match l {
            ScreenLayer::Mask(m) => Some(m),
            ScreenLayer::Slice(_) => None,
        })
    }
}

/// Where a screen's pixels go.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputDevice {
    pub kind: DeviceKind,
    /// The display or stream name, such as "Display 1".
    pub name: String,
    /// The machine-specific id of the device, kept so a setup can find the same display again.
    pub id: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    /// A physical display, usually fullscreen.
    Display,
    /// Not bound to hardware yet, or used only for previs.
    Virtual,
    Ndi,
    Spout,
    Syphon,
    /// A device type openVJ doesn't know, by its Resolume element name.
    Other(String),
}

/// Colour correction for a screen or a slice. Every field is an offset from -1 to 1, 0 is neutral.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ColorTrim {
    pub brightness: f32,
    pub contrast: f32,
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}

impl ColorTrim {
    pub fn is_neutral(&self) -> bool {
        *self == Self::default()
    }
}

/// An entry in a screen's layer list.
// A screen has a handful of layers, so the size difference between slices and masks doesn't matter
// and boxing would only make the type clumsier to build and match.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScreenLayer {
    Slice(Slice),
    Mask(Mask),
}

/// A piece of the composition drawn somewhere on a screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slice {
    pub id: u64,
    pub name: String,
    pub enabled: bool,
    /// What the slice shows. Resolume's "Input Source" value, kept as written: `0:1` is the
    /// composition, other values pick layers or other inputs.
    pub source: String,
    /// The part of the composition shown, in composition pixels.
    pub input: Quad,
    /// The slice's bounding rectangle on the screen after warping, in screen pixels.
    pub output: Quad,
    /// For a polygon slice (Resolume's "Polygon"), the outline cut from the input and where it lands.
    pub shape: Option<SliceShape>,
    pub warp: Warp,
    /// A mask that applies to this slice only.
    pub mask: Option<Mask>,
    pub color: ColorTrim,
    /// Fade this slice's edges where it overlaps others.
    pub soft_edge: bool,
    /// Resolume's flip bits: 1 flips horizontally, 2 vertically.
    pub flip: u8,
    /// Draw black behind the slice instead of letting slices below show through.
    pub black_bg: bool,
    /// Use the slice as a luma key for the layers below it.
    pub is_key: bool,
}

impl Slice {
    /// An unwarped slice from `input` (composition pixels) to `output` (screen pixels).
    pub fn new(id: u64, name: impl Into<String>, input: Quad, output: Quad) -> Self {
        Self {
            id,
            name: name.into(),
            enabled: true,
            source: "0:1".into(),
            input,
            output,
            shape: None,
            warp: Warp::identity(output, 4, 4),
            mask: None,
            color: ColorTrim::default(),
            soft_edge: false,
            flip: 0,
            black_bg: false,
            is_key: false,
        }
    }

    /// Tessellates the slice into a `cols`×`rows` grid of quads (two triangles each).
    ///
    /// Each vertex has its position on the screen (after the warp) and the composition pixel it
    /// samples. Use enough cells for smooth curves: 32×32 is plenty for a 4×4 bezier warp.
    pub fn mesh(&self, cols: u32, rows: u32) -> WarpMesh {
        let (cols, rows) = (cols.max(1), rows.max(1));
        let mut vertices = Vec::with_capacity(((cols + 1) * (rows + 1)) as usize);
        let flip_h = self.flip & 1 != 0;
        let flip_v = self.flip & 2 != 0;
        for r in 0..=rows {
            let v = r as f32 / rows as f32;
            for c in 0..=cols {
                let u = c as f32 / cols as f32;
                let su = if flip_h { 1.0 - u } else { u };
                let sv = if flip_v { 1.0 - v } else { v };
                vertices.push(MeshVertex { position: self.warp.map(u, v), uv: bilinear(&self.input, su, sv) });
            }
        }
        let mut indices = Vec::with_capacity((cols * rows * 6) as usize);
        let w = cols + 1;
        for r in 0..rows {
            for c in 0..cols {
                let i = r * w + c;
                indices.extend_from_slice(&[i, i + 1, i + w + 1, i, i + w + 1, i + w]);
            }
        }
        WarpMesh { vertices, indices }
    }
}

/// A polygon slice's outline: `input` is cut from the composition and drawn at `output`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SliceShape {
    pub input: Contour,
    pub output: Contour,
}

/// Blacks out part of a screen (or of one slice).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mask {
    pub id: u64,
    pub name: String,
    pub enabled: bool,
    /// Resolume's "Invert" flag, on by default: the mask shows only what is inside its shape.
    /// Off, it blacks out the inside instead. See [`Screen::shows`].
    pub invert: bool,
    /// Outlines in screen pixels (slice input pixels for a slice mask).
    pub contours: Vec<Contour>,
}

impl Mask {
    /// Whether `p` is inside the mask's shape (even-odd across its contours).
    pub fn contains(&self, p: Point) -> bool {
        self.contours.iter().filter(|c| c.contains(p)).count() % 2 == 1
    }

    /// Whether this mask on its own lets `p` through.
    pub fn shows(&self, p: Point) -> bool {
        !self.enabled || self.contains(p) == self.invert
    }
}

/// The kind of one contour segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentKind {
    Line,
    /// A cubic bezier, with two control points after its start point.
    Cubic,
}

/// A path of line and cubic bezier segments, stored the way Resolume writes it: each segment
/// contributes its start point, plus two control points for a cubic. The next segment's start (or
/// the first point, for the closing segment) is where a segment ends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contour {
    pub closed: bool,
    pub points: Vec<Point>,
    pub segments: Vec<SegmentKind>,
}

impl Contour {
    /// A closed polygon of straight lines.
    pub fn polygon(points: Vec<Point>) -> Self {
        let segments = vec![SegmentKind::Line; points.len()];
        Self { closed: true, points, segments }
    }

    fn points_mut(&mut self) -> impl Iterator<Item = &mut Point> {
        self.points.iter_mut()
    }

    /// The outline as straight lines, with each cubic split into `steps` pieces.
    pub fn flatten(&self, steps: u32) -> Vec<Point> {
        if self.segments.is_empty() {
            return self.points.clone();
        }
        let steps = steps.max(1);
        let mut out = Vec::new();
        let mut i = 0;
        for seg in &self.segments {
            let Some(&start) = self.points.get(i) else { break };
            out.push(start);
            let span = if *seg == SegmentKind::Cubic { 3 } else { 1 };
            let end = match self.points.get(i + span) {
                Some(&p) => p,
                None if self.closed => self.points[0],
                None => break,
            };
            if *seg == SegmentKind::Cubic && i + 2 < self.points.len() {
                let (c1, c2) = (self.points[i + 1], self.points[i + 2]);
                for s in 1..steps {
                    out.push(cubic(start, c1, c2, end, s as f32 / steps as f32));
                }
            }
            i += span;
        }
        // An open path ends on the point after its last segment.
        if !self.closed
            && let Some(&last) = self.points.get(i)
        {
            out.push(last);
        }
        out
    }

    /// Even-odd point-in-polygon test on the flattened outline.
    pub fn contains(&self, p: Point) -> bool {
        let poly = self.flatten(16);
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
}

/// How the points of a warp grid are joined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointMode {
    /// Straight lines between grid points (Resolume's `PM_LINEAR`).
    #[default]
    Linear,
    /// The grid is a bezier control net (Resolume's `PM_BEZIER`): corners are on the surface and
    /// the points between them pull it into a curve. A 4×4 grid is one bicubic patch, a 7×7 grid
    /// is 2×2 patches, and so on. Other grid sizes fall back to a smooth spline through the points.
    Bezier,
}

/// A slice's warp: a control grid, then a perspective corner pin from `src` to `dst`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Warp {
    /// The un-pinned corners, in screen pixels.
    pub src: Quad,
    /// Where the corners are pinned to, in screen pixels.
    pub dst: Quad,
    /// Grid columns (at least 2).
    pub cols: u32,
    /// Grid rows (at least 2).
    pub rows: u32,
    /// Grid points, row by row, in the un-pinned space of `src`.
    pub points: Vec<Point>,
    pub mode: PointMode,
}

impl Warp {
    /// A warp that changes nothing over `rect`, with a `cols`×`rows` grid ready for editing.
    pub fn identity(rect: Quad, cols: u32, rows: u32) -> Self {
        let (cols, rows) = (cols.max(2), rows.max(2));
        let mut points = Vec::with_capacity((cols * rows) as usize);
        for r in 0..rows {
            for c in 0..cols {
                points.push(bilinear(&rect, c as f32 / (cols - 1) as f32, r as f32 / (rows - 1) as f32));
            }
        }
        Self { src: rect, dst: rect, cols, rows, points, mode: PointMode::Linear }
    }

    /// The smallest and largest grid size the editor offers, per side.
    pub const GRID_MIN: u32 = 2;
    pub const GRID_MAX: u32 = 32;

    /// Changes the grid to `cols`×`rows` points, placing the new points on the surface the old
    /// grid describes, so the slice keeps its shape (exactly for linear grids, closely for bezier
    /// ones). The corner pin is untouched.
    pub fn resize(&mut self, cols: u32, rows: u32) {
        let (cols, rows) = (cols.clamp(Self::GRID_MIN, Self::GRID_MAX), rows.clamp(Self::GRID_MIN, Self::GRID_MAX));
        if (cols, rows) == (self.cols, self.rows) && self.points.len() == (cols * rows) as usize {
            return;
        }
        let mut points = Vec::with_capacity((cols * rows) as usize);
        for r in 0..rows {
            for c in 0..cols {
                points.push(self.grid_point(c as f32 / (cols - 1) as f32, r as f32 / (rows - 1) as f32));
            }
        }
        (self.cols, self.rows, self.points) = (cols, rows, points);
    }

    /// Back to a flat grid of the same size and mode, with the corners un-pinned.
    pub fn reset(&mut self) {
        let mode = self.mode;
        *self = Self { mode, ..Self::identity(self.src, self.cols, self.rows) };
    }

    /// Where grid point `i` (row by row) shows on the screen, after the corner pin.
    pub fn screen_point(&self, i: usize) -> Option<Point> {
        let p = *self.points.get(i)?;
        Some(Homography::from_quads(&self.src, &self.dst).map_or(p, |h| h.apply(p)))
    }

    /// Moves grid point `i` so it shows at `p` on the screen.
    pub fn set_screen_point(&mut self, i: usize, p: Point) -> bool {
        let q = Homography::from_quads(&self.dst, &self.src).map_or(p, |h| h.apply(p));
        match self.points.get_mut(i) {
            Some(slot) => {
                *slot = q;
                true
            }
            None => false,
        }
    }

    /// Whether grid point `i` is one of the four corners, which the corner pin moves instead.
    pub fn is_corner(&self, i: usize) -> bool {
        let (c, r) = (i % self.cols as usize, i / self.cols as usize);
        (c == 0 || c + 1 == self.cols as usize) && (r == 0 || r + 1 == self.rows as usize)
    }

    /// Whether the warp leaves the grid and corners where `identity` would put them.
    pub fn is_identity(&self) -> bool {
        let id = Self::identity(self.src, self.cols, self.rows);
        let close = |a: &Point, b: &Point| (a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01;
        self.src.iter().zip(&self.dst).all(|(a, b)| close(a, b))
            && self.points.len() == id.points.len()
            && self.points.iter().zip(&id.points).all(|(a, b)| close(a, b))
    }

    /// Where the slice point `(u, v)` (each 0 to 1 across the slice) lands on the screen.
    pub fn map(&self, u: f32, v: f32) -> Point {
        let p = self.grid_point(u, v);
        match Homography::from_quads(&self.src, &self.dst) {
            Some(h) => h.apply(p),
            None => p,
        }
    }

    /// The grid's position for `(u, v)` before the corner pin.
    pub fn grid_point(&self, u: f32, v: f32) -> Point {
        let (cols, rows) = (self.cols as usize, self.rows as usize);
        if cols < 2 || rows < 2 || self.points.len() != cols * rows {
            return bilinear(&self.src, u, v);
        }
        // Points past the edge of the grid are extrapolated in a straight line, so the smooth mode
        // keeps an evenly spaced grid perfectly flat right up to its edges.
        let get = |c: usize, r: usize| self.points[r * cols + c];
        let at = |c: isize, r: isize| {
            let edge = |i: isize, n: usize| -> (usize, usize, f32) {
                if i < 0 {
                    (0, 1, i as f32)
                } else if i as usize >= n {
                    (n - 1, n - 2, (i as usize - (n - 1)) as f32)
                } else {
                    (i as usize, i as usize, 0.0)
                }
            };
            let (c0, c1, kc) = edge(c, cols);
            let (r0, r1, kr) = edge(r, rows);
            let p = get(c0, r0);
            let (dc, dr) = (sub(p, get(c1, r0)), sub(p, get(c0, r1)));
            [p[0] + dc[0] * kc.abs() + dr[0] * kr.abs(), p[1] + dc[1] * kc.abs() + dr[1] * kr.abs()]
        };
        let fx = u.clamp(0.0, 1.0) * (cols - 1) as f32;
        let fy = v.clamp(0.0, 1.0) * (rows - 1) as f32;
        let cx = (fx.floor() as isize).min(cols as isize - 2);
        let cy = (fy.floor() as isize).min(rows as isize - 2);
        let (tx, ty) = (fx - cx as f32, fy - cy as f32);
        match self.mode {
            PointMode::Linear => bilinear(&[at(cx, cy), at(cx + 1, cy), at(cx + 1, cy + 1), at(cx, cy + 1)], tx, ty),
            PointMode::Bezier if (cols - 1) % 3 == 0 && (rows - 1) % 3 == 0 => {
                // One bicubic patch per 3×3 block of grid cells.
                let fx = u.clamp(0.0, 1.0) * ((cols - 1) / 3) as f32;
                let fy = v.clamp(0.0, 1.0) * ((rows - 1) / 3) as f32;
                let px = (fx.floor() as usize).min((cols - 1) / 3 - 1);
                let py = (fy.floor() as usize).min((rows - 1) / 3 - 1);
                let (tx, ty) = (fx - px as f32, fy - py as f32);
                let row = |r: usize| {
                    let c = px * 3;
                    cubic(get(c, r), get(c + 1, r), get(c + 2, r), get(c + 3, r), tx)
                };
                let r = py * 3;
                cubic(row(r), row(r + 1), row(r + 2), row(r + 3), ty)
            }
            PointMode::Bezier => {
                let row = |r: isize| catmull_rom(at(cx - 1, r), at(cx, r), at(cx + 1, r), at(cx + 2, r), tx);
                catmull_rom(row(cy - 1), row(cy), row(cy + 1), row(cy + 2), ty)
            }
        }
    }
}

/// A tessellated slice, ready to upload as a vertex and index buffer.
#[derive(Debug, Clone, PartialEq)]
pub struct WarpMesh {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshVertex {
    /// Screen pixels.
    pub position: Point,
    /// Composition pixels.
    pub uv: Point,
}

/// A 3×3 projective transform, row-major.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Homography(pub [[f64; 3]; 3]);

impl Homography {
    /// The transform taking each corner of `src` to the matching corner of `dst`, or `None` when
    /// either quad is degenerate (three corners in a line, or two on top of each other).
    pub fn from_quads(src: &Quad, dst: &Quad) -> Option<Self> {
        if is_degenerate(src) || is_degenerate(dst) {
            return None;
        }
        // Direct linear transform with h33 = 1: two equations per corner, eight unknowns.
        let mut a = [[0.0f64; 9]; 8];
        for i in 0..4 {
            let (x, y) = (src[i][0] as f64, src[i][1] as f64);
            let (dx, dy) = (dst[i][0] as f64, dst[i][1] as f64);
            a[2 * i] = [x, y, 1.0, 0.0, 0.0, 0.0, -x * dx, -y * dx, dx];
            a[2 * i + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -x * dy, -y * dy, dy];
        }
        let h = solve8(a)?;
        let m = [[h[0], h[1], h[2]], [h[3], h[4], h[5]], [h[6], h[7], 1.0]];
        m.iter().flatten().all(|v| v.is_finite()).then_some(Self(m))
    }

    pub fn apply(&self, p: Point) -> Point {
        let m = &self.0;
        let (x, y) = (p[0] as f64, p[1] as f64);
        let w = m[2][0] * x + m[2][1] * y + m[2][2];
        if w.abs() < 1e-12 {
            return p;
        }
        [((m[0][0] * x + m[0][1] * y + m[0][2]) / w) as f32, ((m[1][0] * x + m[1][1] * y + m[1][2]) / w) as f32]
    }
}

/// Gaussian elimination with partial pivoting on an 8×8 system (the 9th column is the right side).
fn solve8(mut a: [[f64; 9]; 8]) -> Option<[f64; 8]> {
    for col in 0..8 {
        let pivot = (col..8).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[pivot][col].abs() < 1e-10 {
            return None;
        }
        a.swap(col, pivot);
        for row in 0..8 {
            if row != col {
                let pivot_row = a[col];
                let f = a[row][col] / pivot_row[col];
                for (x, p) in a[row][col..].iter_mut().zip(&pivot_row[col..]) {
                    *x -= f * p;
                }
            }
        }
    }
    let mut x = [0.0; 8];
    for i in 0..8 {
        x[i] = a[i][8] / a[i][i];
    }
    Some(x)
}

/// The point at `(u, v)` inside a quad, interpolating its corners bilinearly.
pub fn bilinear(q: &Quad, u: f32, v: f32) -> Point {
    let top = lerp(q[0], q[1], u);
    let bottom = lerp(q[3], q[2], u);
    lerp(top, bottom, v)
}

/// Coverage images for masks, so the renderer can upload them as textures.
///
/// Pixels are sampled at their centres; 255 shows, 0 hides.
impl Screen {
    /// The screen's masks combined as [`Screen::shows`] does, at the screen's size, row by row.
    /// `None` when no mask is enabled, so the renderer can skip the texture.
    pub fn mask_image(&self) -> Option<Vec<u8>> {
        let masks: Vec<&Mask> = self.masks().filter(|m| m.enabled).collect();
        if masks.is_empty() {
            return None;
        }
        let [w, h] = self.size();
        let insides: Vec<Vec<bool>> = masks.iter().map(|m| m.inside_image(w, h)).collect();
        let any_inverted = masks.iter().any(|m| m.invert);
        let mut out = vec![0u8; (w * h) as usize];
        for (i, px) in out.iter_mut().enumerate() {
            let mut in_inverted = false;
            let mut hidden = false;
            for (m, inside) in masks.iter().zip(&insides) {
                if m.invert {
                    in_inverted |= inside[i];
                } else if inside[i] {
                    hidden = true;
                }
            }
            if !hidden && (!any_inverted || in_inverted) {
                *px = 255;
            }
        }
        Some(out)
    }
}

impl Slice {
    /// The slice's own mask over a composition of `size`, in composition pixels, or `None` without one.
    pub fn mask_image(&self, size: [u32; 2]) -> Option<Vec<u8>> {
        let mask = self.mask.as_ref().filter(|m| m.enabled)?;
        let [w, h] = size;
        let inside = mask.inside_image(w, h);
        Some(inside.into_iter().map(|i| if i == mask.invert { 255 } else { 0 }).collect())
    }
}

impl Mask {
    /// Which pixels of a `w`×`h` image are inside the mask's shape (even-odd across its contours).
    pub fn inside_image(&self, w: u32, h: u32) -> Vec<bool> {
        let polys: Vec<Vec<Point>> = self.contours.iter().map(|c| c.flatten(16)).collect();
        fill_even_odd(&polys, w, h)
    }
}

/// Scanline fill of closed polygons with the even-odd rule, sampled at pixel centres.
pub fn fill_even_odd(polys: &[Vec<Point>], w: u32, h: u32) -> Vec<bool> {
    let (w, h) = (w as usize, h as usize);
    let mut out = vec![false; w * h];
    let mut xs = Vec::new();
    for y in 0..h {
        let py = y as f32 + 0.5;
        xs.clear();
        for poly in polys {
            let n = poly.len();
            for i in 0..n {
                let (a, b) = (poly[i], poly[(i + 1) % n]);
                if (a[1] > py) != (b[1] > py) {
                    xs.push(a[0] + (py - a[1]) * (b[0] - a[0]) / (b[1] - a[1]));
                }
            }
        }
        xs.sort_by(f32::total_cmp);
        let row = &mut out[y * w..(y + 1) * w];
        for [a, b] in xs.as_chunks::<2>().0 {
            // Pixels whose centre lies between the two crossings.
            let x0 = (a - 0.5).ceil().max(0.0) as usize;
            let x1 = ((b - 0.5).ceil().max(0.0) as usize).min(w);
            if x0 < x1 {
                row[x0..x1].fill(true);
            }
        }
    }
    out
}

fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1]]
}

/// Twice the signed area of the triangle `abc`.
fn cross(a: Point, b: Point, c: Point) -> f64 {
    let (ab, ac) = (sub(b, a), sub(c, a));
    ab[0] as f64 * ac[1] as f64 - ab[1] as f64 * ac[0] as f64
}

/// Whether some three corners of the quad are (nearly) in a line.
fn is_degenerate(q: &Quad) -> bool {
    let size =
        q.iter().flat_map(|a| q.iter().map(move |b| (a[0] - b[0]).abs().max((a[1] - b[1]).abs()))).fold(0.0, f32::max)
            as f64;
    let eps = (size * size * 1e-6).max(1e-9);
    (0..4).any(|i| cross(q[i], q[(i + 1) % 4], q[(i + 2) % 4]).abs() < eps)
}

fn lerp(a: Point, b: Point, t: f32) -> Point {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn cubic(p0: Point, p1: Point, p2: Point, p3: Point, t: f32) -> Point {
    let s = 1.0 - t;
    let (a, b, c, d) = (s * s * s, 3.0 * s * s * t, 3.0 * s * t * t, t * t * t);
    [a * p0[0] + b * p1[0] + c * p2[0] + d * p3[0], a * p0[1] + b * p1[1] + c * p2[1] + d * p3[1]]
}

/// A Catmull-Rom spline through `p1` and `p2`, using `p0` and `p3` for the tangents.
fn catmull_rom(p0: Point, p1: Point, p2: Point, p3: Point, t: f32) -> Point {
    let (t2, t3) = (t * t, t * t * t);
    let f = |a: f32, b: f32, c: f32, d: f32| {
        0.5 * (2.0 * b + (c - a) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (3.0 * b - a - 3.0 * c + d) * t3)
    };
    [f(p0[0], p1[0], p2[0], p3[0]), f(p0[1], p1[1], p2[1], p3[1])]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_keeps_the_shape() {
        let rect = rect_quad(0.0, 0.0, 1920.0, 1080.0);
        let mut w = Warp::identity(rect, 4, 4);
        w.resize(7, 3);
        assert_eq!((w.cols, w.rows, w.points.len()), (7, 3, 21));
        assert!(w.is_identity(), "a flat grid stays flat");
        // Bend the middle column, then refine: the bent point stays on the surface.
        let mut w = Warp::identity(rect, 3, 3);
        w.points[4] = [1060.0, 540.0];
        let before = w.grid_point(0.5, 0.5);
        w.resize(5, 5);
        let after = w.points[12];
        assert!((after[0] - before[0]).abs() < 0.01 && (after[1] - before[1]).abs() < 0.01);
        w.resize(1, 99);
        assert_eq!((w.cols, w.rows), (Warp::GRID_MIN, Warp::GRID_MAX));
    }

    #[test]
    fn grid_points_follow_the_corner_pin() {
        let rect = rect_quad(0.0, 0.0, 1000.0, 1000.0);
        let mut w = Warp::identity(rect, 3, 3);
        w.dst = rect_quad(100.0, 100.0, 600.0, 600.0);
        let mid = w.screen_point(4).unwrap();
        assert!((mid[0] - 350.0).abs() < 0.01 && (mid[1] - 350.0).abs() < 0.01);
        assert!(w.set_screen_point(4, [400.0, 350.0]));
        assert!((w.points[4][0] - 600.0).abs() < 0.01, "stored before the pin");
        let p = w.map(0.5, 0.5);
        assert!((p[0] - 400.0).abs() < 0.01 && (p[1] - 350.0).abs() < 0.01);
        assert!(w.is_corner(0) && w.is_corner(2) && w.is_corner(8) && !w.is_corner(1) && !w.is_corner(4));
        w.reset();
        assert!(w.is_identity());
    }

    #[test]
    fn mask_images_match_shows() {
        let tri = Mask {
            id: 1,
            name: "tri".into(),
            enabled: true,
            invert: true,
            contours: vec![Contour::polygon(vec![[2.0, 2.0], [30.0, 4.0], [10.0, 20.0]])],
        };
        let hole = Mask {
            id: 2,
            name: "hole".into(),
            enabled: true,
            invert: false,
            contours: vec![Contour::polygon(vec![[8.0, 6.0], [14.0, 6.0], [14.0, 10.0], [8.0, 10.0]])],
        };
        let mut screen = OutputSetup::passthrough(32, 24).screens.remove(0);
        assert!(screen.mask_image().is_none());
        screen.layers.push(ScreenLayer::Mask(tri));
        screen.layers.push(ScreenLayer::Mask(hole));
        let img = screen.mask_image().unwrap();
        for y in 0..24 {
            for x in 0..32 {
                let p = [x as f32 + 0.5, y as f32 + 0.5];
                assert_eq!(img[y * 32 + x] == 255, screen.shows(p), "pixel {x},{y}");
            }
        }
        assert!(img.contains(&255) && img.contains(&0));
    }

    fn close(a: Point, b: Point) -> bool {
        (a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01
    }

    #[test]
    fn homography_maps_each_corner() {
        let src = rect_quad(0.0, 0.0, 1920.0, 1080.0);
        let dst = [[98.3, -1.9], [1617.6, 7.4], [1584.2, 1061.4], [-3.7, 966.8]];
        let h = Homography::from_quads(&src, &dst).unwrap();
        for i in 0..4 {
            assert!(close(h.apply(src[i]), dst[i]), "corner {i}");
        }
        // A projective map keeps straight lines straight: the centre goes to where the diagonals cross.
        let c = h.apply([960.0, 540.0]);
        let cross = |p: Point, q: Point, r: Point| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
        assert!(cross(dst[0], dst[2], c).abs() < 1.0 && cross(dst[1], dst[3], c).abs() < 1.0);
    }

    #[test]
    fn degenerate_quads_have_no_homography() {
        let src = rect_quad(0.0, 0.0, 10.0, 10.0);
        assert!(Homography::from_quads(&src, &[[0.0, 0.0], [5.0, 0.0], [10.0, 0.0], [0.0, 10.0]]).is_none());
    }

    #[test]
    fn identity_warp_changes_nothing() {
        let rect = rect_quad(100.0, 50.0, 500.0, 350.0);
        for mode in [PointMode::Linear, PointMode::Bezier] {
            let w = Warp { mode, ..Warp::identity(rect, 4, 4) };
            assert!(w.is_identity());
            for (u, v) in [(0.0, 0.0), (0.25, 0.7), (1.0, 1.0), (0.5, 0.5)] {
                assert!(close(w.map(u, v), [100.0 + 400.0 * u, 50.0 + 300.0 * v]), "{mode:?} {u},{v}");
            }
        }
    }

    #[test]
    fn grid_points_bend_the_slice() {
        let mut w = Warp::identity(rect_quad(0.0, 0.0, 300.0, 300.0), 4, 4);
        w.points[5] = [120.0, 100.0]; // the inner point at grid (1,1), normally at (100,100)
        assert!(close(w.map(1.0 / 3.0, 1.0 / 3.0), [120.0, 100.0]));
        w.mode = PointMode::Bezier;
        // In bezier mode the inner point is a control point: it pulls the surface towards it
        // without the surface passing through it.
        let p = w.map(1.0 / 3.0, 1.0 / 3.0);
        assert!(p[0] > 100.0 && p[0] < 120.0, "{p:?}");
        assert!(close(w.map(1.0, 1.0), [300.0, 300.0]) && close(w.map(0.0, 0.0), [0.0, 0.0]), "corners stay put");
    }

    #[test]
    fn bezier_grid_is_a_control_net() {
        // A 4×4 net that bends a 600-wide slice into a bulb, the way Resolume's PM_BEZIER warps do.
        // The corners sit 90 px in, and the edge handles 30 px out (a third of 90), so each edge
        // curve peaks exactly on the slice's bounding box.
        let mut w = Warp::identity(rect_quad(0.0, 0.0, 600.0, 600.0), 4, 4);
        w.mode = PointMode::Bezier;
        w.points = vec![
            [90.0, 90.0],
            [230.0, -30.0],
            [370.0, -30.0],
            [510.0, 90.0],
            [-30.0, 230.0],
            [220.0, 260.0],
            [380.0, 260.0],
            [630.0, 230.0],
            [-30.0, 370.0],
            [220.0, 340.0],
            [380.0, 340.0],
            [630.0, 370.0],
            [90.0, 510.0],
            [230.0, 630.0],
            [370.0, 630.0],
            [510.0, 510.0],
        ];
        let near = |a: Point, b: Point| (a[0] - b[0]).abs() < 0.5 && (a[1] - b[1]).abs() < 0.5;
        assert!(near(w.map(0.5, 0.0), [300.0, 0.0]), "the top edge peaks at y = 0");
        assert!(near(w.map(0.0, 0.5), [0.0, 300.0]), "the left edge peaks at x = 0");
        assert!(close(w.map(0.0, 0.0), [90.0, 90.0]), "corners sit on the surface");
    }

    #[test]
    fn corner_pin_and_grid_combine() {
        let rect = rect_quad(0.0, 0.0, 100.0, 100.0);
        let mut w = Warp::identity(rect, 2, 2);
        w.dst = rect_quad(10.0, 10.0, 60.0, 60.0);
        assert!(close(w.map(0.5, 0.5), [35.0, 35.0]));
        assert!(!w.is_identity());
    }

    #[test]
    fn slice_mesh_samples_the_input_rect() {
        let mut s = Slice::new(1, "Left", rect_quad(0.0, 0.0, 960.0, 1080.0), rect_quad(0.0, 0.0, 1920.0, 1080.0));
        let m = s.mesh(2, 2);
        assert_eq!((m.vertices.len(), m.indices.len()), (9, 24));
        assert!(close(m.vertices[4].position, [960.0, 540.0]));
        assert!(close(m.vertices[4].uv, [480.0, 540.0]));
        s.flip = 1;
        assert!(close(s.mesh(2, 2).vertices[0].uv, [960.0, 0.0]), "flipped horizontally");
    }

    #[test]
    fn contours_flatten_lines_and_curves() {
        let tri = Contour::polygon(vec![[0.0, 0.0], [100.0, 0.0], [50.0, 100.0]]);
        assert_eq!(tri.flatten(8).len(), 3);
        assert!(tri.contains([50.0, 30.0]) && !tri.contains([5.0, 90.0]));

        // Resolume's 4-segment bezier circle from a slice mask: anchor, two handles, next anchor...
        let circle = Contour {
            closed: true,
            points: vec![
                [960.0, 0.0],
                [1258.2, 0.0],
                [1500.0, 241.8],
                [1500.0, 540.0],
                [1500.0, 838.2],
                [1258.2, 1080.0],
                [960.0, 1080.0],
                [661.8, 1080.0],
                [420.0, 838.2],
                [420.0, 540.0],
                [420.0, 241.8],
                [661.8, 0.0],
            ],
            segments: vec![SegmentKind::Cubic; 4],
        };
        let pts = circle.flatten(4);
        assert_eq!(pts.len(), 16);
        assert!(close(pts[2], cubic([960.0, 0.0], [1258.2, 0.0], [1500.0, 241.8], [1500.0, 540.0], 0.5)));
        assert!(circle.contains([960.0, 540.0]) && !circle.contains([440.0, 20.0]));
    }

    #[test]
    fn masks_show_inside_when_inverted_and_hide_it_otherwise() {
        let square = |x: f32, invert: bool| Mask {
            id: 1,
            name: "Mask".into(),
            enabled: true,
            invert,
            contours: vec![Contour::polygon(rect_quad(x, 0.0, x + 10.0, 10.0).to_vec())],
        };
        let mut m = square(0.0, false);
        assert!(!m.shows([5.0, 5.0]) && m.shows([20.0, 5.0]));
        m.invert = true;
        assert!(m.shows([5.0, 5.0]) && !m.shows([20.0, 5.0]));
        m.enabled = false;
        assert!(m.shows([20.0, 5.0]));

        // On a screen, inverted masks add up: each one opens a window.
        let mut screen = OutputSetup::passthrough(100, 100).screens.remove(0);
        screen.layers.push(ScreenLayer::Mask(square(0.0, true)));
        screen.layers.push(ScreenLayer::Mask(square(20.0, true)));
        assert!(screen.shows([5.0, 5.0]) && screen.shows([25.0, 5.0]) && !screen.shows([15.0, 5.0]));
        screen.layers.push(ScreenLayer::Mask(square(22.0, false)));
        assert!(!screen.shows([25.0, 5.0]), "a plain mask cuts a hole in what the others show");
    }

    #[test]
    fn passthrough_and_rescale() {
        let mut setup = OutputSetup::passthrough(1920, 1080);
        assert_eq!(setup.screens[0].size(), [1920, 1080]);
        setup.rescale_inputs(3840, 2160);
        assert_eq!(setup.screens[0].slices().next().unwrap().input[2], [3840.0, 2160.0]);
        let json = serde_json::to_string(&setup).unwrap();
        assert_eq!(serde_json::from_str::<OutputSetup>(&json).unwrap(), setup);
    }
}
