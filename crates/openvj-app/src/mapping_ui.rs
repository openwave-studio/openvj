//! The output mapping window: what it shows and what its controls do.

use std::rc::Rc;

use openvj_core::{OutputSetup, PointMode, ScreenLayer};
use slint::{ComponentHandle, Image, ModelRc, VecModel};

use super::App;
use crate::mapping::{CORNER_NAMES, Stage, color_bars, grid_handles, grid_lines, path_commands, polyline_commands};
use crate::{MapColor, MapCorner, MapHandle, MapLayer, MappingWindow, OutputWindow};

/// How close to a corner a press grabs it, as a share of the canvas's longer side.
const REACH: f32 = 0.015;

impl App {
    pub fn attach_mapping(&mut self, win: &MappingWindow) {
        self.mapping_ui = win.as_weak();
    }

    /// The mapping window was opened or closed.
    pub fn mapping_shown(&mut self, open: bool) {
        self.mapping_window_open = open;
        if open {
            self.ensure_setup();
            self.mapping_dirty = true;
            self.images_dirty = true;
        } else {
            self.mapping.editor.release();
        }
        self.mapping_view_dirty = true;
    }

    /// Editing starts from a passthrough screen when nothing is mapped yet, with the inputs laid out
    /// for the current composition.
    fn ensure_setup(&mut self) {
        let (w, h) = (self.comp.width, self.comp.height);
        let setup = self.mapping.setup.get_or_insert_with(|| OutputSetup::passthrough(w, h));
        if setup.composition_size != [w, h] {
            setup.rescale_inputs(w, h);
        }
    }

    /// Runs an edit on the setup; `save` writes it to disk when it changed.
    fn edit_mapping(&mut self, save: bool, f: impl FnOnce(&mut crate::mapping::Editor, &mut OutputSetup) -> bool) {
        self.ensure_setup();
        let Some(setup) = self.mapping.setup.as_mut() else { return };
        let mut editor = self.mapping.editor;
        let changed = f(&mut editor, setup);
        self.mapping.editor = editor;
        if changed && save {
            self.mapping.save();
            self.grid_dirty = true; // The settings page's summary.
        }
        self.mapping_view_dirty = true;
    }

    fn reach(&self) -> f32 {
        let [w, h] = self.canvas_size();
        w.max(h) * REACH
    }

    fn canvas_size(&self) -> [f32; 2] {
        let setup = self.mapping.setup.as_ref();
        match self.mapping.editor.stage {
            Stage::Input => [self.comp.width as f32, self.comp.height as f32],
            Stage::Output => setup
                .and_then(|s| s.screens.first())
                .map_or([self.comp.width as f32, self.comp.height as f32], |s| s.size().map(|v| v as f32)),
        }
    }

    pub fn map_stage(&mut self, stage: i32) {
        self.mapping.editor.stage = if stage == 0 { Stage::Input } else { Stage::Output };
        self.mapping.editor.release();
        self.mapping_view_dirty = true;
        self.images_dirty = true;
    }

    pub fn map_layer_picked(&mut self, i: i32) {
        self.mapping.editor.selected = usize::try_from(i).ok();
        self.mapping_view_dirty = true;
    }

    pub fn map_pressed(&mut self, x: f32, y: f32) {
        let reach = self.reach();
        self.edit_mapping(false, |e, s| e.press(s, [x, y], reach));
    }

    pub fn map_dragged(&mut self, x: f32, y: f32) {
        self.edit_mapping(false, |e, s| e.drag_to(s, [x, y]));
    }

    pub fn map_released(&mut self) {
        self.edit_mapping(true, |e, _| e.release());
    }

    pub fn map_nudge(&mut self, dx: f32, dy: f32) {
        self.edit_mapping(true, |e, s| e.nudge(s, [dx, dy]));
    }

    pub fn map_next_corner(&mut self) {
        if let Some(setup) = &self.mapping.setup {
            self.mapping.editor.next_corner(setup);
        }
        self.mapping_view_dirty = true;
    }

    pub fn map_corner_edited(&mut self, corner: i32, axis: i32, text: &str) {
        let Ok(v) = text.trim().replace(',', ".").parse::<f32>() else {
            self.mapping_view_dirty = true; // Put the old value back.
            return;
        };
        self.edit_mapping(true, |e, s| e.set_corner(s, corner.max(0) as usize, axis.max(0) as usize, v));
    }

    pub fn map_reset(&mut self) {
        self.edit_mapping(true, |e, s| e.reset(s));
    }

