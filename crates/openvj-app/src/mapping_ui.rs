//! The output mapping window: what it shows and what its controls do.

use std::rc::Rc;
use std::time::{Duration, Instant};

use openvj_core::{OutputSetup, PointMode, ScreenLayer};
use slint::winit_030::WinitWindowAccessor;
use slint::winit_030::winit::window::{Fullscreen, Window as WinitWindow};
use slint::{ComponentHandle, Image, ModelRc, VecModel};

use super::App;
use crate::mapping::{
    CORNER_NAMES, Monitor, Stage, color_bars, find_monitor, grid_handles, grid_lines, path_commands, polyline_commands,
};
use crate::{MainWindow, MapColor, MapCorner, MapHandle, MapLayer, MapScreen, MappingWindow, OutputWindow};

/// How close to a corner a press grabs it, as a share of the canvas's longer side.
const REACH: f32 = 0.015;

/// How often to look for displays being connected or removed.
const MONITOR_POLL: Duration = Duration::from_secs(2);

/// The output windows of screens 2 and up (screen 1 uses the main output window).
#[derive(Default)]
pub struct ScreenOutputs {
    windows: Vec<OutputWindow>,
    /// Per window (screen 1 first): the display it was last put on, once placed.
    placed: Vec<Option<Option<usize>>>,
    polled: Option<Instant>,
    /// Set when the VJ closes one of these windows: all output windows close, as when closing
    /// the main one.
    close: Rc<std::cell::Cell<bool>>,
}

fn list_monitors(w: &WinitWindow) -> Vec<Monitor> {
    w.available_monitors()
        .map(|m| Monitor { name: m.name().unwrap_or_default(), size: [m.size().width, m.size().height] })
        .collect()
}

