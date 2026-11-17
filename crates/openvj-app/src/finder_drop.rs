//! Files dragged in from the Finder (or another app) onto slots and layers.
//!
//! Slint's winit backend doesn't pass the system's file drags to `DropArea`s yet, and winit
//! 0.30 reports them without a position. So the hover and the drop are fed to Slint here as
//! drag events at the cursor: while files hover, the slot or layer under the cursor lights up
//! like for an in-app drag, and the drop lands on it through `Drag.dropped-on-slot` /
//! `Drag.dropped-on-layer` with the payload "finder" (see ui/drag.slint).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use i_slint_core::api::LogicalPosition;
use i_slint_core::input::BackendDragEvent;
use i_slint_core::items::{AllowedDragActions, DragAction, DropEvent};
use i_slint_core::window::WindowInner;
use slint::winit_030::{EventResult, WinitWindowAccessor, winit::event::WindowEvent};
use slint::{ComponentHandle, Timer, TimerMode};

use crate::{Drag, MainWindow};

#[derive(Default)]
struct State {
    files: Vec<PathBuf>,
    /// The drop was delivered; the remaining `DroppedFile` events of it are ignored.
    dropped: bool,
    /// Last cursor position from the window's own events, where the system has no other way
    /// to tell.
    cursor: Option<(f32, f32)>,
}

/// Hooks file drags on `ui`. `set_files` hands the hovering files to the app before the
/// drop arrives.
pub fn install(ui: &MainWindow, set_files: impl Fn(Vec<PathBuf>) + 'static) {
    let state = Rc::new(RefCell::new(State::default()));
    // While files hover, the hover position is fed to Slint on every tick.
    let hover = Rc::new(Timer::default());
    let w = ui.as_weak();
    ui.window().on_winit_window_event(move |window, event| {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let p = position.to_logical::<f32>(window.scale_factor().into());
                state.borrow_mut().cursor = Some((p.x, p.y));
            }
            WindowEvent::HoveredFile(path) => {
                let mut st = state.borrow_mut();
                if st.dropped {
                    *st = State { cursor: st.cursor, ..State::default() };
                }
                st.files.push(path.clone());
                set_files(st.files.clone());
                if let Some(ui) = w.upgrade() {
                    let name = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
                    let label = match st.files.len() {
                        1 => name,
                        n => format!("{n} files"),
                    };
                    ui.global::<Drag>().invoke_start("source".into(), "finder".into(), label.into());
                }
                if !hover.running() {
                    let (w, state) = (w.clone(), state.clone());
                    hover.start(TimerMode::Repeated, Duration::from_millis(30), move || {
                        let Some(ui) = w.upgrade() else { return };
                        let st = state.borrow();
                        if let Some(at) = cursor(ui.window(), st.cursor) {
                            send(ui.window(), &st.files, at, false);
                        }
                    });
                }
            }
            WindowEvent::DroppedFile(path) => {
                let mut st = state.borrow_mut();
                if st.dropped {
                    return EventResult::Propagate;
                }
                hover.stop();
                if st.files.is_empty() {
                    // No hover first (some systems): this file is the drop.
                    st.files.push(path.clone());
                    set_files(st.files.clone());
                    if let Some(ui) = w.upgrade() {
                        ui.global::<Drag>().invoke_start("source".into(), "finder".into(), "".into());
                    }
                }
                st.dropped = true;
                let files = st.files.clone();
                let at = cursor(window, st.cursor);
                drop(st);
                if let Some(at) = at {
                    send(window, &files, at, true);
                }
                if let Some(ui) = w.upgrade() {
                    ui.global::<Drag>().invoke_end();
                }
            }
            WindowEvent::HoveredFileCancelled => {
                hover.stop();
                let mut st = state.borrow_mut();
                *st = State { cursor: st.cursor, ..State::default() };
                WindowInner::from_pub(window).process_drag_event(BackendDragEvent::Leave);
                if let Some(ui) = w.upgrade() {
                    ui.global::<Drag>().invoke_end();
                }
            }
            _ => {}
        }
        EventResult::Propagate
    });
}

/// A hover (`drop` false) or the drop, at `at` in the window's logical coordinates.
fn send(window: &slint::Window, files: &[PathBuf], at: (f32, f32), drop: bool) {
    let mut data = slint::DataTransfer::default();
    data.set_file_paths(files.iter().cloned());
    let mut event = DropEvent::default();
    event.data = data;
    event.position = LogicalPosition::new(at.0, at.1);
    event.proposed_action = DragAction::Copy;
    let allowed = AllowedDragActions { copy: true, move_: false, link: false };
    let event =
        if drop { BackendDragEvent::Drop { event, allowed } } else { BackendDragEvent::Move { event, allowed } };
    WindowInner::from_pub(window).process_drag_event(event);
}

/// The cursor in the window's logical coordinates. During a system drag macOS sends the
/// window no mouse events, so it's asked for the cursor directly there.
#[cfg(target_os = "macos")]
fn cursor(window: &slint::Window, _last: Option<(f32, f32)>) -> Option<(f32, f32)> {
    use objc2_app_kit::NSView;
    use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    window
        .with_winit_window(|w| {
            let RawWindowHandle::AppKit(handle) = w.window_handle().ok()?.as_raw() else { return None };
            // SAFETY: winit's AppKit handle points at the window's live content NSView, and
            // this runs on the main thread inside winit's event loop.
            let view: &NSView = unsafe { handle.ns_view.cast().as_ref() };
            let in_window = view.window()?.mouseLocationOutsideOfEventStream();
            let p = view.convertPoint_fromView(in_window, None);
            let y = if view.isFlipped() { p.y } else { view.bounds().size.height - p.y };
            Some((p.x as f32, y as f32))
        })
        .flatten()
}

#[cfg(not(target_os = "macos"))]
fn cursor(_window: &slint::Window, last: Option<(f32, f32)>) -> Option<(f32, f32)> {
    last
}