    pub fn map_grid_size(&mut self, cols: i32, rows: i32) {
        self.edit_mapping(true, |e, s| e.set_grid(s, cols.max(0) as u32, rows.max(0) as u32));
    }

    pub fn map_point_mode(&mut self, bezier: bool) {
        let mode = if bezier { PointMode::Bezier } else { PointMode::Linear };
        self.edit_mapping(true, |e, s| e.set_point_mode(s, mode));
    }

    /// A colour bar moved: `slice` true for the selected slice, false for the screen.
    pub fn map_color(&mut self, slice: bool, control: i32, bar: f32) {
        self.edit_mapping(true, |e, s| e.set_color(s, slice, control.max(0) as usize, bar));
    }

    pub fn map_toggle_outlines(&mut self) {
        self.mapping.outlines = !self.mapping.outlines;
        self.mapping_view_dirty = true;
    }

    /// Slice and mask outlines and the point being edited, on the output window, while the
    /// mapping window is open (Resolume draws them on the output while you edit).
    fn sync_output_outlines(&self, out: &OutputWindow) {
        let ed = self.mapping.editor;
        let screen = self.mapping.setup.as_ref().and_then(|s| s.screens.first());
        let show = self.mapping_window_open && self.mapping.outlines;
        let Some(screen) = screen.filter(|_| show) else {
            out.set_outlines("".into());
            out.set_show_point(false);
            return;
        };
        let [w, h] = screen.size();
        out.set_outline_w(w as f32);
        out.set_outline_h(h as f32);
        let out_ed = ed.at_stage(Stage::Output);
        let mut paths = Vec::new();
        for layer in &screen.layers {
            let on = match layer {
                ScreenLayer::Slice(s) => s.enabled,
                ScreenLayer::Mask(m) => m.enabled,
            };
            if on {
                paths.extend(out_ed.layer_outline(layer).iter().map(|o| path_commands(o)));
            }
        }
        out.set_outlines(paths.join(" ").into());
        let point = (ed.stage == Stage::Output)
            .then(|| ed.selected.and_then(|i| screen.layers.get(i)))
            .flatten()
            .and_then(|l| ed.points(l).get(ed.corner).copied());
        out.set_show_point(point.is_some());
        if let Some([x, y]) = point {
            out.set_point_x(x);
            out.set_point_y(y);
        }
    }

    pub fn map_add_slice(&mut self) {
        self.edit_mapping(true, |e, s| e.add_slice(s));
    }

    pub fn map_add_mask(&mut self) {
        self.edit_mapping(true, |e, s| e.add_mask(s));
        self.images_dirty = true; // The stage may have changed.
    }

    pub fn map_add_point(&mut self) {
        self.edit_mapping(true, |e, s| e.add_mask_point(s));
    }

    pub fn map_toggle_invert(&mut self) {
        self.edit_mapping(true, |e, s| e.toggle_invert(s));
    }

    pub fn map_layer_toggled(&mut self, i: i32) {
        self.edit_mapping(true, |e, s| e.toggle_enabled(s, i.max(0) as usize));
    }

    pub fn map_rename(&mut self, name: &str) {
        self.edit_mapping(true, |e, s| e.rename_selected(s, name));
    }

    pub fn map_move(&mut self, delta: i32) {
        self.edit_mapping(true, |e, s| e.move_selected(s, delta));
    }

    pub fn map_remove(&mut self) {
        self.edit_mapping(true, |e, s| e.remove_selected(s));
    }

