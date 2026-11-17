//! openVJ: an open-source, layer-based VJ mixer.

mod app;
mod browser;
mod discover;
mod library;
mod link;
mod mapping;
mod media_browser;
mod midi;
mod player;
mod prefs;
mod shadertoy_import;
mod synth_controls;
mod syphon_out;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use anyhow::{Context, Result};
use openvj_core::Composition;
use openvj_render::{Engine, wgpu};
use slint::{ComponentHandle, Timer, TimerMode};

use app::App;

slint::include_modules!();

/// Creates the GPU device shared by the engine and the UI renderer.
fn init_gpu() -> Result<(wgpu::Instance, wgpu::Adapter, wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .context("no GPU adapter found")?;
    let info = adapter.get_info();
    eprintln!("openVJ: GPU {} ({:?})", info.name, info.backend);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("openVJ"),
        // DXV and HAP frames are uploaded block-compressed when the GPU can sample BC.
        required_features: adapter.features() & wgpu::Features::TEXTURE_COMPRESSION_BC,
        required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
        ..Default::default()
    }))
    .context("could not open the GPU device")?;
    Ok((instance, adapter, device, queue))
}

/// Runs `f` with mutable app state, then pushes UI changes.
/// Skips silently if the state is already borrowed (e.g. a modal dialog is open).
fn with_app(
    app: &Rc<RefCell<App>>,
    ui: &slint::Weak<MainWindow>,
    out: &slint::Weak<OutputWindow>,
    f: impl FnOnce(&mut App),
) {
    let (Some(ui), Some(out)) = (ui.upgrade(), out.upgrade()) else { return };
    let Ok(mut app) = app.try_borrow_mut() else { return };
    f(&mut app);
    app.sync(&ui, &out);
}

fn comp_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("openVJ composition", &["json"]).set_title("openVJ composition")
}

fn playlist_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("openVJ playlist", &[openvj_core::PLAYLIST_EXTENSION])
}

/// Opens openVJ compositions and imports Resolume ones.
fn open_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new()
        .add_filter("Compositions", &["json", "avc"])
        .add_filter("openVJ composition", &["json"])
        .add_filter("Resolume composition", &["avc"])
        .set_title("Open composition")
}

/// macOS: windows created while this is set get no system title bar. Their content fills the
/// whole window and the close/minimise/zoom buttons sit over the app's own top bar, as in openDJ.
/// Only the main window uses it; the output and mapping windows keep their normal frame.
#[cfg(target_os = "macos")]
mod titlebar {
    use std::cell::Cell;

    use slint::winit_030::winit::platform::macos::WindowAttributesExtMacOS;
    use slint::winit_030::winit::window::WindowAttributes;

    /// Width the window buttons take on the left of the top bar.
    pub const INSET: f32 = 70.0;

    thread_local! {
        static HIDE: Cell<bool> = const { Cell::new(false) };
    }

    pub fn hook(attrs: WindowAttributes) -> WindowAttributes {
        if !HIDE.get() {
            return attrs;
        }
        attrs.with_titlebar_transparent(true).with_title_hidden(true).with_fullsize_content_view(true)
    }

    /// Runs `create` with the title bar hidden for the windows it makes.
    pub fn hidden<T>(create: impl FnOnce() -> T) -> T {
        HIDE.set(true);
        let made = create();
        HIDE.set(false);
        made
    }
}