/// An output window for an extra screen: F or a double-click toggles fullscreen, Esc leaves it.
fn new_output_window(close: Rc<std::cell::Cell<bool>>) -> Result<OutputWindow, slint::PlatformError> {
    let win = OutputWindow::new()?;
    let weak = win.as_weak();
    win.on_toggle_fullscreen(move || {
        if let Some(w) = weak.upgrade() {
            let fs = w.window().is_fullscreen();
            w.window().set_fullscreen(!fs);
        }
    });
    let weak = win.as_weak();
    win.on_exit_fullscreen(move || {
        if let Some(w) = weak.upgrade() {
            w.window().set_fullscreen(false);
        }
    });
    win.window().on_close_requested(move || {
        close.set(true);
        slint::CloseRequestResponse::KeepWindowShown
    });
    Ok(win)
}

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
                .and_then(|s| s.screens.get(self.mapping.editor.screen))
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

    /// Slice and mask outlines and the point being edited, on the window showing screen `index`,
    /// while the mapping window is open and editing that screen (Resolume draws them on the
    /// output while you edit).
    fn apply_outlines(&self, out: &OutputWindow, index: usize) {
        let ed = self.mapping.editor;
        let show = self.mapping_window_open && self.mapping.outlines && ed.screen == index;
        let screen = self.mapping.setup.as_ref().and_then(|s| s.screens.get(index));
        let Some(screen) = screen.filter(|_| show) else {
            if !out.get_outlines().is_empty() || out.get_show_point() {
                out.set_outlines("".into());
                out.set_show_point(false);
            }
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
        let paths = paths.join(" ");
        if out.get_outlines() != paths.as_str() {
            out.set_outlines(paths.into());
        }
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

    /// Keeps one output window per screen: `out` shows screen 1 and `outs` the rest. Each goes
    /// fullscreen on its screen's display when that display is connected, else stays a window.
    /// Call once per frame, after [`App::frame`].
    pub fn sync_screen_outputs(&mut self, ui: &MainWindow, out: &OutputWindow, outs: &mut ScreenOutputs) {
        if outs.polled.is_none_or(|t| t.elapsed() >= MONITOR_POLL) {
            outs.polled = Some(Instant::now());
            let monitors = ui.window().with_winit_window(list_monitors).unwrap_or_default();
            if monitors != self.mapping.monitors {
                self.mapping.monitors = monitors;
                outs.placed.clear(); // Place every window again.
                self.mapping_view_dirty = true;
            }
        }
        if outs.close.take() && self.output_open {
            out.window().set_fullscreen(false);
            let _ = out.hide();
            self.output_shown(false);
        }
        let screens = self.mapping.setup.as_ref().map(|s| s.screens.as_slice()).unwrap_or_default();
        let extra = screens.len().saturating_sub(1);
        while outs.windows.len() < extra {
            match new_output_window(outs.close.clone()) {
                Ok(w) => outs.windows.push(w),
                Err(e) => {
                    eprintln!("openVJ: could not open an output window: {e}");
                    break;
                }
            }
        }
        outs.placed.resize(outs.windows.len() + 1, None);
        for (i, win) in outs.windows.iter().enumerate() {
            let screen = screens.get(i + 1);
            let show = self.output_open && screen.is_some_and(|s| s.enabled);
            if !show {
                if win.window().is_visible() {
                    win.window().set_fullscreen(false);
                    let _ = win.hide();
                }
                outs.placed[i + 1] = None;
                continue;
            }
            if !win.window().is_visible() {
                let _ = win.show();
            }
            let name = screen.map(|s| s.name.as_str()).unwrap_or_default();
            if win.get_screen_name() != name {
                win.set_screen_name(name.into());
            }
            if let Some(t) = self.engine.screen(i + 1) {
                win.set_frame(Image::try_from(t.texture.clone()).expect("screen texture is importable"));
            }
            self.apply_outlines(win, i + 1);
            win.window().request_redraw();
        }
        if self.output_open {
            if let Some(s) = screens.first() {
                let name = s.name.as_str();
                if out.get_screen_name() != name {
                    out.set_screen_name(name.into());
                }
            }
            self.apply_outlines(out, 0);
        } else {
            outs.placed[0] = None;
        }
        // Fullscreen on the chosen display.
        let all = std::iter::once(out).chain(outs.windows.iter());
        for (i, (win, screen)) in all.zip(screens).enumerate() {
            if !win.window().is_visible() {
                continue;
            }
            let want = find_monitor(&screen.device, &self.mapping.monitors);
            if outs.placed[i] == Some(want) {
                continue;
            }
            let placed = win.window().with_winit_window(|w| {
                match want.and_then(|m| w.available_monitors().nth(m)) {
                    Some(m) => w.set_fullscreen(Some(Fullscreen::Borderless(Some(m)))),
                    // Display gone or none chosen: leave fullscreen if we put it there.
                    None if outs.placed[i].flatten().is_some() => w.set_fullscreen(None),
                    None => {}
                }
            });
            if placed.is_some() {
                outs.placed[i] = Some(want);
            }
        }
    }

    pub fn map_screen_picked(&mut self, i: i32) {
        self.edit_mapping(false, |e, s| e.pick_screen(s, i.max(0) as usize));
        self.images_dirty = true;
    }

    pub fn map_screen_toggled(&mut self, i: i32) {
        self.edit_mapping(true, |e, s| e.toggle_screen(s, i.max(0) as usize));
    }

    pub fn map_add_screen(&mut self) {
        self.edit_mapping(true, |e, s| e.add_screen(s));
        self.images_dirty = true;
    }

    pub fn map_remove_screen(&mut self) {
        self.edit_mapping(true, |e, s| e.remove_screen(s));
        self.images_dirty = true;
    }

    pub fn map_rename_screen(&mut self, name: &str) {
        self.edit_mapping(true, |e, s| e.rename_screen(s, name));
    }

    /// 0: a window; n: the nth connected display.
    pub fn map_display_picked(&mut self, choice: i32) {
        if choice < 0 || choice as usize > self.mapping.monitors.len() {
            return; // The "not connected" entry: keep the display it names.
        }
        let monitor = usize::try_from(choice - 1).ok().and_then(|i| self.mapping.monitors.get(i)).cloned();
        self.edit_mapping(true, |e, s| e.set_display(s, monitor.as_ref()));
        self.images_dirty = true;
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
    pub(super) fn sync_mapping_window(&mut self, images: bool) {
        self.mapping_view_dirty = false;
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
                Stage::Output => self.engine.screen(ed.screen),
            };
            if let Some(t) = target {
                win.set_picture(Image::try_from(t.texture.clone()).expect("texture is importable"));
            }
        }

        let screens = self.mapping.setup.as_ref().map(|s| s.screens.as_slice()).unwrap_or_default();
        let monitors = &self.mapping.monitors;
        let rows: Vec<MapScreen> = screens
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let [w, h] = s.size();
                let place = match (find_monitor(&s.device, monitors), &s.device.kind) {
                    (Some(m), _) => format!("Display {}", m + 1),
                    (None, openvj_core::DeviceKind::Display) => "not connected".into(),
                    _ => "window".into(),
                };
                MapScreen {
                    name: s.name.clone().into(),
                    info: format!("{w}×{h} · {place}").into(),
                    enabled: s.enabled,
                    selected: i == ed.screen,
                }
            })
            .collect();
        win.set_screens(ModelRc::from(Rc::new(VecModel::from(rows))));
        let screen = screens.get(ed.screen);
        win.set_screen_name(screen.map_or("Screen 1".into(), |s| s.name.clone()).into());
        win.set_can_remove_screen(screens.len() > 1);
        let mut displays = vec![slint::SharedString::from("Window")];
        displays.extend(
            monitors
                .iter()
                .enumerate()
                .map(|(i, m)| format!("Display {}: {} ({}×{})", i + 1, m.name, m.size[0], m.size[1]).into()),
        );
        let chosen = screen.and_then(|s| find_monitor(&s.device, monitors)).map_or(0, |m| m as i32 + 1);
        if let Some(s) = screen.filter(|s| s.device.kind == openvj_core::DeviceKind::Display && chosen == 0) {
            displays.push(format!("{} (not connected)", s.device.name).into());
        }
        let missing = displays.len() > monitors.len() + 1;
        win.set_displays(ModelRc::from(Rc::new(VecModel::from(displays))));
        win.set_display_index(if missing { monitors.len() as i32 + 1 } else { chosen });

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