    /// Pushes the setup to the mapping window. `images`: fetch the canvas picture again.
    pub(super) fn sync_mapping_window(&mut self, images: bool, out: &OutputWindow) {
        self.mapping_view_dirty = false;
        self.sync_output_outlines(out);
        let Some(win) = self.mapping_ui.upgrade() else { return };
        let ed = self.mapping.editor;
        let stage = ed.stage;
        win.set_stage(if stage == Stage::Input { 0 } else { 1 });
        win.set_test_card(self.mapping.test_card);
        win.set_outlines(self.mapping.outlines);
        let [cw, ch] = self.canvas_size();
        win.set_canvas_w(cw);
        win.set_canvas_h(ch);
        if images {
            let target = match stage {
                Stage::Input => Some(self.engine.output()),
                Stage::Output => self.engine.screen(0),
            };
            if let Some(t) = target {
                win.set_picture(Image::try_from(t.texture.clone()).expect("texture is importable"));
            }
        }

        let screen = self.mapping.setup.as_ref().and_then(|s| s.screens.first());
        win.set_screen_name(screen.map_or("Screen 1".into(), |s| s.name.clone()).into());
        win.set_screen_info(screen.map_or(String::new(), |s| format!("{}×{}", s.size()[0], s.size()[1])).into());

        let bars = |trim, opacity| {
            let rows: Vec<MapColor> = color_bars(trim, opacity)
                .into_iter()
                .map(|(name, value, text)| MapColor { name: name.into(), value, display: text.into() })
                .collect();
            ModelRc::from(Rc::new(VecModel::from(rows)))
        };
        if let Some(s) = screen {
            win.set_screen_colors(bars(&s.color, Some(s.opacity)));
        }
        let mut layers = Vec::new();
        let mut handles = Vec::new();
        let mut corners = Vec::new();
        let mut selected_name = String::new();
        let mut selected_mask: Option<bool> = None;
        let mut grid: Option<(u32, u32, bool, String)> = None;
        for (i, layer) in screen.map(|s| s.layers.as_slice()).unwrap_or_default().iter().enumerate() {
            let selected = ed.selected == Some(i);
            let (name, is_mask, enabled) = match layer {
                ScreenLayer::Slice(s) => (&s.name, false, s.enabled),
                ScreenLayer::Mask(m) => (&m.name, true, m.enabled),
            };
            let outline = ed.layer_outline(layer).iter().map(|o| path_commands(o)).collect::<Vec<_>>().join(" ");
            layers.push(MapLayer { name: name.clone().into(), is_mask, enabled, selected, outline: outline.into() });
            if !selected {
                continue;
            }
            selected_name = name.clone();
            let warp = match layer {
                ScreenLayer::Mask(m) => {
                    selected_mask = Some(m.invert);
                    None
                }
                ScreenLayer::Slice(s) if stage == Stage::Output => {
                    win.set_slice_colors(bars(&s.color, None));
                    let lines = polyline_commands(&grid_lines(&s.warp));
                    grid = Some((s.warp.cols, s.warp.rows, s.warp.mode == PointMode::Bezier, lines));
                    Some(&s.warp)
                }
                ScreenLayer::Slice(s) => {
                    win.set_slice_colors(bars(&s.color, None));
                    None
                }
            };
            let grid_index: Vec<usize> = warp.map(|w| grid_handles(w).collect()).unwrap_or_default();
            for (c, p) in ed.points(layer).iter().enumerate() {
                let is_grid = !is_mask && c >= 4;
                handles.push(MapHandle { x: p[0], y: p[1], selected: c == ed.corner, grid: is_grid });
                // The table lists the corners (or mask points), plus the grid point being edited.
                if is_grid && c != ed.corner {
                    continue;
                }
                let label = match (is_mask, warp) {
                    (true, _) => format!("Point {}", c + 1),
                    (false, Some(w)) if is_grid => {
                        let g = grid_index.get(c - 4).copied().unwrap_or(0);
                        format!("Grid {},{}", g % w.cols as usize + 1, g / w.cols as usize + 1)
                    }
                    _ => CORNER_NAMES[c % 4].to_string(),
                };
                corners.push(MapCorner {
                    name: label.into(),
                    index: c as i32,
                    x: format!("{:.0}", p[0]).into(),
                    y: format!("{:.0}", p[1]).into(),
                    selected: c == ed.corner,
                });
            }
        }
        win.set_has_grid(grid.is_some());
        if let Some((cols, rows, bezier, lines)) = grid {
            win.set_grid_cols(cols as i32);
            win.set_grid_rows(rows as i32);
            win.set_grid_bezier(bezier);
            win.set_grid_lines(lines.into());
        } else {
            win.set_grid_lines("".into());
        }
        win.set_selected_is_mask(selected_mask.is_some());
        win.set_selected_invert(selected_mask.unwrap_or(false));
        win.set_has_selection(!handles.is_empty());
        win.set_selected_name(selected_name.into());
        win.set_layers(ModelRc::from(Rc::new(VecModel::from(layers))));
        win.set_handles(ModelRc::from(Rc::new(VecModel::from(handles))));
        win.set_corners(ModelRc::from(Rc::new(VecModel::from(corners))));
        win.set_hint(
            match stage {
                Stage::Input => "Drag a slice to move what it takes from the composition, or a corner to reshape it",
                Stage::Output => {
                    "Drag a corner to pin it, or a small grid point to bend the slice · arrows nudge 1 px, Shift+arrows 10 px · Tab picks the next point"
                }
            }
            .into(),
        );
    }
}