fn main() -> Result<()> {
    let (instance, adapter, device, queue) = init_gpu()?;
    let selector = slint::BackendSelector::new();
    #[cfg(target_os = "macos")]
    let selector = selector.with_winit_window_attributes_hook(titlebar::hook);
    selector
        .require_wgpu_30(slint::wgpu_30::WGPUConfiguration::Manual {
            instance,
            adapter,
            device: device.clone(),
            queue: queue.clone(),
        })
        .select()
        .context("could not start the UI on this GPU")?;

    // ISF effects from ~/Documents/openVJ/Effects and the standard ISF folders, reloaded on change.
    openvj_core::effect_folders::watch(openvj_core::effect_folders::default_dirs(), openvj_render::check_effect);
    openvj_core::milkdrop::watch(openvj_core::milkdrop::default_dirs());

    // `openvj [composition.json | composition.avc | media folder]`
    let arg = std::env::args_os().nth(1).map(PathBuf::from);
    let comp = Composition::demo();
    let engine = Engine::new(device, queue, comp.width, comp.height);
    let app = Rc::new(RefCell::new(App::new(engine, comp)));

    #[cfg(target_os = "macos")]
    let ui = titlebar::hidden(MainWindow::new)?;
    #[cfg(not(target_os = "macos"))]
    let ui = MainWindow::new()?;
    #[cfg(target_os = "macos")]
    ui.set_titlebar_inset(titlebar::INSET);
    {
        let w = ui.as_weak();
        ui.on_toggle_maximized(move || {
            if let Some(ui) = w.upgrade() {
                let window = ui.window();
                window.set_maximized(!window.is_maximized());
            }
        });
    }
    let out = OutputWindow::new()?;
    // Open in the layout used last time (Mixer the first time).
    // OPENVJ_LAYOUT=n opens layout n (0 Mixer, 1 Player, 2 Synthesis, 3 Discover), for screenshots.
    ui.set_layout(std::env::var("OPENVJ_LAYOUT").ok().and_then(|v| v.parse().ok()).unwrap_or_else(prefs::layout));
    {
        let mut a = app.borrow_mut();
        a.attach(&ui);
        match &arg {
            Some(p) if p.is_dir() => a.set_browser_dir(p.clone()),
            Some(p) => a.open(p),
            None => {}
        }
        if !arg.as_ref().is_some_and(|p| p.is_file()) {
            // Start the demo with its first column playing and its first clip selected.
            a.column_clicked(0);
            a.cell_right_clicked(0, 0);
            a.status_message("Demo loaded. Click a clip to launch it, or a column header to launch the whole column.");
        }
        a.layout_changed(ui.get_layout());
        a.sync(&ui, &out);
    }

    let (w, o) = (ui.as_weak(), out.as_weak());
    macro_rules! on {
        ($setter:ident, |$app:ident $(, $arg:ident : $ty:ty)*| $body:expr) => {{
            let (app, w, o) = (app.clone(), w.clone(), o.clone());
            ui.$setter(move |$($arg: $ty),*| with_app(&app, &w, &o, |$app| $body));
        }};
    }

    on!(on_cell_clicked, |a, l: i32, c: i32| a.cell_clicked(l as usize, c as usize));
    on!(on_cell_pressed, |a, l: i32, c: i32| a.cell_pressed(l as usize, c as usize));
    on!(on_cell_released, |a, l: i32, c: i32| a.cell_released(l as usize, c as usize));
    on!(on_cell_right_clicked, |a, l: i32, c: i32| a.cell_right_clicked(l as usize, c as usize));
    on!(on_column_clicked, |a, c: i32| a.column_clicked(c as usize));
    on!(on_layer_selected, |a, l: i32| a.select_layer(l));
    on!(on_layer_opacity, |a, l: i32, v: f32| a.layer_opacity(l, v));
    on!(on_layer_blend, |a, l: i32, m: i32| a.layer_blend(l, m));
    on!(on_layer_bypass, |a, l: i32| a.layer_bypass(l));
    on!(on_layer_solo, |a, l: i32| a.layer_solo(l));
    on!(on_layer_clear, |a, l: i32| a.layer_clear(l));
    on!(on_clear_all, |a| a.clear_all());
    on!(on_add_layer, |a| a.add_layer());
    on!(on_add_column, |a| a.add_column());
    on!(on_deck_clicked, |a, i: i32| a.deck_clicked(i.max(0) as usize));
    on!(on_deck_add, |a| a.deck_add());
    on!(on_deck_renamed, |a, i: i32, t: slint::SharedString| a.deck_renamed(i.max(0) as usize, &t));
    on!(on_deck_remove, |a, i: i32| a.deck_remove(i.max(0) as usize));
    on!(on_remove_layer, |a| a.remove_layer());
    on!(on_tap, |a| a.tap());
    on!(on_nudge_bpm, |a, d: f32| a.nudge_bpm(d));
    on!(on_master_changed, |a, v: f32| a.master(v));
    on!(on_new_comp, |a| a.new_composition());
    on!(on_clear_mapping, |a| a.clear_mapping());
    on!(on_toggle_test_card, |a| a.toggle_test_card());
    on!(on_input_clicked, |a, i: i32| a.input_clicked(i as usize));
    on!(on_input_double_clicked, |a, i: i32| a.input_double_clicked(i as usize));
    on!(on_refresh_inputs, |a| a.refresh_inputs());
    on!(on_file_clicked, |a, i: i32| a.file_clicked(i as usize));
    on!(on_file_double_clicked, |a, i: i32| a.file_double_clicked(i as usize));
    on!(on_browser_up, |a| a.browser_up());
    on!(on_param_set, |a, s: i32, o: i32, i: i32, v: f32| a.param_set(s, o, i as usize, v));
    on!(on_param_mod, |a, s: i32, o: i32, i: i32, m: i32| a.param_mod(s, o, i as usize, m.max(0) as usize));
    on!(on_param_mod_amount, |a, s: i32, o: i32, i: i32, v: f32| a.param_mod_amount(s, o, i as usize, v));
    on!(on_param_mod_tempo, |a, s: i32, o: i32, i: i32, r: i32| a.param_mod_tempo(s, o, i as usize, r));
    on!(on_open_effects_folder, |a| a.open_effects_folder());

    // Drag and drop from the browser and library (ui/drag.slint).
    {
        let g = ui.global::<Drag>();
        g.on_payload(slint::DataTransfer::from);
        macro_rules! dropped {
            ($setter:ident, |$app:ident, $item:ident $(, $arg:ident : $ty:ty)*| $body:expr) => {{
                let (app, w, o) = (app.clone(), w.clone(), o.clone());
                g.$setter(move |$item: slint::SharedString $(, $arg: $ty)*| {
                    // The drag source may be rebuilt by the sync below and miss its
                    // drag-finished, so the drag ends here too.
                    if let Some(ui) = w.upgrade() {
                        ui.global::<Drag>().invoke_end();
                    }
                    with_app(&app, &w, &o, |$app| $body)
                });
            }};
        }
        dropped!(on_dropped_on_slot, |a, item, l: i32, c: i32| a.drop_on_slot(
            &item,
            l.max(0) as usize,
            c.max(0) as usize
        ));
        dropped!(on_dropped_on_layer, |a, item, l: i32| a.drop_on_layer(&item, l.max(0) as usize));
        dropped!(on_dropped_on_chain, |a, item, scope: i32| a.drop_on_chain(&item, scope));
    }

    // The library grid (ui/library.slint).
    {
        let g = ui.global::<Lib>();
        macro_rules! lib {
            ($setter:ident, |$app:ident $(, $arg:ident : $ty:ty)*| $body:expr) => {{
                let (app, w, o) = (app.clone(), w.clone(), o.clone());
                g.$setter(move |$($arg: $ty),*| with_app(&app, &w, &o, |$app| $body));
            }};
        }
        lib!(on_tile_clicked, |a, i: i32| a.lib_tile_clicked(i));
        lib!(on_tile_double_clicked, |a, i: i32| a.lib_tile_double_clicked(i));
        lib!(on_chip_clicked, |a, i: i32| a.lib_chip_clicked(i));
        lib!(on_rows_shown, |a, first: i32, last: i32| a.lib_rows_shown(first, last));
        lib!(on_set_intensity, |a, n: i32| a.lib_set_intensity(n));
        lib!(on_toggle_favorite, |a| a.lib_toggle_favorite());
        lib!(on_set_kept, |a, kept: bool| a.lib_set_kept(kept));
        lib!(on_set_unwanted, |a, unwanted: bool| a.lib_set_unwanted(unwanted));
        lib!(on_place, |a| a.lib_place());
        lib!(on_effect_add, |a, to: i32| a.lib_effect_add(to));
        lib!(on_use_preview_thumbnail, |a| a.lib_use_preview_thumbnail());
        lib!(on_done, |a| a.library_done());
        lib!(on_toggle_expanded, |a| a.lib_toggle_expanded());
        lib!(on_side_clicked, |a, i: i32| a.lib_side_clicked(i));
        lib!(on_add_tag, |a, t: slint::SharedString| a.lib_add_tag(&t));
        lib!(on_remove_tag, |a, i: i32| a.lib_remove_tag(i));
        lib!(on_add_to_crate, |a, t: slint::SharedString| a.lib_add_to_crate(&t));
        lib!(on_remove_from_crate, |a, i: i32| a.lib_remove_from_crate(i));
        lib!(on_add_to_playlist, |a| a.lib_add_to_playlist());
        let (app2, w2, o2) = (app.clone(), w.clone(), o.clone());
        g.on_key(move |text| {
            let mut handled = false;
            with_app(&app2, &w2, &o2, |a| handled = a.lib_key(&text));
            handled
        });
        let (app2, w2, o2) = (app.clone(), w.clone(), o.clone());
        g.on_filters_changed(move || {
            let Some(ui) = w2.upgrade() else { return };
            let g = ui.global::<Lib>();
            let (search, kind, intensity, reactive) =
                (g.get_search(), g.get_kind(), g.get_intensity(), g.get_reactive());
            let tab = ui.get_browser_tab();
            with_app(&app2, &w2, &o2, |a| a.lib_filters_changed(&search, kind, intensity, reactive, tab));
        });
    }

    on!(on_layout_changed, |a, i: i32| {
        prefs::set_layout(i);
        a.layout_changed(i);
    });

    // The Discover layout (ui/discover.slint).
    {
        let g = ui.global::<Disc>();
        macro_rules! disc {
            ($setter:ident, |$app:ident $(, $arg:ident : $ty:ty)*| $body:expr) => {{
                let (app, w, o) = (app.clone(), w.clone(), o.clone());
                g.$setter(move |$($arg: $ty),*| with_app(&app, &w, &o, |$app| $body));
            }};
        }
        disc!(on_pack_clicked, |a, i: i32| a.disc_pack_clicked(i));
        disc!(on_order_picked, |a, i: i32| a.disc_order_picked(i));
        disc!(on_param_set, |a, i: i32, v: f32| a.disc_param_set(i, v));
        disc!(on_lock_toggled, |a, i: i32| a.disc_lock_toggled(i));
        disc!(on_randomize, |a| a.disc_randomize());
        disc!(on_undo, |a| a.disc_undo());
        disc!(on_reset, |a| a.disc_reset());
        disc!(on_set_intensity, |a, n: i32| a.disc_set_intensity(n));
        disc!(on_add_tag, |a, t: slint::SharedString| a.disc_add_tag(&t));
        disc!(on_remove_tag, |a, i: i32| a.disc_remove_tag(i));
        disc!(on_crate_clicked, |a, i: i32| a.disc_crate_clicked(i));
        disc!(on_add_crate, |a, t: slint::SharedString| a.disc_add_crate(&t));
        disc!(on_decide, |a, i: i32| a.disc_decide(i));
        disc!(on_jump, |a, i: i32| a.disc_jump(i));
        let (app2, w2, o2) = (app.clone(), w.clone(), o.clone());
        g.on_key(move |text| {
            let mut handled = false;
            with_app(&app2, &w2, &o2, |a| handled = a.disc_key(&text));
            handled
        });
    }

    // MIDI map mode (MidiLearn in ui/state.slint).
    {
        let g = ui.global::<MidiLearn>();
        macro_rules! learn {
            ($setter:ident, |$app:ident $(, $arg:ident : $ty:ty)*| $body:expr) => {{
                let (app, w, o) = (app.clone(), w.clone(), o.clone());
                g.$setter(move |$($arg: $ty),*| with_app(&app, &w, &o, |$app| $body));
            }};
        }
        learn!(on_toggle, |a| a.learn_toggle());
        learn!(on_pick, |a, id: slint::SharedString| a.learn_pick(&id));
        learn!(on_forget, |a| a.learn_forget());
        learn!(on_behaviour_picked, |a, i: i32| a.learn_behaviour(i));
        learn!(on_range, |a, min: f32, max: f32| a.learn_range(min, max));
        // Evaluated while drawing, so never while the app is borrowed for an update.
        let app2 = app.clone();
        g.on_mappable(move |id, _| app2.try_borrow().is_ok_and(|a| a.learn_target(&id).is_some()));
        let app2 = app.clone();
        g.on_label(move |id, _| app2.try_borrow().map(|a| a.learn_label(&id).into()).unwrap_or_default());
    }

    // Import from Shadertoy (ui/shadertoy.slint).
    {
        let g = ui.global::<Shadertoy>();
        g.set_api_key(shadertoy_import::api_key().into());
        let w2 = w.clone();
        g.on_key_edited(move |key| {
            let Some(ui) = w2.upgrade() else { return };
            ui.global::<Shadertoy>().set_api_key(key.clone());
            if let Err(e) = shadertoy_import::set_api_key(&key) {
                eprintln!("openVJ: {e}");
            }
        });
        g.on_open_link(|url| {
            if let Err(e) = shadertoy_import::open_url(&url) {
                eprintln!("openVJ: couldn't open {url}: {e}");
            }
        });
        let w2 = w.clone();
        g.on_import(move || {
            let Some(ui) = w2.upgrade() else { return };
            let g = ui.global::<Shadertoy>();
            if g.get_busy() {
                return;
            }
            g.set_busy(true);
            g.set_message("".into());
            let request = shadertoy_import::Request { link: g.get_link().into(), code: g.get_code().into() };
            let w3 = w2.clone();
            shadertoy_import::start(request, move || {
                let _ = w3.upgrade_in_event_loop(|ui| ui.global::<Shadertoy>().invoke_finished());
            });
        });
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        g.on_finished(move || {
            let (Some(ui), Some(out)) = (w.upgrade(), o.upgrade()) else { return };
            let Ok(mut a) = app.try_borrow_mut() else { return };
            a.shadertoy_finished(&ui);
            a.sync(&ui, &out);
        });
    }

    on!(on_open_milk_folder, |a| a.open_milk_folder());
    on!(on_effect_toggle, |a, s: i32, i: i32| a.effect_toggle(s, i.max(0) as usize));
    on!(on_effect_remove, |a, s: i32, i: i32| a.effect_remove(s, i.max(0) as usize));
    on!(on_effect_move, |a, s: i32, i: i32, d: i32| a.effect_move(s, i.max(0) as usize, d));
    on!(on_audio_toggle, |a| a.audio_toggle());
    on!(on_audio_pick, |a, i: i32| a.audio_pick(i.max(0) as usize));
    on!(on_link_toggle, |a| a.link_toggle());
    on!(on_clock_toggle, |a| a.clock_toggle());
    on!(on_syphon_toggle, |a| a.syphon_toggle());
    on!(on_midi_toggle, |a| a.midi_toggle());
    on!(on_midi_clear, |a| a.midi_clear());
    on!(on_clip_renamed, |a, t: slint::SharedString| a.clip_renamed(&t));
    on!(on_clip_remove, |a| a.clip_remove());
    on!(on_clip_restart, |a| a.clip_restart());
    on!(on_layer_renamed, |a, t: slint::SharedString| a.layer_renamed(&t));
    on!(on_clip_play_mode_picked, |a, i: i32| a.clip_play_mode(i.max(0) as usize));
    on!(on_clip_autopilot_picked, |a, i: i32| a.clip_autopilot(i.max(0) as usize));
    on!(on_clip_router_input_picked, |a, i: i32| a.clip_router_input(i.max(0) as usize));
    on!(on_clip_trigger_picked, |a, i: i32| a.clip_trigger(i.max(0) as usize));
    on!(on_clip_synced_picked, |a, b: bool| a.clip_synced(b));
    on!(on_layer_side_picked, |a, i: i32| a.layer_side(i.max(0) as usize));
    on!(on_layer_group_picked, |a, i: i32| a.layer_group(i.max(0) as usize));
    on!(on_group_renamed, |a, t: slint::SharedString| a.group_renamed(&t));
    on!(on_group_blend, |a, i: i32| a.group_blend(i));
    on!(on_group_bypass_toggled, |a| a.group_bypass());
    on!(on_group_solo_toggled, |a| a.group_solo());
    on!(on_group_ungroup, |a| a.group_ungroup());
    on!(on_crossfade_curve_picked, |a, i: i32| a.crossfade_curve(i.max(0) as usize));

    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_key(move |text| {
            let mut handled = false;
            with_app(&app, &w, &o, |a| handled = a.key(&text));
            handled
        });
    }

    // File dialogs run their own modal loop, so never hold the app borrow across them.
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_import_mapping(move || {
            let dialog = rfd::FileDialog::new()
                .add_filter("Resolume Advanced Output preset", &["xml"])
                .set_title("Import a Resolume Advanced Output preset");
            if let Some(path) = dialog.pick_file() {
                with_app(&app, &w, &o, |a| a.import_mapping(&path));
            }
        });
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_open_comp(move || {
            if let Some(path) = open_dialog().pick_file() {
                with_app(&app, &w, &o, |a| a.open(&path));
            }
        });
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_save_comp(move || {
            let current = app.try_borrow().ok().and_then(|a| a.file());
            let path = current.or_else(|| comp_dialog().set_file_name("composition.json").save_file());
            if let Some(path) = path {
                with_app(&app, &w, &o, |a| a.save(&path));
            }
        });
    }
    // Synthesis layout.
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_synth_focus(move |l| with_app(&app, &w, &o, |a| a.synth_focus(l as usize)));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_synth_group(move |scope, owner, op| with_app(&app, &w, &o, |a| a.synth_group(scope, owner, op)));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_synth_row(move |row, op| with_app(&app, &w, &o, |a| a.synth_row(row, op)));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_synth_touch(move |scope, owner| with_app(&app, &w, &o, |a| a.synth_touch(scope, owner)));
    }
    // Synthesis media overlay and browser.
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_overlay_pick(move |i| with_app(&app, &w, &o, |a| a.overlay_pick(i as usize)));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_overlay_clear(move || with_app(&app, &w, &o, |a| a.overlay_clear()));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_overlay_transport(move |op| with_app(&app, &w, &o, |a| a.overlay_transport(op)));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_overlay_scrub(move |at| with_app(&app, &w, &o, |a| a.overlay_scrub(at)));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_overlay_play_mode_picked(move |i| with_app(&app, &w, &o, |a| a.overlay_play_mode(i as usize)));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_overlay_blend_picked(move |i| with_app(&app, &w, &o, |a| a.overlay_blend(i as usize)));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_media_chip_picked(move |i| with_app(&app, &w, &o, |a| a.media_chip(i as usize)));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_media_lock(move || with_app(&app, &w, &o, |a| a.media_lock()));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_media_refresh(move || with_app(&app, &w, &o, |a| a.media_refresh()));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_media_remove_folder(move |i| with_app(&app, &w, &o, |a| a.media_remove_folder(i as usize)));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_media_add_folder(move || {
            if let Some(dir) = rfd::FileDialog::new().set_title("Add a media folder").pick_folder() {
                with_app(&app, &w, &o, |a| a.media_add_folder(dir));
            }
        });
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_macro_toggle(move |m, scope, owner, index| {
            with_app(&app, &w, &o, |a| a.macro_toggle(m as usize, scope, owner, index as usize))
        });
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_macro_clear(move |m| with_app(&app, &w, &o, |a| a.macro_clear(m as usize)));
    }
    // Player layout.
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_automix_toggle(move || with_app(&app, &w, &o, |a| a.automix_toggle()));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_automix_next(move || with_app(&app, &w, &o, |a| a.automix_next()));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_playlist_add_selected(move || with_app(&app, &w, &o, |a| a.playlist_add_selected()));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_clip_to_playlist(move || with_app(&app, &w, &o, |a| a.playlist_add_selected()));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_playlist_remove(move |i| with_app(&app, &w, &o, |a| a.playlist_remove(i as usize)));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_playlist_move(move |from, to| with_app(&app, &w, &o, |a| a.playlist_move(from as usize, to)));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_playlist_play_for(move |i, k| with_app(&app, &w, &o, |a| a.playlist_play_for(i as usize, k as usize)));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_playlist_length_picked(move |i| with_app(&app, &w, &o, |a| a.playlist_length(i as usize)));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_playlist_style_picked(move |i| with_app(&app, &w, &o, |a| a.playlist_style(i as usize)));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_playlist_shuffle_toggled(move || with_app(&app, &w, &o, |a| a.playlist_shuffle()));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_playlist_repeat_toggled(move || with_app(&app, &w, &o, |a| a.playlist_repeat()));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_playlist_new(move || with_app(&app, &w, &o, |a| a.playlist_new()));
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_playlist_open(move || {
            if let Some(path) = playlist_dialog().set_title("Open playlist").pick_file() {
                with_app(&app, &w, &o, |a| a.playlist_open(&path));
            }
        });
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_playlist_save(move || {
            let current = app.try_borrow().ok().and_then(|a| a.playlist_file());
            let path = current.or_else(|| {
                playlist_dialog()
                    .set_title("Save playlist")
                    .set_file_name(format!("playlist.{}", openvj_core::PLAYLIST_EXTENSION))
                    .save_file()
            });
            if let Some(path) = path {
                with_app(&app, &w, &o, |a| a.playlist_save(&path));
            }
        });
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_relink_media(move || {
            if let Some(dir) = rfd::FileDialog::new().set_title("Folder with the missing media").pick_folder() {
                with_app(&app, &w, &o, |a| a.relink_media(&dir));
            }
        });
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_midi_import(move || {
            let mut dialog = rfd::FileDialog::new()
                .add_filter("Resolume MIDI preset", &["xml"])
                .set_title("Import Resolume MIDI preset");
            if let Some(dir) = openvj_resolume::midi_preset_dirs().into_iter().find(|d| d.is_dir()) {
                dialog = dialog.set_directory(dir);
            }
            if let Some(path) = dialog.pick_file() {
                with_app(&app, &w, &o, |a| a.midi_import(&path));
            }
        });
    }
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        ui.on_browser_choose_folder(move || {
            if let Some(dir) = rfd::FileDialog::new().set_title("Media folder").pick_folder() {
                with_app(&app, &w, &o, |a| a.set_browser_dir(dir));
            }
        });
    }

    // Output window.
    let show_output = {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        move |show: bool, fullscreen: Option<bool>| {
            let Some(out) = o.upgrade() else { return };
            if show {
                let _ = out.show();
                if let Some(fs) = fullscreen {
                    out.window().set_fullscreen(fs);
                }
            } else {
                out.window().set_fullscreen(false);
                let _ = out.hide();
            }
            with_app(&app, &w, &o, |a| a.output_shown(show));
        }
    };
    {
        let (app, show) = (app.clone(), show_output.clone());
        ui.on_toggle_output(move || {
            let open = app.try_borrow().map(|a| a.output_open).unwrap_or(false);
            show(!open, None);
        });
    }
    {
        let show = show_output.clone();
        ui.on_fullscreen_output(move || show(true, Some(true)));
    }
    {
        let o = o.clone();
        out.on_toggle_fullscreen(move || {
            if let Some(out) = o.upgrade() {
                let fs = out.window().is_fullscreen();
                out.window().set_fullscreen(!fs);
            }
        });
    }
    {
        let o = o.clone();
        out.on_exit_fullscreen(move || {
            if let Some(out) = o.upgrade() {
                out.window().set_fullscreen(false);
            }
        });
    }
    {
        let show = show_output.clone();
        out.window().on_close_requested(move || {
            show(false, None);
            slint::CloseRequestResponse::KeepWindowShown
        });
    }
    // Output mapping window.
    let mw = MappingWindow::new()?;
    {
        app.borrow_mut().attach_mapping(&mw);
        let show_mapping = {
            let (app, w, o, m) = (app.clone(), w.clone(), o.clone(), mw.as_weak());
            move |show: bool| {
                let Some(m) = m.upgrade() else { return };
                let _ = if show { m.show() } else { m.hide() };
                with_app(&app, &w, &o, |a| a.mapping_shown(show));
            }
        };
        {
            let show = show_mapping.clone();
            ui.on_open_mapping(move || show(true));
        }
        {
            let show = show_mapping.clone();
            mw.on_done(move || show(false));
        }
        {
            let show = show_mapping.clone();
            mw.window().on_close_requested(move || {
                show(false);
                slint::CloseRequestResponse::KeepWindowShown
            });
        }
        macro_rules! map_on {
            ($setter:ident, |$app:ident $(, $arg:ident : $ty:ty)*| $body:expr) => {{
                let (app, w, o) = (app.clone(), w.clone(), o.clone());
                mw.$setter(move |$($arg: $ty),*| with_app(&app, &w, &o, |$app| $body));
            }};
        }
        map_on!(on_stage_picked, |a, s: i32| a.map_stage(s));
        map_on!(on_layer_picked, |a, i: i32| a.map_layer_picked(i));
        map_on!(on_canvas_pressed, |a, x: f32, y: f32| a.map_pressed(x, y));
        map_on!(on_canvas_dragged, |a, x: f32, y: f32| a.map_dragged(x, y));
        map_on!(on_canvas_released, |a| a.map_released());
        map_on!(on_nudge, |a, x: f32, y: f32| a.map_nudge(x, y));
        map_on!(on_next_corner, |a| a.map_next_corner());
        map_on!(on_corner_edited, |a, c: i32, axis: i32, t: slint::SharedString| a.map_corner_edited(c, axis, &t));
        map_on!(on_reset_selected, |a| a.map_reset());
        map_on!(on_grid_size, |a, c: i32, r: i32| a.map_grid_size(c, r));
        map_on!(on_point_mode, |a, b: bool| a.map_point_mode(b));
        map_on!(on_add_slice, |a| a.map_add_slice());
        map_on!(on_remove_selected, |a| a.map_remove());
        map_on!(on_add_mask, |a| a.map_add_mask());
        map_on!(on_renamed, |a, t: slint::SharedString| a.map_rename(&t));
        map_on!(on_move_layer, |a, d: i32| a.map_move(d));
        map_on!(on_add_point, |a| a.map_add_point());
        map_on!(on_toggle_invert, |a| a.map_toggle_invert());
        map_on!(on_layer_toggled, |a, i: i32| a.map_layer_toggled(i));
        map_on!(on_toggle_test_card, |a| a.toggle_test_card());
        map_on!(on_toggle_outlines, |a| a.map_toggle_outlines());
        map_on!(on_toggle_soft_edge, |a| a.map_toggle_soft_edge());
        map_on!(on_soft_edge_changed, |a, c: i32, v: f32| a.map_soft_edge(c, v));
        map_on!(on_screen_picked, |a, i: i32| a.map_screen_picked(i));
        map_on!(on_screen_toggled, |a, i: i32| a.map_screen_toggled(i));
        map_on!(on_add_screen, |a| a.map_add_screen());
        map_on!(on_remove_screen, |a| a.map_remove_screen());
        map_on!(on_screen_renamed, |a, t: slint::SharedString| a.map_rename_screen(&t));
        map_on!(on_display_picked, |a, i: i32| a.map_display_picked(i));
        map_on!(on_color_changed, |a, slice: bool, c: i32, v: f32| a.map_color(slice, c, v));
        {
            let (app, w, o) = (app.clone(), w.clone(), o.clone());
            mw.on_import_preset(move || {
                let dialog = rfd::FileDialog::new()
                    .add_filter("Resolume Advanced Output preset", &["xml"])
                    .set_title("Import a Resolume Advanced Output preset");
                if let Some(path) = dialog.pick_file() {
                    with_app(&app, &w, &o, |a| a.import_mapping(&path));
                }
            });
        }
    }

    ui.window().on_close_requested(|| {
        let _ = slint::quit_event_loop();
        slint::CloseRequestResponse::HideWindow
    });

    // Render loop.
    let timer = Timer::default();
    {
        let (app, w, o) = (app.clone(), w.clone(), o.clone());
        let snapshot = std::env::var_os("OPENVJ_SNAPSHOT").map(PathBuf::from);
        let mut frame_count = 0u32;
        let mut screen_outputs = app::ScreenOutputs::default();
        timer.start(TimerMode::Repeated, Duration::from_millis(16), move || {
            let (Some(ui), Some(out)) = (w.upgrade(), o.upgrade()) else { return };
            if let Ok(mut a) = app.try_borrow_mut() {
                a.frame(&ui, &out);
                a.sync_screen_outputs(&ui, &out, &mut screen_outputs);
            }
            frame_count += 1;
            // Debug aid: OPENVJ_SNAPSHOT=out.png saves a screenshot of the window after ~2 s and quits.
            if let Some(path) = snapshot.as_ref().filter(|_| frame_count == 120) {
                match ui.window().take_snapshot() {
                    Ok(buf) => {
                        let (w, h) = (buf.width(), buf.height());
                        if let Err(e) = openvj_media::save_png(path, w, h, buf.as_bytes().to_vec()) {
                            eprintln!("openVJ: snapshot failed: {e:#}");
                        }
                    }
                    Err(e) => eprintln!("openVJ: snapshot failed: {e}"),
                }
                let _ = slint::quit_event_loop();
            }
        });
    }

    ui.show()?;
    ui.invoke_focus_keys();
    slint::run_event_loop()?;
    app.borrow_mut().library.save_now();
    Ok(())
}
