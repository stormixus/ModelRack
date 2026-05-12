use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::scanner;
use crate::strings;
use crate::view_model;
use crate::view_model::{
    browser_count_label_for_language, display_path_label, smart_filter_from_key, AppPrefs,
    AppViewSnapshot, BrowserCard as BrowserCardVm, CardLabelMode, DateFormatMode, Density,
    DisplayQuery, LibraryFilter, ScanStatus, SortBy, ViewMode,
};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use slint::winit_030::winit::dpi::PhysicalSize;
use slint::winit_030::WinitWindowAccessor;
use slint::{Color, Model, Rgba8Pixel, SharedPixelBuffer};

slint::include_modules!();

const DEFAULT_WINDOW_WIDTH: u32 = 1480;
const DEFAULT_WINDOW_HEIGHT: u32 = 920;
const MIN_WINDOW_WIDTH: u32 = 960;
const MIN_WINDOW_HEIGHT: u32 = 640;
const LIBRARY_WATCH_DEBOUNCE: Duration = Duration::from_millis(750);
const LIBRARY_WATCH_POLL_INTERVAL: Duration = Duration::from_millis(250);
const SCAN_ENTRY_BATCH_SIZE: usize = 8;
const SCAN_ENTRY_BATCH_INTERVAL: Duration = Duration::from_millis(120);
/// After this many models are queued during a single folder scan, new models
/// skip inline PNG generation and rely on disk cache hits only until the count
/// drops (never during the same scan) or the user opens a file (detail path
/// still calls `ensure_thumbnail`). Keeps huge libraries from rendering hundreds
/// of meshes on the scan thread.
const SCAN_INLINE_THUMBNAIL_MAX_LIBRARY_ENTRIES: usize = 512;
const PREVIEW_ORBIT_FRAME_INTERVAL: Duration = Duration::from_millis(16);
const PREVIEW_ORBIT_SETTLE_DELAY: Duration = Duration::from_millis(120);

const DEMO_ROOT: &str = "/Users/hwankishin/Library/3d";

pub fn run() -> Result<(), slint::PlatformError> {
    configure_slint_backend()?;
    crate::fonts::install_slint_fonts();

    let ui = ModelRackWindow::new()?;
    let state = Rc::new(RefCell::new(ShellState::load()));
    let watcher_runtime = Rc::new(RefCell::new(LibraryWatcherRuntime::new()));
    let scan_runtime = Rc::new(RefCell::new(LibraryScanRuntime::new()));
    let snapshot = state.borrow_mut().snapshot_idle();

    apply_snapshot(&ui, &snapshot);
    apply_detail_rc(&ui, &state);
    apply_settings(&ui, &state.borrow());
    // Respect the user's startup preference: when set to "empty", skip the
    // automatic last-library restore and present the empty state with its
    // clickable CTA. Default "last" preserves the historical behavior.
    let startup_mode = state.borrow().prefs.startup_view.clone();
    if startup_mode != "empty" {
        let restored_queue = state.borrow().restored_library_scan_queue();
        start_library_scan_queue(
            &ui,
            &state,
            &scan_runtime,
            &watcher_runtime,
            restored_queue,
            "Restoring last library",
        );
    }

    let weak = ui.as_weak();
    let open_state = state.clone();
    let open_watcher = watcher_runtime.clone();
    let open_scan = scan_runtime.clone();
    ui.on_open_folder(move || {
        if let Some(ui) = weak.upgrade() {
            choose_library_folder(&ui, &open_state, &open_scan, &open_watcher);
        }
    });

    let weak = ui.as_weak();
    let refresh_state = state.clone();
    let refresh_watcher = watcher_runtime.clone();
    let refresh_scan = scan_runtime.clone();
    ui.on_refresh_library(move || {
        if let Some(ui) = weak.upgrade() {
            request_library_folder_scans(
                &ui,
                &refresh_state,
                &refresh_scan,
                &refresh_watcher,
                "Refreshing library",
            );
        }
    });

    let weak = ui.as_weak();
    let search_state = state.clone();
    ui.on_apply_search(move |query| {
        if let Some(ui) = weak.upgrade() {
            let snapshot = {
                let mut state = search_state.borrow_mut();
                state.search_query = query.to_string();
                state.selected_index = None;
                state.snapshot_done()
            };
            apply_snapshot(&ui, &snapshot);
            apply_detail_rc(&ui, &search_state);
            apply_settings(&ui, &search_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let view_state = state.clone();
    ui.on_choose_view_mode(move |mode| {
        if let Some(ui) = weak.upgrade() {
            let snapshot = {
                let mut state = view_state.borrow_mut();
                state.choose_view_mode(mode.as_str());
                state.snapshot_done()
            };
            apply_snapshot(&ui, &snapshot);
            apply_settings(&ui, &view_state.borrow());
            save_prefs_status(&ui, &view_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let view_state = state.clone();
    ui.on_cycle_view_mode(move || {
        if let Some(ui) = weak.upgrade() {
            let snapshot = {
                let mut state = view_state.borrow_mut();
                state.cycle_view_mode();
                state.snapshot_done()
            };
            apply_snapshot(&ui, &snapshot);
            apply_settings(&ui, &view_state.borrow());
            save_prefs_status(&ui, &view_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let density_state = state.clone();
    ui.on_choose_density(move |density| {
        if let Some(ui) = weak.upgrade() {
            let snapshot = {
                let mut state = density_state.borrow_mut();
                state.choose_density(density.as_str());
                state.snapshot_done()
            };
            apply_snapshot(&ui, &snapshot);
            apply_settings(&ui, &density_state.borrow());
            save_prefs_status(&ui, &density_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let density_state = state.clone();
    ui.on_cycle_density(move || {
        if let Some(ui) = weak.upgrade() {
            let snapshot = {
                let mut state = density_state.borrow_mut();
                state.cycle_density();
                state.snapshot_done()
            };
            apply_snapshot(&ui, &snapshot);
            apply_settings(&ui, &density_state.borrow());
            save_prefs_status(&ui, &density_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let sort_state = state.clone();
    ui.on_toggle_sort(move || {
        if let Some(ui) = weak.upgrade() {
            let snapshot = {
                let mut state = sort_state.borrow_mut();
                state.sort_ascending = !state.sort_ascending;
                state.prefs.sort_ascending = state.sort_ascending;
                state.selected_index = None;
                state.snapshot_done()
            };
            apply_snapshot(&ui, &snapshot);
            apply_detail_rc(&ui, &sort_state);
            apply_settings(&ui, &sort_state.borrow());
            save_prefs_status(&ui, &sort_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let filter_state = state.clone();
    ui.on_choose_filter(move |key| {
        if let Some(ui) = weak.upgrade() {
            apply_filter_key(&ui, &filter_state, key.as_str());
        }
    });

    let weak = ui.as_weak();
    let toggle_folder_state = state.clone();
    ui.on_toggle_sidebar_folder(move |key| {
        if let Some(ui) = weak.upgrade() {
            toggle_sidebar_folder(&ui, &toggle_folder_state, key.as_str());
        }
    });

    let weak = ui.as_weak();
    let sidebar_context_state = state.clone();
    let sidebar_context_scan = scan_runtime.clone();
    ui.on_sidebar_context_action(move |action, key, label| {
        if let Some(ui) = weak.upgrade() {
            match action.as_str() {
                "filter" => apply_filter_key(&ui, &sidebar_context_state, key.as_str()),
                "reveal" => {
                    let Some(path) = folder_path_from_filter_key(key.as_str()) else {
                        ui.set_status_text("No folder path available for that sidebar item".into());
                        return;
                    };
                    match reveal_path_in_file_manager(&path) {
                        Ok(()) => {
                            ui.set_status_text(format!("Revealing {}", path.display()).into())
                        }
                        Err(err) => {
                            ui.set_status_text(format!("Could not reveal folder: {}", err).into())
                        }
                    }
                }
                "rescan" => {
                    let Some(path) = folder_path_from_filter_key(key.as_str()) else {
                        ui.set_status_text("No folder path available for that sidebar item".into());
                        return;
                    };
                    rescan_sidebar_folder(
                        &ui,
                        &sidebar_context_state,
                        &sidebar_context_scan,
                        &path,
                        label.as_str(),
                    );
                }
                "remove" => {
                    let Some(path) = folder_path_from_filter_key(key.as_str()) else {
                        ui.set_status_text("No folder path available for that sidebar item".into());
                        return;
                    };
                    remove_sidebar_folder_from_library(
                        &ui,
                        &sidebar_context_state,
                        &path,
                        label.as_str(),
                    );
                }
                "trash" => {
                    let Some(path) = folder_path_from_filter_key(key.as_str()) else {
                        ui.set_status_text("No folder path available for that sidebar item".into());
                        return;
                    };
                    move_sidebar_folder_to_trash(
                        &ui,
                        &sidebar_context_state,
                        &sidebar_context_scan,
                        &path,
                        label.as_str(),
                    );
                }
                "copy" => {
                    let text = folder_path_from_filter_key(key.as_str())
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| label.to_string());
                    match copy_text_to_clipboard(&text) {
                        Ok(()) => ui.set_status_text(format!("Copied {}", text).into()),
                        Err(err) => ui.set_status_text(format!("Could not copy: {}", err).into()),
                    }
                }
                _ => {}
            }
        }
    });

    let weak = ui.as_weak();
    let undo_state = state.clone();
    let undo_scan = scan_runtime.clone();
    ui.on_undo_library_action(move || {
        if let Some(ui) = weak.upgrade() {
            undo_library_action(&ui, &undo_state, &undo_scan);
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_open_settings(move || {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.settings_open = true;
            }
            apply_settings(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_close_settings(move || {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.settings_open = false;
            }
            apply_settings(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_choose_settings_tab(move |tab| {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.settings_tab = tab.to_string();
            }
            apply_settings(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_choose_settings_language(move |language| {
        if let Some(ui) = weak.upgrade() {
            let snapshot = {
                let mut state = settings_state.borrow_mut();
                state.choose_language(language.as_str());
                state.snapshot_done()
            };
            apply_snapshot(&ui, &snapshot);
            apply_detail_rc(&ui, &settings_state);
            apply_settings(&ui, &settings_state.borrow());
            save_prefs_status(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_choose_settings_theme(move |theme| {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.choose_theme(theme.as_str());
            }
            apply_settings(&ui, &settings_state.borrow());
            save_prefs_status(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_choose_settings_accent(move |accent| {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.choose_accent(accent.as_str());
            }
            apply_settings(&ui, &settings_state.borrow());
            save_prefs_status(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_choose_settings_sort(move |sort| {
        if let Some(ui) = weak.upgrade() {
            let snapshot = {
                let mut state = settings_state.borrow_mut();
                state.choose_sort(sort.as_str());
                state.selected_index = None;
                state.snapshot_done()
            };
            apply_snapshot(&ui, &snapshot);
            apply_detail_rc(&ui, &settings_state);
            apply_settings(&ui, &settings_state.borrow());
            save_prefs_status(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_choose_settings_thumbnail_style(move |style| {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.choose_thumbnail_style(style.as_str());
                ui.set_status_text("Thumbnail style preference updated".into());
            }
            apply_settings(&ui, &settings_state.borrow());
            save_prefs_status(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_choose_settings_thumbnail_lighting(move |lighting| {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.choose_thumbnail_lighting(lighting.as_str());
                ui.set_status_text("Thumbnail lighting preference updated".into());
            }
            apply_settings(&ui, &settings_state.borrow());
            save_prefs_status(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_choose_settings_thumbnail_aa(move |aa| {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.choose_thumbnail_aa(aa.as_str());
                ui.set_status_text("Thumbnail anti-aliasing preference updated".into());
            }
            apply_settings(&ui, &settings_state.borrow());
            save_prefs_status(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_choose_settings_card_label(move |key| {
        if let Some(ui) = weak.upgrade() {
            let snapshot = {
                let mut state = settings_state.borrow_mut();
                state.choose_card_label_mode(key.as_str());
                state.snapshot_done()
            };
            apply_snapshot(&ui, &snapshot);
            apply_settings(&ui, &settings_state.borrow());
            save_prefs_status(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_choose_settings_date_format(move |key| {
        if let Some(ui) = weak.upgrade() {
            let snapshot = {
                let mut state = settings_state.borrow_mut();
                state.choose_date_format_mode(key.as_str());
                state.snapshot_done()
            };
            apply_snapshot(&ui, &snapshot);
            apply_settings(&ui, &settings_state.borrow());
            save_prefs_status(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_toggle_settings_show_file_extensions(move |on| {
        if let Some(ui) = weak.upgrade() {
            let snapshot = {
                let mut state = settings_state.borrow_mut();
                state.set_show_file_extensions(on);
                state.snapshot_done()
            };
            apply_snapshot(&ui, &snapshot);
            apply_settings(&ui, &settings_state.borrow());
            save_prefs_status(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_choose_settings_startup(move |key| {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.choose_startup_view(key.as_str());
            }
            apply_settings(&ui, &settings_state.borrow());
            save_prefs_status(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_regenerate_thumbnails(move || {
        if let Some(ui) = weak.upgrade() {
            let language = ui.get_settings_language_key().to_string();
            let count = {
                let state = settings_state.borrow();
                state.entries.len()
            };
            match crate::thumbnail_cache::clear_all() {
                Ok(()) => {
                    ui.set_status_text(regenerate_thumbnails_status(&language, count, true).into());
                }
                Err(err) => {
                    ui.set_status_text(format!("Could not clear thumbnail cache: {}", err).into());
                }
            }
        }
    });

    let weak = ui.as_weak();
    ui.on_clear_thumbnail_cache(move || {
        if let Some(ui) = weak.upgrade() {
            let language = ui.get_settings_language_key().to_string();
            match crate::thumbnail_cache::clear_all() {
                Ok(()) => {
                    ui.set_status_text(clear_cache_status(&language).into());
                }
                Err(err) => {
                    ui.set_status_text(format!("Could not clear thumbnail cache: {}", err).into());
                }
            }
        }
    });

    let weak = ui.as_weak();
    ui.on_check_updates(move || {
        if let Some(ui) = weak.upgrade() {
            let language = ui.get_settings_language_key().to_string();
            ui.set_status_text(update_status_text_for_language(&language).into());
            ui.set_settings_update_status(update_status_text_for_language(&language).into());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_cycle_settings_language(move || {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.cycle_language();
            }
            apply_settings(&ui, &settings_state.borrow());
            save_prefs_status(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_toggle_settings_theme(move || {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.toggle_theme();
            }
            apply_settings(&ui, &settings_state.borrow());
            save_prefs_status(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_cycle_settings_density(move || {
        if let Some(ui) = weak.upgrade() {
            let snapshot = {
                let mut state = settings_state.borrow_mut();
                state.cycle_density();
                state.snapshot_done()
            };
            apply_snapshot(&ui, &snapshot);
            apply_settings(&ui, &settings_state.borrow());
            save_prefs_status(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_choose_slicer(move || {
        if let Some(ui) = weak.upgrade() {
            let selected = rfd::FileDialog::new()
                .set_title("Choose slicer")
                .pick_file()
                .or_else(|| {
                    rfd::FileDialog::new()
                        .set_title("Choose slicer app bundle")
                        .pick_folder()
                });
            if let Some(path) = selected {
                {
                    let mut state = settings_state.borrow_mut();
                    state.prefs.slicer_path = path.display().to_string();
                }
                apply_settings(&ui, &settings_state.borrow());
                save_prefs_status(&ui, &settings_state.borrow());
            }
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_choose_settings_slicer(move |path| {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.prefs.slicer_path = path.to_string();
                ui.set_status_text(slicer_status_text(&state.prefs.slicer_path).into());
            }
            apply_settings(&ui, &settings_state.borrow());
            save_prefs_status(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_toggle_settings_printer(move |key| {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                match state.toggle_printer_profile(&key) {
                    Ok(()) => ui.set_status_text(
                        settings_printer_status(&state.prefs, &load_printer_profiles()).into(),
                    ),
                    Err(message) => ui.set_status_text(message.into()),
                }
            }
            apply_detail_rc(&ui, &settings_state);
            apply_settings(&ui, &settings_state.borrow());
            save_prefs_status(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_choose_settings_printer_maker(move |maker| {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.choose_settings_printer_maker(maker.as_str());
            }
            apply_settings(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_choose_settings_printer_model(move |model| {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.choose_settings_printer_model(model.as_str());
            }
            apply_settings(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_choose_settings_printer_nozzle(move |nozzle| {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                if let Err(message) = state.choose_settings_printer_nozzle(nozzle.as_str()) {
                    ui.set_status_text(message.into());
                }
            }
            // Selecting a nozzle is a picker-only action — no prefs change,
            // no save. Re-render the settings panel so the Add button can
            // enable/disable.
            apply_settings(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_add_settings_printer(move || {
        if let Some(ui) = weak.upgrade() {
            let language = ui.get_settings_language_key().to_string();
            let status = {
                let mut state = settings_state.borrow_mut();
                match state.add_pending_printer() {
                    Ok(_) => settings_printer_status(&state.prefs, &load_printer_profiles()),
                    Err(message) => printer_add_error_text(&language, message),
                }
            };
            ui.set_status_text(status.into());
            apply_detail_rc(&ui, &settings_state);
            apply_settings(&ui, &settings_state.borrow());
            save_prefs_status(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_choose_settings_default_printer(move |key| {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.choose_default_printer(&key);
                ui.set_status_text(
                    settings_printer_status(&state.prefs, &load_printer_profiles()).into(),
                );
            }
            apply_detail_rc(&ui, &settings_state);
            apply_settings(&ui, &settings_state.borrow());
            save_prefs_status(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let estimate_state = state.clone();
    ui.on_choose_estimate_printer(move |key| {
        if let Some(ui) = weak.upgrade() {
            let mut state = estimate_state.borrow_mut();
            state.choose_estimate_printer(&key);
            apply_detail(&ui, &mut state);
        }
    });

    let weak = ui.as_weak();
    let select_state = state.clone();
    ui.on_select_model(move |index| {
        if let Some(ui) = weak.upgrade() {
            let mut state = select_state.borrow_mut();
            state.selected_index = Some(index as usize);
            state.reset_preview_orbit();
            state.reset_preview_plate();
            apply_detail(&ui, &mut state);
        }
    });

    let weak = ui.as_weak();
    let model_context_state = state.clone();
    ui.on_model_context_action(move |action, index| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if index < 0 {
            ui.set_status_text("Model context action has no target".into());
            return;
        }

        let mut state = model_context_state.borrow_mut();
        let index = index as usize;
        let Some(path) = state.displayed.get(index).map(|entry| entry.path.clone()) else {
            ui.set_status_text("Model is no longer available".into());
            return;
        };
        state.selected_index = Some(index);

        match action.as_str() {
            "open" => match launch_model(&path, &state.prefs.slicer_path) {
                Ok(()) => ui.set_status_text(format!("Opening {}", path.display()).into()),
                Err(err) => ui.set_status_text(format!("Could not open slicer: {}", err).into()),
            },
            "reveal" => match reveal_path_in_file_manager(&path) {
                Ok(()) => ui.set_status_text(format!("Revealed {}", path.display()).into()),
                Err(err) => ui.set_status_text(format!("Could not reveal model: {}", err).into()),
            },
            "favorite" => {
                let allow_sidecar_writes = state.sidecar_writes_enabled;
                let prefs = state.prefs.clone();
                match persist_favorite_toggle(
                    &prefs,
                    &mut state.entries,
                    &path,
                    allow_sidecar_writes,
                ) {
                    Ok(Some(favorite)) if allow_sidecar_writes => {
                        ui.set_status_text(
                            if favorite {
                                "Favorite saved"
                            } else {
                                "Favorite removed"
                            }
                            .into(),
                        );
                    }
                    Ok(Some(favorite)) => ui.set_status_text(
                        if favorite {
                            "Favorite updated for demo model"
                        } else {
                            "Favorite removed for demo model"
                        }
                        .into(),
                    ),
                    Ok(None) => ui.set_status_text("Selected model is no longer available".into()),
                    Err(err) => {
                        ui.set_status_text(format!("Could not save favorite: {}", err).into());
                        return;
                    }
                }

                let snapshot = state.snapshot_done();
                state.reselect_path(&path);
                apply_snapshot(&ui, &snapshot);
                apply_detail(&ui, &mut state);
                apply_settings(&ui, &state);
                return;
            }
            "print-plus" | "print-minus" => {
                let delta = if action.as_str() == "print-minus" {
                    -1
                } else {
                    1
                };
                let allow_sidecar_writes = state.sidecar_writes_enabled;
                let prefs = state.prefs.clone();
                match persist_print_count_delta(
                    &prefs,
                    &mut state.entries,
                    &path,
                    allow_sidecar_writes,
                    delta,
                ) {
                    Ok(Some(count)) if allow_sidecar_writes => {
                        ui.set_status_text(format!("Print count saved: {}", count).into())
                    }
                    Ok(Some(count)) => {
                        ui.set_status_text(format!("Demo print count updated: {}", count).into())
                    }
                    Ok(None) => ui.set_status_text("Selected model is no longer available".into()),
                    Err(err) => {
                        ui.set_status_text(format!("Could not update print count: {}", err).into());
                        return;
                    }
                }

                let snapshot = state.snapshot_done();
                state.reselect_path(&path);
                apply_snapshot(&ui, &snapshot);
                apply_detail(&ui, &mut state);
                apply_settings(&ui, &state);
                return;
            }
            "copy-path" => match copy_text_to_clipboard(&path.display().to_string()) {
                Ok(()) => ui.set_status_text("Copied model path".into()),
                Err(err) => ui.set_status_text(format!("Could not copy path: {}", err).into()),
            },
            "copy-name" => {
                let file_name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_string();
                match copy_text_to_clipboard(&file_name) {
                    Ok(()) => ui.set_status_text("Copied model filename".into()),
                    Err(err) => {
                        ui.set_status_text(format!("Could not copy filename: {}", err).into())
                    }
                }
            }
            _ => ui.set_status_text(format!("Unknown model action: {}", action).into()),
        }

        apply_detail(&ui, &mut state);
    });

    let weak = ui.as_weak();
    let plate_state = state.clone();
    ui.on_choose_preview_plate(move |index| {
        if let Some(ui) = weak.upgrade() {
            let mut state = plate_state.borrow_mut();
            state.choose_preview_plate(index);
            apply_detail(&ui, &mut state);
        }
    });

    let orbit_pending = Rc::new(RefCell::new(OrbitAccumulator::default()));
    let orbit_frame_timer = Rc::new(slint::Timer::default());
    let weak = ui.as_weak();
    let orbit_state = state.clone();
    let orbit_frame_pending = orbit_pending.clone();
    let orbit_frame_timer_for_callback = orbit_frame_timer.clone();
    orbit_frame_timer.start(
        slint::TimerMode::Repeated,
        PREVIEW_ORBIT_FRAME_INTERVAL,
        move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let Some((delta_x, delta_y)) = orbit_frame_pending.borrow_mut().take() else {
                orbit_frame_timer_for_callback.stop();
                return;
            };
            let mut state = orbit_state.borrow_mut();
            state.orbit_preview(delta_x, delta_y);
            apply_detail_with_quality(&ui, &mut state, DetailPreviewQuality::Interactive);
        },
    );

    let orbit_settle_timer = Rc::new(slint::Timer::default());
    let weak = ui.as_weak();
    let orbit_state = state.clone();
    let orbit_settle_pending = orbit_pending.clone();
    let orbit_frame_timer_for_orbit = orbit_frame_timer.clone();
    let orbit_settle_timer_for_callback = orbit_settle_timer.clone();
    ui.on_preview_orbit(move |delta_x, delta_y| {
        orbit_pending.borrow_mut().push(delta_x, delta_y);
        orbit_frame_timer_for_orbit.restart();
        let weak = weak.clone();
        let orbit_state = orbit_state.clone();
        let orbit_settle_pending = orbit_settle_pending.clone();
        orbit_settle_timer_for_callback.start(
            slint::TimerMode::SingleShot,
            PREVIEW_ORBIT_SETTLE_DELAY,
            move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                if let Some((delta_x, delta_y)) = orbit_settle_pending.borrow_mut().take() {
                    orbit_state.borrow_mut().orbit_preview(delta_x, delta_y);
                }
                let mut state = orbit_state.borrow_mut();
                apply_detail_with_quality(&ui, &mut state, DetailPreviewQuality::High);
            },
        );
    });

    let weak = ui.as_weak();
    let orbit_reset_state = state.clone();
    ui.on_preview_orbit_reset(move || {
        if let Some(ui) = weak.upgrade() {
            let mut state = orbit_reset_state.borrow_mut();
            state.reset_preview_orbit();
            apply_detail_with_quality(&ui, &mut state, DetailPreviewQuality::High);
        }
    });

    let weak = ui.as_weak();
    let fav_state = state.clone();
    ui.on_toggle_favorite(move || {
        if let Some(ui) = weak.upgrade() {
            let mut state = fav_state.borrow_mut();
            if let Some(idx) = state.selected_index {
                let selected_path = state.displayed.get(idx).map(|entry| entry.path.clone());
                if let Some(path) = selected_path {
                    let allow_sidecar_writes = state.sidecar_writes_enabled;
                    let prefs = state.prefs.clone();
                    match persist_favorite_toggle(
                        &prefs,
                        &mut state.entries,
                        &path,
                        allow_sidecar_writes,
                    ) {
                        Ok(Some(_)) if allow_sidecar_writes => {
                            ui.set_status_text("Favorite saved".into())
                        }
                        Ok(Some(_)) => ui.set_status_text("Favorite updated for demo model".into()),
                        Ok(None) => {}
                        Err(err) => {
                            ui.set_status_text(format!("Could not save favorite: {}", err).into());
                            return;
                        }
                    }
                }
                let snapshot = state.snapshot_done();
                apply_snapshot(&ui, &snapshot);
                apply_detail(&ui, &mut state);
            }
        }
    });

    let weak = ui.as_weak();
    let metadata_state = state.clone();
    ui.on_rename_model(move |name| {
        let Some(ui) = weak.upgrade() else {
            return false;
        };
        let mut state = metadata_state.borrow_mut();
        match state.rename_selected_model(name.as_str()) {
            Ok(Some(new_path)) => {
                let snapshot = state.snapshot_done();
                state.reselect_path(&new_path);
                ui.set_status_text(format!("Renamed to {}", new_path.display()).into());
                apply_snapshot(&ui, &snapshot);
                apply_detail(&ui, &mut state);
                apply_settings(&ui, &state);
                true
            }
            Ok(None) => {
                ui.set_status_text("Select a model before renaming".into());
                false
            }
            Err(err) => {
                ui.set_status_text(format!("Could not rename model: {}", err).into());
                false
            }
        }
    });

    let weak = ui.as_weak();
    let metadata_state = state.clone();
    ui.on_save_metadata(move |tags, author, notes| {
        if let Some(ui) = weak.upgrade() {
            let mut state = metadata_state.borrow_mut();
            let Some(path) = state.selected_model_path() else {
                ui.set_status_text("Select a model before saving metadata".into());
                return;
            };

            let allow_sidecar_writes = state.sidecar_writes_enabled;
            let prefs = state.prefs.clone();
            match persist_metadata_fields(
                &prefs,
                &mut state.entries,
                &path,
                allow_sidecar_writes,
                tags.as_str(),
                author.as_str(),
                notes.as_str(),
            ) {
                Ok(Some(_)) if allow_sidecar_writes => ui.set_status_text("Metadata saved".into()),
                Ok(Some(_)) => ui.set_status_text("Metadata updated for demo model".into()),
                Ok(None) => ui.set_status_text("Selected model is no longer available".into()),
                Err(err) => {
                    ui.set_status_text(format!("Could not save metadata: {}", err).into());
                    return;
                }
            }

            let snapshot = state.snapshot_done();
            state.reselect_path(&path);
            apply_snapshot(&ui, &snapshot);
            apply_detail(&ui, &mut state);
            apply_settings(&ui, &state);
        }
    });

    let weak = ui.as_weak();
    let tag_state = state.clone();
    ui.on_add_tags(move |tags| {
        if let Some(ui) = weak.upgrade() {
            let mut state = tag_state.borrow_mut();
            let Some(path) = state.selected_model_path() else {
                ui.set_status_text("Select a model before adding tags".into());
                return;
            };

            let allow_sidecar_writes = state.sidecar_writes_enabled;
            let prefs = state.prefs.clone();
            match persist_add_tags(
                &prefs,
                &mut state.entries,
                &path,
                allow_sidecar_writes,
                tags.as_str(),
            ) {
                Ok(Some(count)) if allow_sidecar_writes => {
                    ui.set_status_text(format!("Tags saved: {}", count).into())
                }
                Ok(Some(count)) => {
                    ui.set_status_text(format!("Demo tags updated: {}", count).into())
                }
                Ok(None) => ui.set_status_text("Selected model is no longer available".into()),
                Err(err) => {
                    ui.set_status_text(format!("Could not add tags: {}", err).into());
                    return;
                }
            }

            let snapshot = state.snapshot_done();
            state.reselect_path(&path);
            apply_snapshot(&ui, &snapshot);
            apply_detail(&ui, &mut state);
            apply_settings(&ui, &state);
        }
    });

    let weak = ui.as_weak();
    let drop_tag_state = state.clone();
    ui.on_add_tag_to_model(move |model_index, tag| {
        if let Some(ui) = weak.upgrade() {
            let mut state = drop_tag_state.borrow_mut();
            let Some(path) = state.displayed_model_path_from_str(model_index.as_str()) else {
                ui.set_status_text("Model is no longer available for tag drop".into());
                return false;
            };

            let allow_sidecar_writes = state.sidecar_writes_enabled;
            let prefs = state.prefs.clone();
            match persist_add_existing_tag(
                &prefs,
                &mut state.entries,
                &path,
                allow_sidecar_writes,
                tag.as_str(),
            ) {
                Ok(Some(TagDropOutcome::Added { tag, count })) if allow_sidecar_writes => {
                    ui.set_status_text(format!("Tag added: {tag} ({count})").into())
                }
                Ok(Some(TagDropOutcome::Added { tag, count })) => {
                    ui.set_status_text(format!("Demo tag added: {tag} ({count})").into())
                }
                Ok(Some(TagDropOutcome::AlreadyPresent { tag, count })) => {
                    ui.set_status_text(format!("Tag already present: {tag} ({count})").into())
                }
                Ok(None) => {
                    ui.set_status_text("Tag drop target is no longer available".into());
                    return false;
                }
                Err(err) => {
                    ui.set_status_text(format!("Could not add dropped tag: {err}").into());
                    return false;
                }
            }

            let snapshot = state.snapshot_done();
            state.reselect_path(&path);
            apply_snapshot(&ui, &snapshot);
            apply_detail(&ui, &mut state);
            apply_settings(&ui, &state);
            true
        } else {
            false
        }
    });

    let weak = ui.as_weak();
    let tag_state = state.clone();
    ui.on_remove_tag(move |index| {
        if let Some(ui) = weak.upgrade() {
            let mut state = tag_state.borrow_mut();
            let Some(path) = state.selected_model_path() else {
                ui.set_status_text("Select a model before removing tags".into());
                return;
            };

            let allow_sidecar_writes = state.sidecar_writes_enabled;
            let prefs = state.prefs.clone();
            match persist_remove_tag(
                &prefs,
                &mut state.entries,
                &path,
                allow_sidecar_writes,
                index,
            ) {
                Ok(Some(count)) if allow_sidecar_writes => {
                    ui.set_status_text(format!("Tag removed: {}", count).into())
                }
                Ok(Some(count)) => {
                    ui.set_status_text(format!("Demo tag removed: {}", count).into())
                }
                Ok(None) => ui.set_status_text("Tag is no longer available".into()),
                Err(err) => {
                    ui.set_status_text(format!("Could not remove tag: {}", err).into());
                    return;
                }
            }

            let snapshot = state.snapshot_done();
            state.reselect_path(&path);
            apply_snapshot(&ui, &snapshot);
            apply_detail(&ui, &mut state);
            apply_settings(&ui, &state);
        }
    });

    let weak = ui.as_weak();
    let print_state = state.clone();
    ui.on_adjust_printed_count(move |delta| {
        if let Some(ui) = weak.upgrade() {
            let mut state = print_state.borrow_mut();
            let Some(path) = state.selected_model_path() else {
                ui.set_status_text("Select a model before updating print count".into());
                return;
            };

            let allow_sidecar_writes = state.sidecar_writes_enabled;
            let prefs = state.prefs.clone();
            match persist_print_count_delta(
                &prefs,
                &mut state.entries,
                &path,
                allow_sidecar_writes,
                delta,
            ) {
                Ok(Some(count)) if allow_sidecar_writes => {
                    ui.set_status_text(format!("Print count saved: {}", count).into())
                }
                Ok(Some(count)) => {
                    ui.set_status_text(format!("Demo print count updated: {}", count).into())
                }
                Ok(None) => ui.set_status_text("Selected model is no longer available".into()),
                Err(err) => {
                    ui.set_status_text(format!("Could not update print count: {}", err).into());
                    return;
                }
            }

            let snapshot = state.snapshot_done();
            state.reselect_path(&path);
            apply_snapshot(&ui, &snapshot);
            apply_detail(&ui, &mut state);
            apply_settings(&ui, &state);
        }
    });

    let weak = ui.as_weak();
    let print_history_state = state.clone();
    ui.on_add_print_record(
        move |material, printer, profile, nozzle, layer_height, duration, notes| {
            if let Some(ui) = weak.upgrade() {
                let mut state = print_history_state.borrow_mut();
                let Some(path) = state.selected_model_path() else {
                    ui.set_status_text("Select a model before adding print history".into());
                    return;
                };

                let allow_sidecar_writes = state.sidecar_writes_enabled;
                let prefs = state.prefs.clone();
                match persist_add_print_record(
                    &prefs,
                    &mut state.entries,
                    &path,
                    allow_sidecar_writes,
                    material.as_str(),
                    printer.as_str(),
                    profile.as_str(),
                    nozzle.as_str(),
                    layer_height.as_str(),
                    duration.as_str(),
                    notes.as_str(),
                    &today_date_utc(),
                ) {
                    Ok(Some(count)) if allow_sidecar_writes => {
                        ui.set_status_text(format!("Print record saved: {}", count).into())
                    }
                    Ok(Some(count)) => {
                        ui.set_status_text(format!("Demo print record added: {}", count).into())
                    }
                    Ok(None) => ui.set_status_text("Selected model is no longer available".into()),
                    Err(err) => {
                        ui.set_status_text(format!("Could not add print record: {}", err).into());
                        return;
                    }
                }

                let snapshot = state.snapshot_done();
                state.reselect_path(&path);
                apply_snapshot(&ui, &snapshot);
                apply_detail(&ui, &mut state);
                apply_settings(&ui, &state);
            }
        },
    );

    let weak = ui.as_weak();
    let print_history_state = state.clone();
    ui.on_remove_print_record(move |index| {
        if let Some(ui) = weak.upgrade() {
            let mut state = print_history_state.borrow_mut();
            let Some(path) = state.selected_model_path() else {
                ui.set_status_text("Select a model before removing print history".into());
                return;
            };

            let allow_sidecar_writes = state.sidecar_writes_enabled;
            let prefs = state.prefs.clone();
            match persist_remove_print_record(
                &prefs,
                &mut state.entries,
                &path,
                allow_sidecar_writes,
                index,
            ) {
                Ok(Some(count)) if allow_sidecar_writes => {
                    ui.set_status_text(format!("Print record removed: {}", count).into())
                }
                Ok(Some(count)) => {
                    ui.set_status_text(format!("Demo print record removed: {}", count).into())
                }
                Ok(None) => ui.set_status_text("Print record is no longer available".into()),
                Err(err) => {
                    ui.set_status_text(format!("Could not remove print record: {}", err).into());
                    return;
                }
            }

            let snapshot = state.snapshot_done();
            state.reselect_path(&path);
            apply_snapshot(&ui, &snapshot);
            apply_detail(&ui, &mut state);
            apply_settings(&ui, &state);
        }
    });

    let weak = ui.as_weak();
    let slicer_state = state.clone();
    ui.on_open_in_slicer(move || {
        if let Some(ui) = weak.upgrade() {
            let state = slicer_state.borrow();
            let Some(path) = state.selected_model_path() else {
                ui.set_status_text("Select a model before opening a slicer".into());
                return;
            };
            match launch_model(&path, &state.prefs.slicer_path) {
                Ok(()) => ui.set_status_text(format!("Opening {}", path.display()).into()),
                Err(err) => ui.set_status_text(format!("Could not open slicer: {}", err).into()),
            }
        }
    });

    ui.on_window_close(move || {
        crate::macos::hide_window();
    });

    ui.on_window_minimize(move || {
        crate::macos::minimize_window();
    });

    ui.on_window_fullscreen(move || {
        crate::macos::fullscreen_window();
    });

    let weak = ui.as_weak();
    ui.on_titlebar_drag(move |_x, _y| {
        if let Some(ui) = weak.upgrade() {
            let _ = ui.window().with_winit_window(|window| window.drag_window());
        }
    });

    let weak = ui.as_weak();
    let auto_state = state.clone();
    let auto_watcher = watcher_runtime.clone();
    let auto_scan = scan_runtime.clone();
    let watch_poll_timer = Rc::new(slint::Timer::default());
    watch_poll_timer.start(
        slint::TimerMode::Repeated,
        LIBRARY_WATCH_POLL_INTERVAL,
        move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let watch_poll = auto_watcher
                .borrow_mut()
                .poll(Instant::now(), LIBRARY_WATCH_DEBOUNCE);
            if watch_poll.refresh_due {
                request_library_folder_scans(
                    &ui,
                    &auto_state,
                    &auto_scan,
                    &auto_watcher,
                    "Auto-refreshing library after file change",
                );
            }
            if let Some(error) = watch_poll.error {
                ui.set_status_text(
                    format!("File watcher error: {error}; use Refresh manually").into(),
                );
            }
            let scan_poll = auto_scan.borrow_mut().poll();
            if !scan_poll.entry_batches.is_empty() {
                apply_scan_entry_batches(
                    &ui,
                    &auto_state,
                    scan_poll.entry_batches,
                    scan_poll.progress.as_ref(),
                );
            }
            if let Some(progress) = scan_poll.progress {
                apply_scan_progress(&ui, &progress);
            }
            if let Some(result) = scan_poll.result {
                apply_scan_result(&ui, &auto_state, &auto_scan, &auto_watcher, result);
            }
        },
    );

    let weak = ui.as_weak();
    let menu_state = state.clone();
    let menu_watcher = watcher_runtime.clone();
    let menu_scan = scan_runtime.clone();
    let menu_poll_timer = Rc::new(slint::Timer::default());
    menu_poll_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(150),
        move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };

            if crate::macos::take_settings_request() {
                {
                    let mut state = menu_state.borrow_mut();
                    state.settings_open = true;
                    state.settings_tab = "general".to_string();
                }
                apply_settings(&ui, &menu_state.borrow());
                ui.set_status_text("Settings opened from the macOS menu bar".into());
            }

            if crate::macos::take_open_library_request() {
                choose_library_folder(&ui, &menu_state, &menu_scan, &menu_watcher);
            }

            if crate::macos::take_undo_request() {
                undo_library_action(&ui, &menu_state, &menu_scan);
            }
        },
    );

    crate::macos::install_app_icon();
    ui.show()?;
    crate::macos::install_app_menu();

    let _ = ui.window().with_winit_window(|window| {
        window.set_resizable(true);
        window.set_min_inner_size(Some(PhysicalSize::new(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT)));
        window.set_max_inner_size(None::<PhysicalSize<u32>>);
        let _ = window.request_inner_size(PhysicalSize::new(
            DEFAULT_WINDOW_WIDTH,
            DEFAULT_WINDOW_HEIGHT,
        ));
    });

    crate::macos::configure_native_window_chrome();

    // Re-apply after the first macOS layout pass so restored/reopened windows
    // keep the native wrapper and full-screen collection behavior.
    slint::Timer::single_shot(std::time::Duration::from_millis(50), move || {
        crate::macos::install_app_menu();
        crate::macos::configure_native_window_chrome();
        crate::macos::show_windows();
    });

    slint::run_event_loop()?;
    drop(menu_poll_timer);
    drop(watch_poll_timer);
    Ok(())
}

#[cfg(target_os = "macos")]
fn configure_slint_backend() -> Result<(), slint::PlatformError> {
    use i_slint_backend_winit::winit::platform::macos::WindowAttributesExtMacOS;

    let backend = i_slint_backend_winit::Backend::builder()
        .with_default_menu_bar(false)
        .with_window_attributes_hook(|attributes| {
            attributes
                .with_fullsize_content_view(true)
                .with_titlebar_transparent(true)
                .with_title_hidden(true)
                .with_titlebar_buttons_hidden(true)
        })
        .build()?;
    slint::platform::set_platform(Box::new(backend)).map_err(slint::PlatformError::SetPlatformError)
}

#[cfg(not(target_os = "macos"))]
fn configure_slint_backend() -> Result<(), slint::PlatformError> {
    Ok(())
}

#[derive(Debug)]
enum WatchMessage {
    Changed {
        generation: u64,
        paths: Vec<PathBuf>,
    },
    Error {
        generation: u64,
        message: String,
    },
}

#[derive(Default, Debug, PartialEq, Eq)]
struct WatchPoll {
    refresh_due: bool,
    error: Option<String>,
}

#[derive(Default, Debug)]
struct WatchDebounce {
    pending_since: Option<Instant>,
}

impl WatchDebounce {
    fn record(&mut self, now: Instant) {
        self.pending_since = Some(now);
    }

    fn consume_if_due(&mut self, now: Instant, debounce: Duration) -> bool {
        let Some(pending_since) = self.pending_since else {
            return false;
        };
        if now.duration_since(pending_since) >= debounce {
            self.pending_since = None;
            true
        } else {
            false
        }
    }
}

struct ScanResult {
    generation: u64,
    folder: PathBuf,
    entries: Vec<scanner::StlFileInfo>,
    skipped: usize,
}

struct ScanProgress {
    generation: u64,
    folder: PathBuf,
    found: usize,
    scanned: usize,
    total: usize,
    skipped: usize,
    current: String,
}

struct ScanEntryBatch {
    generation: u64,
    folder: PathBuf,
    entries: Vec<scanner::StlFileInfo>,
}

#[derive(Default)]
struct ScanPoll {
    progress: Option<ScanProgress>,
    entry_batches: Vec<ScanEntryBatch>,
    result: Option<ScanResult>,
}

enum ScanMessage {
    Progress(ScanProgress),
    EntryBatch(ScanEntryBatch),
    Result(ScanResult),
}

struct LibraryScanRuntime {
    next_generation: u64,
    active_generation: Option<u64>,
    active_folder: Option<PathBuf>,
    tx: mpsc::Sender<ScanMessage>,
    rx: mpsc::Receiver<ScanMessage>,
}

impl LibraryScanRuntime {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            next_generation: 0,
            active_generation: None,
            active_folder: None,
            tx,
            rx,
        }
    }

    fn request_scan(&mut self, folder: &Path) -> ScanRequest {
        if self.active_generation.is_some() {
            return ScanRequest::AlreadyRunning;
        }

        self.next_generation += 1;
        let generation = self.next_generation;
        self.active_generation = Some(generation);
        self.active_folder = Some(folder.to_path_buf());
        let folder = folder.to_path_buf();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let total = count_supported_model_files(&folder);
            let (entries, skipped) = scan_folder_entries(&folder, generation, total, &tx);
            let _ = tx.send(ScanMessage::Result(ScanResult {
                generation,
                folder,
                entries,
                skipped,
            }));
        });
        ScanRequest::Started
    }

    fn poll(&mut self) -> ScanPoll {
        let mut latest = ScanPoll::default();
        while let Ok(message) = self.rx.try_recv() {
            match message {
                ScanMessage::Progress(progress)
                    if Some(progress.generation) == self.active_generation
                        && crate::view_model::scan_message_folder_matches_active(
                            self.active_folder.as_deref(),
                            progress.folder.as_path(),
                        ) =>
                {
                    latest.progress = Some(progress);
                }
                ScanMessage::EntryBatch(batch)
                    if Some(batch.generation) == self.active_generation
                        && crate::view_model::scan_message_folder_matches_active(
                            self.active_folder.as_deref(),
                            batch.folder.as_path(),
                        ) =>
                {
                    latest.entry_batches.push(batch);
                }
                ScanMessage::Result(result)
                    if Some(result.generation) == self.active_generation
                        && crate::view_model::scan_message_folder_matches_active(
                            self.active_folder.as_deref(),
                            result.folder.as_path(),
                        ) =>
                {
                    self.active_generation = None;
                    self.active_folder = None;
                    latest.result = Some(result);
                }
                _ => {}
            }
        }
        latest
    }

    fn invalidate(&mut self) {
        self.next_generation += 1;
        self.active_generation = None;
        self.active_folder = None;
        while self.rx.try_recv().is_ok() {}
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ScanRequest {
    Started,
    AlreadyRunning,
}

struct LibraryWatcherRuntime {
    watcher: Option<RecommendedWatcher>,
    watched_folder: Option<PathBuf>,
    generation: u64,
    tx: mpsc::Sender<WatchMessage>,
    rx: mpsc::Receiver<WatchMessage>,
    debounce: WatchDebounce,
}

impl LibraryWatcherRuntime {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            watcher: None,
            watched_folder: None,
            generation: 0,
            tx,
            rx,
            debounce: WatchDebounce::default(),
        }
    }

    fn watch_folder(&mut self, folder: &Path) -> Result<(), String> {
        if self.watched_folder.as_deref() == Some(folder) && self.watcher.is_some() {
            return Ok(());
        }

        self.watcher = None;
        self.watched_folder = None;
        self.reset_pending_messages();
        self.generation += 1;
        let generation = self.generation;

        let tx = self.tx.clone();
        let mut watcher = notify::recommended_watcher(
            move |result: notify::Result<notify::Event>| match result {
                Ok(event)
                    if event
                        .paths
                        .iter()
                        .any(|path| is_refresh_relevant_path(path)) =>
                {
                    let _ = tx.send(WatchMessage::Changed {
                        generation,
                        paths: event.paths,
                    });
                }
                Ok(_) => {}
                Err(err) => {
                    let _ = tx.send(WatchMessage::Error {
                        generation,
                        message: err.to_string(),
                    });
                }
            },
        )
        .map_err(|err| format!("Could not create file watcher: {err}"))?;

        watcher
            .watch(folder, RecursiveMode::Recursive)
            .map_err(|err| format!("Could not watch {}: {err}", folder.display()))?;

        self.watcher = Some(watcher);
        self.watched_folder = Some(folder.to_path_buf());
        Ok(())
    }

    fn poll(&mut self, now: Instant, debounce: Duration) -> WatchPoll {
        let mut saw_relevant = false;
        let mut poll = WatchPoll::default();
        while let Ok(message) = self.rx.try_recv() {
            match message {
                WatchMessage::Changed { generation, paths }
                    if generation == self.generation
                        && paths.iter().any(|path| is_refresh_relevant_path(path)) =>
                {
                    saw_relevant = true;
                }
                WatchMessage::Changed { .. } => {}
                WatchMessage::Error {
                    generation,
                    message,
                } if generation == self.generation => {
                    poll.error = Some(message);
                    self.watcher = None;
                    self.watched_folder = None;
                    self.debounce = WatchDebounce::default();
                }
                WatchMessage::Error { .. } => {}
            }
        }
        if saw_relevant {
            self.debounce.record(now);
        }
        poll.refresh_due = self.debounce.consume_if_due(now, debounce);
        poll
    }

    fn reset_pending_messages(&mut self) {
        while self.rx.try_recv().is_ok() {}
        self.debounce = WatchDebounce::default();
    }
}

fn is_refresh_relevant_path(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if file_name.ends_with(".modelrack.json") {
        return true;
    }

    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "stl" | "3mf" | "obj" | "step" | "stp" | "scad"
            )
        })
        .unwrap_or(false)
}

fn set_watch_status(
    ui: &ModelRackWindow,
    watcher_runtime: &Rc<RefCell<LibraryWatcherRuntime>>,
    folder: &Path,
) {
    let language = ui.get_settings_language_key().to_string();
    match watcher_runtime.borrow_mut().watch_folder(folder) {
        Ok(()) => ui.set_status_text(
            localized(
                "Watching library for changes",
                "라이브러리 변경 감시 중",
                "ライブラリの変更を監視中",
                &language,
            )
            .into(),
        ),
        Err(err) => ui.set_status_text(
            match language_key(&language) {
                "ko" => format!("{err}; 새로고침을 수동으로 사용하세요"),
                "ja" => format!("{err}; 手動で更新してください"),
                _ => format!("{err}; use Refresh manually"),
            }
            .into(),
        ),
    }
}

fn choose_library_folder(
    ui: &ModelRackWindow,
    state: &Rc<RefCell<ShellState>>,
    scan_runtime: &Rc<RefCell<LibraryScanRuntime>>,
    watcher_runtime: &Rc<RefCell<LibraryWatcherRuntime>>,
) {
    ui.set_status_text("Choose a library folder".into());
    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
        if scan_runtime.borrow().active_generation.is_some() {
            let mut state_mut = state.borrow_mut();
            ShellState::merge_library_folder_into_prefs(&mut state_mut.prefs, &folder);
            state_mut.queue_pending_scan_roots([folder.clone()]);
            ui.set_status_text(format!("Queued {} for scan", folder.display()).into());
            save_prefs_status(ui, &state_mut);
        } else {
            scan_runtime.borrow_mut().invalidate();
            start_folder_scan(ui, state, scan_runtime, &folder, "Scanning selected folder");
            set_watch_status(ui, watcher_runtime, &folder);
        }
    }
}

fn start_library_scan_queue(
    ui: &ModelRackWindow,
    state: &Rc<RefCell<ShellState>>,
    scan_runtime: &Rc<RefCell<LibraryScanRuntime>>,
    watcher_runtime: &Rc<RefCell<LibraryWatcherRuntime>>,
    folders: Vec<PathBuf>,
    status: &str,
) -> Option<PathBuf> {
    let mut folders = crate::view_model::dedupe_paths_keep_order(folders);
    folders.retain(|folder| folder.is_dir());
    let first = folders.first().cloned()?;
    {
        let mut state = state.borrow_mut();
        state.replace_pending_scan_queue(folders.into_iter().skip(1));
    }
    scan_runtime.borrow_mut().invalidate();
    start_folder_scan(ui, state, scan_runtime, &first, status);
    set_watch_status(ui, watcher_runtime, &first);
    Some(first)
}

fn start_folder_scan(
    ui: &ModelRackWindow,
    state: &Rc<RefCell<ShellState>>,
    scan_runtime: &Rc<RefCell<LibraryScanRuntime>>,
    folder: &Path,
    status: &str,
) {
    ui.set_scan_progress_percent(0);
    let snapshot = state.borrow_mut().begin_folder_scan(folder, status);
    apply_snapshot(ui, &snapshot);
    apply_detail_rc(ui, state);
    apply_settings(ui, &state.borrow());
    save_prefs_status(ui, &state.borrow());
    match scan_runtime.borrow_mut().request_scan(folder) {
        ScanRequest::Started => {}
        ScanRequest::AlreadyRunning => {
            ui.set_status_text("Scan already running for this library".into())
        }
    }
}

fn request_library_folder_scans(
    ui: &ModelRackWindow,
    state: &Rc<RefCell<ShellState>>,
    scan_runtime: &Rc<RefCell<LibraryScanRuntime>>,
    watcher_runtime: &Rc<RefCell<LibraryWatcherRuntime>>,
    status: &str,
) -> Option<PathBuf> {
    let folders = state.borrow().restored_library_scan_queue();
    let Some(first) = folders.first().cloned() else {
        ui.set_status_text("Choose a real library folder before refreshing".into());
        return None;
    };

    let active_folder = scan_runtime.borrow().active_folder.clone();
    if scan_runtime.borrow().active_generation.is_some() {
        let queued: Vec<PathBuf> = folders
            .into_iter()
            .filter(|folder| {
                active_folder.as_ref().is_none_or(|active| {
                    !crate::view_model::paths_equal_or_same_target(active, folder)
                })
            })
            .collect();
        if queued.is_empty() {
            ui.set_status_text("Refresh already running".into());
        } else {
            state.borrow_mut().queue_pending_scan_roots(queued);
            ui.set_status_text("Refresh queued after current scan finishes".into());
        }
        return Some(first);
    }

    start_library_scan_queue(ui, state, scan_runtime, watcher_runtime, folders, status)
}

fn apply_scan_progress(ui: &ModelRackWindow, progress: &ScanProgress) {
    let language = ui.get_settings_language_key().to_string();
    let percent = if progress.total == 0 {
        0
    } else if progress.scanned < progress.total {
        ((progress.scanned * 100) / progress.total).min(98)
    } else {
        ((progress.scanned.min(progress.total) * 100) / progress.total).min(99)
    };
    ui.set_scan_progress_percent(percent as i32);
    ui.set_status_text(
        match language_key(&language) {
            "ko" => format!(
                "스캔 중 {} · 발견 {} · {} / {} · {}개 건너뜀",
                progress.current,
                progress.found,
                progress.scanned,
                progress.total,
                progress.skipped
            ),
            "ja" => format!(
                "スキャン中 {} · 検出 {} · {} / {} · {} 件スキップ",
                progress.current,
                progress.found,
                progress.scanned,
                progress.total,
                progress.skipped
            ),
            _ => format!(
                "Scanning {} · found {} · {} / {} · {} skipped",
                progress.current,
                progress.found,
                progress.scanned,
                progress.total,
                progress.skipped
            ),
        }
        .into(),
    );
}

fn apply_scan_entry_batches(
    ui: &ModelRackWindow,
    state: &Rc<RefCell<ShellState>>,
    batches: Vec<ScanEntryBatch>,
    progress: Option<&ScanProgress>,
) {
    let snapshot = state
        .borrow_mut()
        .apply_scan_entry_batches(batches, progress);
    if let Some(snapshot) = snapshot {
        apply_snapshot(ui, &snapshot);
        apply_detail_rc(ui, state);
        apply_settings(ui, &state.borrow());
    }
}

fn rescan_sidebar_folder(
    ui: &ModelRackWindow,
    state: &Rc<RefCell<ShellState>>,
    scan_runtime: &Rc<RefCell<LibraryScanRuntime>>,
    folder: &Path,
    label: &str,
) {
    let status = format!("Rescanning {label}");
    let Some(scan_root) = state.borrow().scan_root_for_folder(folder) else {
        ui.set_status_text(format!("Folder no longer exists: {}", folder.display()).into());
        return;
    };

    if scan_runtime.borrow().active_generation.is_some() {
        state
            .borrow_mut()
            .queue_pending_scan_roots([scan_root.clone()]);
        ui.set_status_text(format!("Queued {label} for rescan").into());
        return;
    }

    scan_runtime.borrow_mut().invalidate();
    start_folder_scan(ui, state, scan_runtime, &scan_root, &status);
}

fn remove_sidebar_folder_from_library(
    ui: &ModelRackWindow,
    state: &Rc<RefCell<ShellState>>,
    folder: &Path,
    label: &str,
) {
    let snapshot = {
        let mut state = state.borrow_mut();
        state.undo_stack.push(LibraryUndo {
            folder: folder.to_path_buf(),
            label: label.to_string(),
        });
        let is_library_root = state
            .prefs
            .library_folders
            .iter()
            .any(|root| root.as_path() == folder);
        if is_library_root {
            state.remove_library_root(folder);
            state.selected_index = None;
            if matches!(&state.filter, LibraryFilter::Folder(active) if active.starts_with(folder))
            {
                state.filter = LibraryFilter::All;
            }
            if state.prefs.library_folders.is_empty() {
                state.clear_library_state();
                state.snapshot_idle()
            } else {
                state.snapshot_done()
            }
        } else {
            state.add_excluded_folder(folder);
            state
                .entries
                .retain(|entry| !entry.path.starts_with(folder));
            state.selected_index = None;
            if matches!(&state.filter, LibraryFilter::Folder(active) if active.starts_with(folder))
            {
                state.filter = LibraryFilter::All;
            }
            state.snapshot_done()
        }
    };
    apply_snapshot(ui, &snapshot);
    apply_detail_rc(ui, state);
    apply_settings(ui, &state.borrow());
    save_prefs_status(ui, &state.borrow());
    ui.set_status_text(format!("Removed {label} from library").into());
}

fn move_sidebar_folder_to_trash(
    ui: &ModelRackWindow,
    state: &Rc<RefCell<ShellState>>,
    scan_runtime: &Rc<RefCell<LibraryScanRuntime>>,
    folder: &Path,
    label: &str,
) {
    if !folder.exists() {
        ui.set_status_text(format!("Folder no longer exists: {}", folder.display()).into());
        return;
    }

    if !confirm_move_folder_to_trash(label, folder) {
        ui.set_status_text("Move to Trash cancelled".into());
        return;
    }

    let active_root = state.borrow().active_real_folder();
    match move_path_to_trash(folder) {
        Ok(()) => {
            ui.set_status_text(format!("Deleted folder {label}").into());
            {
                let mut state = state.borrow_mut();
                state.remove_excluded_folder_tree(folder);
                state
                    .undo_stack
                    .retain(|undo| !undo.folder.starts_with(folder));
                state
                    .entries
                    .retain(|entry| !entry.path.starts_with(folder));
                state.selected_index = None;
                if matches!(&state.filter, LibraryFilter::Folder(active) if active.starts_with(folder))
                {
                    state.filter = LibraryFilter::All;
                }
                state
                    .prefs
                    .library_folders
                    .retain(|root| !root.starts_with(folder));
                if state
                    .prefs
                    .last_folder
                    .as_ref()
                    .is_some_and(|p| p.starts_with(folder))
                {
                    state.prefs.last_folder = state.prefs.library_folders.last().cloned();
                }
            }
            save_prefs_status(ui, &state.borrow());
            if state.borrow().prefs.library_folders.is_empty() {
                scan_runtime.borrow_mut().invalidate();
                clear_deleted_library(ui, state);
                ui.set_status_text(format!("Deleted folder {label}").into());
            } else {
                let snapshot = state.borrow_mut().snapshot_done();
                apply_snapshot(ui, &snapshot);
                apply_detail_rc(ui, state);
                apply_settings(ui, &state.borrow());
                save_prefs_status(ui, &state.borrow());
                if let Some(root) = active_root {
                    if root.exists() && !root.starts_with(folder) {
                        match scan_runtime.borrow_mut().request_scan(&root) {
                            ScanRequest::Started => {
                                ui.set_scan_progress_percent(0);
                                ui.set_status_text(
                                    format!("Deleted folder {label}; rescanning library").into(),
                                );
                            }
                            ScanRequest::AlreadyRunning => {
                                ui.set_status_text("Library rescan already running".into());
                            }
                        }
                    }
                }
            }
        }
        Err(err) => ui.set_status_text(format!("Could not move folder to Trash: {err}").into()),
    }
}

fn confirm_move_folder_to_trash(label: &str, folder: &Path) -> bool {
    match rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title("Delete folder?")
        .set_description(format!(
            "Move “{}” to the Trash and remove it from the library?\n\n{}",
            label,
            folder.display()
        ))
        .set_buttons(rfd::MessageButtons::OkCancelCustom(
            "Delete Folder".to_string(),
            "Cancel".to_string(),
        ))
        .show()
    {
        rfd::MessageDialogResult::Ok => true,
        rfd::MessageDialogResult::Custom(value) => value == "Delete Folder",
        _ => false,
    }
}

fn move_path_to_trash(path: &Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let script = r#"
on run argv
    tell application "Finder"
        delete POSIX file (item 1 of argv)
    end tell
end run
"#;
        let status = Command::new("osascript")
            .arg("-e")
            .arg(script)
            .arg(path)
            .status()?;
        if status.success() {
            return Ok(());
        }
        return Err(io::Error::other(format!(
            "osascript exited with status {status}"
        )));
    }

    #[cfg(target_os = "windows")]
    {
        let status = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(
                "Add-Type -AssemblyName Microsoft.VisualBasic; \
                 [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteDirectory($args[0], \
                 'OnlyErrorDialogs', 'SendToRecycleBin')",
            )
            .arg(path)
            .status()?;
        if status.success() {
            return Ok(());
        }
        return Err(io::Error::other(format!(
            "powershell exited with status {status}"
        )));
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        for program in ["gio", "trash-put"] {
            let mut command = Command::new(program);
            if program == "gio" {
                command.arg("trash");
            }
            let status = command.arg(path).status();
            match status {
                Ok(status) if status.success() => return Ok(()),
                Ok(_) | Err(_) => {}
            }
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no system trash command found",
        ))
    }
}

fn undo_library_action(
    ui: &ModelRackWindow,
    state: &Rc<RefCell<ShellState>>,
    scan_runtime: &Rc<RefCell<LibraryScanRuntime>>,
) {
    let Some(undo) = state.borrow_mut().undo_stack.pop() else {
        ui.set_status_text("Nothing to undo".into());
        return;
    };

    let active_root = {
        let mut state = state.borrow_mut();
        state.remove_excluded_folder(&undo.folder);
        state.active_real_folder()
    };
    save_prefs_status(ui, &state.borrow());

    if let Some(root) = active_root.filter(|root| undo.folder.starts_with(root)) {
        match scan_runtime.borrow_mut().request_scan(&root) {
            ScanRequest::Started => {
                ui.set_scan_progress_percent(0);
                ui.set_status_text(format!("Restoring {} to library", undo.label).into());
            }
            ScanRequest::AlreadyRunning => {
                ui.set_status_text("Restore queued after current scan finishes".into());
            }
        }
    } else if undo.folder.is_dir() {
        start_folder_scan(
            ui,
            state,
            scan_runtime,
            &undo.folder,
            &format!("Restoring {}", undo.label),
        );
    } else {
        let snapshot = state.borrow_mut().snapshot_done();
        apply_snapshot(ui, &snapshot);
        apply_detail_rc(ui, state);
        apply_settings(ui, &state.borrow());
        ui.set_status_text(
            format!(
                "Undo restored library setting, but folder is missing: {}",
                undo.folder.display()
            )
            .into(),
        );
    }
}

fn clear_deleted_library(ui: &ModelRackWindow, state: &Rc<RefCell<ShellState>>) {
    let snapshot = {
        let mut state = state.borrow_mut();
        state.clear_library_state();
        state.snapshot_idle()
    };
    apply_snapshot(ui, &snapshot);
    apply_detail_rc(ui, state);
    apply_settings(ui, &state.borrow());
    save_prefs_status(ui, &state.borrow());
}

fn apply_scan_result(
    ui: &ModelRackWindow,
    state: &Rc<RefCell<ShellState>>,
    scan_runtime: &Rc<RefCell<LibraryScanRuntime>>,
    watcher_runtime: &Rc<RefCell<LibraryWatcherRuntime>>,
    result: ScanResult,
) {
    ui.set_scan_progress_percent(-1);
    let should_apply = state.borrow().should_apply_completed_scan(&result.folder);
    let (snapshot, next_scan) = {
        let mut state = state.borrow_mut();
        let snapshot = if should_apply {
            Some(state.apply_scan_result(result))
        } else {
            None
        };
        let next = state.pop_next_pending_scan_root();
        (snapshot, next)
    };
    if let Some(snapshot) = snapshot {
        apply_snapshot(ui, &snapshot);
        apply_detail_rc(ui, state);
        apply_settings(ui, &state.borrow());
        save_prefs_status(ui, &state.borrow());
    }
    if let Some(folder) = next_scan {
        start_folder_scan(
            ui,
            state,
            scan_runtime,
            &folder,
            "Scanning queued library folder",
        );
        set_watch_status(ui, watcher_runtime, &folder);
    }
}

fn apply_detail_rc(ui: &ModelRackWindow, state: &Rc<RefCell<ShellState>>) {
    let mut state = state.borrow_mut();
    apply_detail(ui, &mut state);
}

fn apply_detail(ui: &ModelRackWindow, state: &mut ShellState) {
    apply_detail_with_quality(ui, state, DetailPreviewQuality::High);
}

fn apply_detail_with_quality(
    ui: &ModelRackWindow,
    state: &mut ShellState,
    quality: DetailPreviewQuality,
) {
    ui.set_selected_card_index(state.selected_index.map(|i| i as i32).unwrap_or(-1));
    if let Some(idx) = state.selected_index {
        if let Some(entry) = state.displayed.get(idx).cloned() {
            let language = state.prefs.language.clone();
            ui.set_has_selection(true);
            ui.set_selected_thumb_key(crate::view_model::thumbnail_key(&entry.filename).into());
            let yaw = state.preview_orbit_yaw;
            let pitch = state.preview_orbit_pitch;
            let preview = state.selected_preview(&entry);
            let (thumb_image, thumb_ready) = preview
                .as_ref()
                .map(|preview| {
                    render_detail_preview_image(&entry, &preview.mesh, yaw, pitch, quality)
                })
                .unwrap_or_else(|| load_thumbnail_image(entry.thumbnail_path.as_deref()));
            ui.set_selected_thumb_image(thumb_image);
            ui.set_selected_thumb_ready(thumb_ready);
            ui.set_detail_name(entry.filename.clone().into());
            ui.set_detail_path(detail_parent_label(&entry, state).into());
            ui.set_detail_format(
                match entry.stl_type {
                    scanner::StlType::Binary => "Binary STL",
                    scanner::StlType::Ascii => "ASCII STL",
                    scanner::StlType::ThreeMf => "3MF",
                    scanner::StlType::Obj => "OBJ",
                    scanner::StlType::Step => "STEP",
                    scanner::StlType::Scad => "SCAD",
                    scanner::StlType::LargeStl => "Large STL",
                    scanner::StlType::Unknown => "Unknown",
                }
                .into(),
            );
            let plate_count = preview.as_ref().map_or(0, |preview| preview.plate_count);
            ui.set_detail_has_plates(plate_count > 1);
            ui.set_detail_plate_summary(
                preview
                    .as_ref()
                    .filter(|preview| preview.plate_count > 1)
                    .map(|preview| match language_key(&language) {
                        "ko" => {
                            format!(
                                "{}개 플레이트 · {} 표시 중",
                                preview.plate_count, preview.selected_label
                            )
                        }
                        "ja" => {
                            format!(
                                "{} プレート · {} を表示中",
                                preview.plate_count, preview.selected_label
                            )
                        }
                        _ => {
                            format!(
                                "{} plates · showing {}",
                                preview.plate_count, preview.selected_label
                            )
                        }
                    })
                    .unwrap_or_default()
                    .into(),
            );
            ui.set_detail_plate_selected_label(
                preview
                    .as_ref()
                    .filter(|preview| preview.plate_count > 1)
                    .map(|preview| preview.selected_label.clone())
                    .unwrap_or_default()
                    .into(),
            );
            ui.set_detail_plate_tabs(slint::ModelRc::new(slint::VecModel::from(
                preview
                    .as_ref()
                    .map(|preview| {
                        preview
                            .tab_rows
                            .iter()
                            .map(|tab| PlateTab {
                                label: tab.label.clone().into(),
                                index: tab.index,
                                selected: tab.selected,
                            })
                            .collect::<Vec<PlateTab>>()
                    })
                    .unwrap_or_default(),
            )));
            ui.set_detail_tris(
                preview
                    .as_ref()
                    .map(|preview| preview.triangle_count)
                    .or(entry.triangle_count)
                    .map(|t| {
                        if t >= 1_000_000 {
                            format!("{:.2}M", t as f64 / 1_000_000.0)
                        } else if t >= 1_000 {
                            format!("{:.1}K", t as f64 / 1_000.0)
                        } else {
                            format!("{}", t)
                        }
                    })
                    .unwrap_or_else(|| "—".to_string())
                    .into(),
            );
            ui.set_detail_dims(
                preview
                    .as_ref()
                    .and_then(|preview| preview.dimensions)
                    .or(entry.dimensions)
                    .map(|[x, y, z]| format!("{:.1} × {:.1} × {:.1} mm", x, y, z))
                    .unwrap_or_else(|| "—".to_string())
                    .into(),
            );
            ui.set_detail_volume(
                preview
                    .as_ref()
                    .and_then(|preview| preview.volume_cm3)
                    .map(|volume| format!("{:.1} cm³", volume))
                    .or_else(|| {
                        preview
                            .as_ref()
                            .and_then(|preview| preview.dimensions)
                            .or(entry.dimensions)
                            .map(|[x, y, z]| format!("~{:.1} cm³", x * y * z / 1000.0 * 0.12))
                    })
                    .unwrap_or_else(|| "—".to_string())
                    .into(),
            );
            ui.set_detail_filesize(format_size(entry.size).into());
            ui.set_detail_hash(
                entry
                    .hash
                    .iter()
                    .take(8)
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>()
                    .into(),
            );
            ui.set_detail_modified(
                entry
                    .modified
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| {
                        let secs = d.as_secs();
                        let days = secs / 86400;
                        if days < 1 {
                            "today".to_string()
                        } else if days < 30 {
                            format!("{}d ago", days)
                        } else {
                            format!("{}mo ago", days / 30)
                        }
                    })
                    .unwrap_or_else(|| "—".to_string())
                    .into(),
            );
            ui.set_detail_added(
                entry
                    .meta
                    .as_ref()
                    .and_then(|meta| meta.added.clone())
                    .unwrap_or_else(|| "—".to_string())
                    .into(),
            );
            ui.set_detail_author(
                entry
                    .meta
                    .as_ref()
                    .and_then(|m| {
                        if m.author.is_empty() {
                            None
                        } else {
                            Some(m.author.clone())
                        }
                    })
                    .unwrap_or_else(|| "You".to_string())
                    .into(),
            );
            ui.set_detail_tags_label(
                entry
                    .meta
                    .as_ref()
                    .map(|m| {
                        if m.tags.is_empty() {
                            "No tags".to_string()
                        } else {
                            m.tags.join(" · ")
                        }
                    })
                    .unwrap_or_else(|| "No tags".to_string())
                    .into(),
            );
            ui.set_detail_tags_input(
                entry
                    .meta
                    .as_ref()
                    .map(|m| m.tags.join(", "))
                    .unwrap_or_default()
                    .into(),
            );
            let tag_chips = entry
                .meta
                .as_ref()
                .map(|meta| meta.tags.iter().map(tag_chip).collect::<Vec<TagChip>>())
                .unwrap_or_default();
            ui.set_detail_tag_chips(slint::ModelRc::new(slint::VecModel::from(tag_chips)));
            ui.set_detail_notes(
                entry
                    .meta
                    .as_ref()
                    .and_then(|m| (!m.notes.is_empty()).then(|| m.notes.clone()))
                    .unwrap_or_else(|| "Add notes...".to_string())
                    .into(),
            );
            ui.set_detail_printed_count(entry.meta.as_ref().map_or(0, |m| m.printed as i32));
            let print_history = entry
                .meta
                .as_ref()
                .map(|meta| {
                    meta.print_history
                        .iter()
                        .rev()
                        .map(print_history_row)
                        .collect::<Vec<PrintHistoryRow>>()
                })
                .unwrap_or_default();
            ui.set_detail_print_history(slint::ModelRc::new(slint::VecModel::from(print_history)));
            ui.set_detail_fav(entry.meta.as_ref().map(|m| m.favorite).unwrap_or(false));

            // Mesh health (deterministic from triangle count)
            let manifold = entry.stl_type != scanner::StlType::Unknown;
            let selected_triangle_count = preview
                .as_ref()
                .map(|preview| preview.triangle_count)
                .or(entry.triangle_count)
                .unwrap_or(0);
            let selected_dimensions = preview
                .as_ref()
                .and_then(|preview| preview.dimensions)
                .or(entry.dimensions);
            let watertight = manifold && selected_triangle_count > 100;
            let normals = manifold;
            ui.set_detail_manifold(manifold);
            ui.set_detail_watertight(watertight);
            ui.set_detail_normals(normals);
            ui.set_detail_health_score(
                if manifold { 50 } else { 20 }
                    + if watertight { 25 } else { 0 }
                    + if normals { 15 } else { 0 },
            );

            // Print estimate
            if let Some([x, y, z]) = selected_dimensions {
                let catalog = load_printer_profiles();
                let profile = state.selected_estimate_profile(&catalog);
                let selected_volume_cm3 = preview.as_ref().and_then(|preview| preview.volume_cm3);
                let estimate =
                    estimate_print_for_dimensions([x, y, z], selected_volume_cm3, &profile);
                ui.set_detail_estimate_time(estimate.time_label.into());
                ui.set_detail_estimate_grams(estimate.grams_label.into());
                ui.set_detail_estimate_layers(estimate.layers_label.into());
                ui.set_detail_bed_fit(estimate.bed_fit);
                ui.set_detail_estimate_printer_label(profile.label.clone().into());
                ui.set_detail_estimate_printer_detail(printer_detail(&profile).into());
                let printer_choices =
                    estimate_printer_choices(&state.prefs, &catalog, &profile.key);
                ui.set_detail_estimate_has_printer_choices(printer_choices.len() > 1);
                ui.set_detail_estimate_printers(slint::ModelRc::new(slint::VecModel::from(
                    printer_choices,
                )));
            } else {
                clear_print_estimate(ui);
            }
        } else {
            ui.set_has_selection(false);
            ui.set_selected_thumb_key("rack".into());
            ui.set_selected_thumb_image(slint::Image::default());
            ui.set_selected_thumb_ready(false);
            clear_detail_plate_tabs(ui);
            clear_detail_tag_chips(ui);
            clear_detail_print_history(ui);
            clear_print_estimate(ui);
        }
    } else {
        ui.set_has_selection(false);
        ui.set_selected_thumb_key("rack".into());
        ui.set_selected_thumb_image(slint::Image::default());
        ui.set_selected_thumb_ready(false);
        clear_detail_plate_tabs(ui);
        clear_detail_tag_chips(ui);
        clear_detail_print_history(ui);
        clear_print_estimate(ui);
    }
}

fn clear_print_estimate(ui: &ModelRackWindow) {
    ui.set_detail_estimate_time("".into());
    ui.set_detail_estimate_grams("".into());
    ui.set_detail_estimate_layers("".into());
    ui.set_detail_estimate_printer_label("".into());
    ui.set_detail_estimate_printer_detail("".into());
    ui.set_detail_estimate_has_printer_choices(false);
    ui.set_detail_estimate_printers(slint::ModelRc::new(slint::VecModel::from(Vec::<
        PrinterProfileChoice,
    >::new())));
    ui.set_detail_bed_fit(false);
}

fn clear_detail_plate_tabs(ui: &ModelRackWindow) {
    ui.set_detail_has_plates(false);
    ui.set_detail_plate_summary("".into());
    ui.set_detail_plate_selected_label("".into());
    ui.set_detail_plate_tabs(slint::ModelRc::new(slint::VecModel::from(
        Vec::<PlateTab>::new(),
    )));
}

fn clear_detail_tag_chips(ui: &ModelRackWindow) {
    ui.set_detail_tag_chips(slint::ModelRc::new(slint::VecModel::from(
        Vec::<TagChip>::new(),
    )));
}

fn clear_detail_print_history(ui: &ModelRackWindow) {
    ui.set_detail_print_history(slint::ModelRc::new(slint::VecModel::from(Vec::<
        PrintHistoryRow,
    >::new())));
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1_024 {
        format!("{:.1} KB", bytes as f64 / 1_024.0)
    } else {
        format!("{} B", bytes)
    }
}

fn today_date_utc() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| (duration.as_secs() / 86_400) as i64)
        .unwrap_or(0);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
}

fn app_prefs_path() -> PathBuf {
    if let Some(path) = std::env::var_os("MODELRACK_PREFS_PATH") {
        return PathBuf::from(path);
    }

    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        return home
            .join("Library")
            .join("Application Support")
            .join("ModelRack")
            .join("prefs.json");
    }

    #[cfg(target_os = "windows")]
    {
        let root = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        return root.join("ModelRack").join("prefs.json");
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let root = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."));
        root.join("modelrack").join("prefs.json")
    }
}

fn load_app_prefs_from_path(path: &Path) -> io::Result<AppPrefs> {
    match fs::read_to_string(path) {
        Ok(data) => {
            let mut prefs: AppPrefs = serde_json::from_str(&data)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            if prefs.library_folders.is_empty() {
                if let Some(folder) = prefs.last_folder.clone() {
                    prefs.library_folders.push(folder);
                }
            }
            prefs.normalize_path_fields();
            Ok(prefs)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(AppPrefs::default()),
        Err(err) => Err(err),
    }
}

fn save_app_prefs(prefs: &AppPrefs) -> io::Result<()> {
    save_app_prefs_to_path(&app_prefs_path(), prefs)
}

fn save_app_prefs_to_path(path: &Path, prefs: &AppPrefs) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(prefs)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, json)?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

fn save_prefs_status(ui: &ModelRackWindow, state: &ShellState) {
    if let Err(err) = save_app_prefs(&state.prefs) {
        ui.set_status_text(format!("Could not save settings: {}", err).into());
    }
}

#[derive(Debug, PartialEq, Eq)]
struct LaunchCommand {
    program: String,
    args: Vec<String>,
    wait_for_exit: bool,
}

fn launcher_command(model_path: &Path, slicer_path: &str) -> LaunchCommand {
    let model = model_path.display().to_string();
    let slicer = slicer_path.trim();

    if slicer.is_empty() {
        return default_open_command(model);
    }

    #[cfg(target_os = "macos")]
    if Path::new(slicer)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
    {
        return LaunchCommand {
            program: "open".to_string(),
            args: vec!["-a".to_string(), slicer.to_string(), model],
            wait_for_exit: true,
        };
    }

    LaunchCommand {
        program: slicer.to_string(),
        args: vec![model],
        wait_for_exit: false,
    }
}

#[cfg(target_os = "macos")]
fn default_open_command(model: String) -> LaunchCommand {
    LaunchCommand {
        program: "open".to_string(),
        args: vec![model],
        wait_for_exit: true,
    }
}

#[cfg(target_os = "windows")]
fn default_open_command(model: String) -> LaunchCommand {
    LaunchCommand {
        program: "cmd".to_string(),
        args: vec!["/C".to_string(), "start".to_string(), "".to_string(), model],
        wait_for_exit: true,
    }
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn default_open_command(model: String) -> LaunchCommand {
    LaunchCommand {
        program: "xdg-open".to_string(),
        args: vec![model],
        wait_for_exit: true,
    }
}

fn launch_model(model_path: &Path, slicer_path: &str) -> io::Result<()> {
    validate_launch_request(model_path, slicer_path)?;
    let command = launcher_command(model_path, slicer_path);
    run_launch_command(command)?;
    Ok(())
}

fn validate_launch_request(model_path: &Path, slicer_path: &str) -> io::Result<()> {
    if !model_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("model does not exist: {}", model_path.display()),
        ));
    }

    let slicer = slicer_path.trim();
    if !slicer.is_empty() {
        let slicer = Path::new(slicer);
        if !slicer.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("slicer does not exist: {}", slicer.display()),
            ));
        }
    }

    Ok(())
}

fn run_launch_command(command: LaunchCommand) -> io::Result<()> {
    let mut child = Command::new(&command.program).args(&command.args).spawn()?;

    if command.wait_for_exit {
        let status = child.wait()?;
        if !status.success() {
            return Err(io::Error::other(format!(
                "{} exited with status {}",
                command.program, status
            )));
        }
    } else {
        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            if let Some(status) = child.try_wait()? {
                if !status.success() {
                    return Err(io::Error::other(format!(
                        "{} exited with status {}",
                        command.program, status
                    )));
                }
                return Ok(());
            }
        }

        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }

    Ok(())
}

fn folder_path_from_filter_key(key: &str) -> Option<PathBuf> {
    key.strip_prefix("folder:")
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
}

fn reveal_path_in_file_manager(path: &Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg("-R").arg(path).status()?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .status()?;
        return Ok(());
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let target = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        Command::new("xdg-open").arg(target).status()?;
        Ok(())
    }
}

fn copy_text_to_clipboard(text: &str) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let mut child = Command::new("pbcopy").stdin(Stdio::piped()).spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }
        let status = child.wait()?;
        if !status.success() {
            return Err(io::Error::other(format!(
                "pbcopy exited with status {}",
                status
            )));
        }
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        let mut child = Command::new("clip").stdin(Stdio::piped()).spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }
        let status = child.wait()?;
        if !status.success() {
            return Err(io::Error::other(format!(
                "clip exited with status {}",
                status
            )));
        }
        return Ok(());
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let mut child = Command::new("wl-copy").stdin(Stdio::piped()).spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }
        let status = child.wait()?;
        if !status.success() {
            return Err(io::Error::other(format!(
                "wl-copy exited with status {}",
                status
            )));
        }
        Ok(())
    }
}

fn try_sync_entry_to_db(prefs: &AppPrefs, entries: &[scanner::StlFileInfo], path: &Path) {
    let Some(root) = crate::db::library_root_for_model_path(path, &prefs.library_folders) else {
        return;
    };
    let Some(entry) = entries.iter().find(|entry| entry.path == path) else {
        return;
    };
    if let Err(err) = crate::db::upsert_entries_for_library(&root, std::slice::from_ref(entry)) {
        eprintln!(
            "Warning: library DB sync failed for {}: {:#}",
            path.display(),
            err
        );
    }
}

fn persist_favorite_toggle(
    prefs: &AppPrefs,
    entries: &mut [scanner::StlFileInfo],
    path: &Path,
    allow_sidecar_writes: bool,
) -> anyhow::Result<Option<bool>> {
    let Some(entry) = entries.iter_mut().find(|entry| entry.path == path) else {
        return Ok(None);
    };

    let mut meta = entry.meta.clone().unwrap_or_default();
    meta.favorite = !meta.favorite;
    if allow_sidecar_writes {
        if !path.exists() {
            anyhow::bail!("model does not exist: {}", path.display());
        }
        scanner::write_sidecar(path, &meta)?;
    }
    entry.meta = Some(meta.clone());
    if allow_sidecar_writes {
        try_sync_entry_to_db(prefs, entries, path);
    }
    Ok(Some(meta.favorite))
}

fn parse_tag_input(input: &str) -> Vec<String> {
    let mut tags = Vec::new();
    for tag in input
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
    {
        if !tags.iter().any(|existing| existing == tag) {
            tags.push(tag.to_string());
        }
    }
    tags
}

fn persist_metadata_fields(
    prefs: &AppPrefs,
    entries: &mut [scanner::StlFileInfo],
    path: &Path,
    allow_sidecar_writes: bool,
    tags: &str,
    author: &str,
    notes: &str,
) -> anyhow::Result<Option<scanner::SidecarMeta>> {
    update_model_meta(prefs, entries, path, allow_sidecar_writes, |meta| {
        meta.tags = parse_tag_input(tags);
        meta.author = author.trim().to_string();
        meta.notes = notes.to_string();
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TagDropOutcome {
    Added { tag: String, count: usize },
    AlreadyPresent { tag: String, count: usize },
}

fn persist_add_existing_tag(
    prefs: &AppPrefs,
    entries: &mut [scanner::StlFileInfo],
    path: &Path,
    allow_sidecar_writes: bool,
    tag: &str,
) -> anyhow::Result<Option<TagDropOutcome>> {
    let tag = tag.trim();
    if tag.is_empty() {
        return Ok(None);
    }

    let Some(entry) = entries.iter_mut().find(|entry| entry.path == path) else {
        return Ok(None);
    };

    let current_count = entry.meta.as_ref().map_or(0, |meta| meta.tags.len());
    if entry
        .meta
        .as_ref()
        .is_some_and(|meta| meta.tags.iter().any(|existing| existing == tag))
    {
        return Ok(Some(TagDropOutcome::AlreadyPresent {
            tag: tag.to_string(),
            count: current_count,
        }));
    }

    if allow_sidecar_writes && !path.exists() {
        anyhow::bail!("model does not exist: {}", path.display());
    }

    let mut meta = entry.meta.clone().unwrap_or_default();
    meta.tags.push(tag.to_string());
    if allow_sidecar_writes {
        scanner::write_sidecar(path, &meta)?;
    }
    let count = meta.tags.len();
    entry.meta = Some(meta);
    if allow_sidecar_writes {
        try_sync_entry_to_db(prefs, entries, path);
    }
    Ok(Some(TagDropOutcome::Added {
        tag: tag.to_string(),
        count,
    }))
}

fn persist_add_tags(
    prefs: &AppPrefs,
    entries: &mut [scanner::StlFileInfo],
    path: &Path,
    allow_sidecar_writes: bool,
    input: &str,
) -> anyhow::Result<Option<usize>> {
    let additions = parse_tag_input(input);
    if additions.is_empty() {
        return Ok(entries
            .iter()
            .find(|entry| entry.path == path)
            .map(|entry| entry.meta.as_ref().map_or(0, |meta| meta.tags.len())));
    }

    let updated = update_model_meta(prefs, entries, path, allow_sidecar_writes, |meta| {
        for tag in additions {
            if !meta.tags.iter().any(|existing| existing == &tag) {
                meta.tags.push(tag);
            }
        }
    })?;
    Ok(updated.map(|meta| meta.tags.len()))
}

fn persist_remove_tag(
    prefs: &AppPrefs,
    entries: &mut [scanner::StlFileInfo],
    path: &Path,
    allow_sidecar_writes: bool,
    tag_index: i32,
) -> anyhow::Result<Option<usize>> {
    let Some(entry) = entries.iter().find(|entry| entry.path == path) else {
        return Ok(None);
    };
    let Some(tag_index) = usize::try_from(tag_index).ok().filter(|index| {
        entry
            .meta
            .as_ref()
            .is_some_and(|meta| *index < meta.tags.len())
    }) else {
        return Ok(None);
    };

    let updated = update_model_meta(prefs, entries, path, allow_sidecar_writes, |meta| {
        meta.tags.remove(tag_index);
    })?;
    Ok(updated.map(|meta| meta.tags.len()))
}

fn persist_print_count_delta(
    prefs: &AppPrefs,
    entries: &mut [scanner::StlFileInfo],
    path: &Path,
    allow_sidecar_writes: bool,
    delta: i32,
) -> anyhow::Result<Option<u32>> {
    let updated = update_model_meta(prefs, entries, path, allow_sidecar_writes, |meta| {
        let next = meta.printed as i64 + delta as i64;
        meta.printed = next.max(0).min(u32::MAX as i64) as u32;
    })?;
    Ok(updated.map(|meta| meta.printed))
}

fn persist_add_print_record(
    prefs: &AppPrefs,
    entries: &mut [scanner::StlFileInfo],
    path: &Path,
    allow_sidecar_writes: bool,
    material: &str,
    printer: &str,
    profile: &str,
    nozzle: &str,
    layer_height: &str,
    duration: &str,
    notes: &str,
    date: &str,
) -> anyhow::Result<Option<u32>> {
    let updated = update_model_meta(prefs, entries, path, allow_sidecar_writes, |meta| {
        meta.print_history.push(scanner::PrintRecord {
            date: date.to_string(),
            material: material.trim().to_string(),
            printer: printer.trim().to_string(),
            profile: profile.trim().to_string(),
            nozzle: nozzle.trim().to_string(),
            layer_height: layer_height.trim().to_string(),
            duration: duration.trim().to_string(),
            success: true,
            notes: notes.trim().to_string(),
        });
        meta.printed = meta.printed.saturating_add(1);
    })?;
    Ok(updated.map(|meta| meta.printed))
}

fn persist_remove_print_record(
    prefs: &AppPrefs,
    entries: &mut [scanner::StlFileInfo],
    path: &Path,
    allow_sidecar_writes: bool,
    display_index: i32,
) -> anyhow::Result<Option<u32>> {
    let Some(entry) = entries.iter().find(|entry| entry.path == path) else {
        return Ok(None);
    };
    let Some(history_index) =
        history_storage_index(entry.meta.as_ref(), display_index).filter(|_| display_index >= 0)
    else {
        return Ok(None);
    };

    let updated = update_model_meta(prefs, entries, path, allow_sidecar_writes, |meta| {
        meta.print_history.remove(history_index);
        meta.printed = meta.printed.saturating_sub(1);
    })?;
    Ok(updated.map(|meta| meta.printed))
}

fn history_storage_index(meta: Option<&scanner::SidecarMeta>, display_index: i32) -> Option<usize> {
    let len = meta?.print_history.len();
    let display_index = usize::try_from(display_index).ok()?;
    if display_index >= len {
        None
    } else {
        Some(len - 1 - display_index)
    }
}

fn update_model_meta(
    prefs: &AppPrefs,
    entries: &mut [scanner::StlFileInfo],
    path: &Path,
    allow_sidecar_writes: bool,
    update: impl FnOnce(&mut scanner::SidecarMeta),
) -> anyhow::Result<Option<scanner::SidecarMeta>> {
    let Some(entry) = entries.iter_mut().find(|entry| entry.path == path) else {
        return Ok(None);
    };

    let mut meta = entry.meta.clone().unwrap_or_default();
    update(&mut meta);
    if allow_sidecar_writes {
        if !path.exists() {
            anyhow::bail!("model does not exist: {}", path.display());
        }
        scanner::write_sidecar(path, &meta)?;
    }
    entry.meta = Some(meta.clone());
    if allow_sidecar_writes {
        try_sync_entry_to_db(prefs, entries, path);
    }
    Ok(Some(meta))
}

fn persist_model_rename(
    entries: &mut [scanner::StlFileInfo],
    path: &Path,
    allow_file_writes: bool,
    requested_name: &str,
) -> anyhow::Result<Option<PathBuf>> {
    let Some(entry) = entries.iter_mut().find(|entry| entry.path == path) else {
        return Ok(None);
    };
    let new_file_name = normalized_model_file_name(requested_name, &entry.path)?;
    let parent = entry
        .path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("model path has no parent folder"))?;
    let new_path = parent.join(new_file_name);

    if new_path == entry.path {
        return Ok(Some(entry.path.clone()));
    }

    if allow_file_writes {
        if !entry.path.exists() {
            anyhow::bail!("model does not exist: {}", entry.path.display());
        }
        if new_path.exists() {
            anyhow::bail!("target already exists: {}", new_path.display());
        }
        let old_sidecar = scanner::sidecar_path(&entry.path);
        let new_sidecar = scanner::sidecar_path(&new_path);
        if old_sidecar.exists() && new_sidecar.exists() {
            anyhow::bail!("metadata sidecar already exists: {}", new_sidecar.display());
        }

        fs::rename(&entry.path, &new_path).map_err(|err| {
            anyhow::anyhow!(
                "failed to rename {} to {}: {}",
                entry.path.display(),
                new_path.display(),
                err
            )
        })?;
        if old_sidecar.exists() {
            fs::rename(&old_sidecar, &new_sidecar).map_err(|err| {
                anyhow::anyhow!(
                    "renamed model, but failed to rename sidecar {} to {}: {}",
                    old_sidecar.display(),
                    new_sidecar.display(),
                    err
                )
            })?;
        }
    }

    entry.path = new_path.clone();
    entry.filename = new_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    Ok(Some(new_path))
}

fn normalized_model_file_name(
    requested_name: &str,
    original_path: &Path,
) -> anyhow::Result<String> {
    let trimmed = requested_name.trim();
    if trimmed.is_empty() {
        anyhow::bail!("filename cannot be empty");
    }
    if trimmed == "." || trimmed == ".." {
        anyhow::bail!("filename cannot be {}", trimmed);
    }
    if trimmed
        .chars()
        .any(|ch| matches!(ch, '/' | '\\' | ':' | '\0'))
    {
        anyhow::bail!("filename cannot contain path separators");
    }
    if Path::new(trimmed)
        .file_name()
        .and_then(|name| name.to_str())
        != Some(trimmed)
    {
        anyhow::bail!("filename must stay in the same folder");
    }

    let mut file_name = trimmed.to_string();
    if Path::new(&file_name).extension().is_none() {
        if let Some(ext) = original_path.extension().and_then(|ext| ext.to_str()) {
            file_name.push('.');
            file_name.push_str(ext);
        }
    }
    let ext = Path::new(&file_name)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    if !scanner::is_supported_model_ext(&ext) {
        anyhow::bail!("unsupported model extension: {}", ext);
    }
    Ok(file_name)
}

fn is_excluded_path(path: &Path, excluded_folders: &[PathBuf]) -> bool {
    excluded_folders
        .iter()
        .any(|excluded| path.starts_with(excluded))
}

fn filter_excluded_entries(
    entries: Vec<scanner::StlFileInfo>,
    excluded_folders: &[PathBuf],
) -> Vec<scanner::StlFileInfo> {
    if excluded_folders.is_empty() {
        return entries;
    }

    entries
        .into_iter()
        .filter(|entry| !is_excluded_path(&entry.path, excluded_folders))
        .collect()
}

fn dedupe_entries_by_path(entries: &mut Vec<scanner::StlFileInfo>) {
    let mut seen = HashSet::new();
    entries.retain(|entry| seen.insert(entry.path.clone()));
}

#[derive(Clone)]
struct LibraryUndo {
    folder: PathBuf,
    label: String,
}

#[derive(Clone, Copy)]
enum DetailPreviewQuality {
    High,
    Interactive,
}

#[derive(Default)]
struct OrbitAccumulator {
    delta_x: f32,
    delta_y: f32,
}

impl OrbitAccumulator {
    fn push(&mut self, delta_x: f32, delta_y: f32) {
        self.delta_x += delta_x;
        self.delta_y += delta_y;
    }

    fn take(&mut self) -> Option<(f32, f32)> {
        let delta_x = self.delta_x;
        let delta_y = self.delta_y;
        self.delta_x = 0.0;
        self.delta_y = 0.0;
        ((delta_x.abs() + delta_y.abs()) > 0.001).then_some((delta_x, delta_y))
    }
}

struct ShellState {
    entries: Vec<scanner::StlFileInfo>,
    displayed: Vec<scanner::StlFileInfo>,
    current_folder: Option<PathBuf>,
    prefs: AppPrefs,
    /// Scan roots waiting for the current scan to finish, used by startup restore,
    /// refresh-all, and folders added while a scan is already running.
    pending_scan_queue: VecDeque<PathBuf>,
    undo_stack: Vec<LibraryUndo>,
    search_query: String,
    filter: LibraryFilter,
    sort_by: SortBy,
    sort_ascending: bool,
    skipped: usize,
    settings_open: bool,
    settings_tab: String,
    selected_index: Option<usize>,
    preview_orbit_yaw: f32,
    preview_orbit_pitch: f32,
    preview_mesh: Option<(PathBuf, scanner::MeshData)>,
    preview_plates: Option<(PathBuf, Vec<scanner::ThreeMfPlate>)>,
    preview_plate_index: Option<usize>,
    settings_printer_maker: String,
    settings_printer_model: String,
    /// Pending nozzle for the maker/model picker. Empty until the user
    /// selects a nozzle; cleared after a successful add.
    settings_printer_nozzle: String,
    estimate_printer_key: String,
    sidecar_writes_enabled: bool,
    streaming_scan_generation: Option<u64>,
}

struct PreviewSelection {
    mesh: scanner::MeshData,
    plate_count: usize,
    selected_label: String,
    tab_rows: Vec<PreviewPlateTab>,
    triangle_count: usize,
    dimensions: Option<[f32; 3]>,
    volume_cm3: Option<f32>,
}

struct PreviewPlateTab {
    label: String,
    index: i32,
    selected: bool,
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq)]
struct PrinterProfile {
    key: String,
    label: String,
    #[serde(default)]
    printer_label: String,
    #[serde(default)]
    process_label: String,
    #[serde(default)]
    nozzle_diameter_mm: f32,
    build_volume: [f32; 3],
    layer_height_mm: f32,
    grams_per_minute: f32,
}

#[derive(Debug, serde::Deserialize)]
struct PrinterProfileCatalog {
    #[serde(default, rename = "version")]
    _version: u32,
    #[serde(default)]
    default_profile_key: String,
    #[serde(default)]
    profiles: Vec<PrinterProfile>,
}

struct PrintEstimate {
    time_label: String,
    grams_label: String,
    layers_label: String,
    bed_fit: bool,
}

impl Default for ShellState {
    fn default() -> Self {
        Self::with_prefs(AppPrefs::default())
    }
}

impl ShellState {
    fn load() -> Self {
        Self::load_from_path(&app_prefs_path())
    }

    fn load_from_path(path: &Path) -> Self {
        let prefs = load_app_prefs_from_path(path).unwrap_or_default();
        Self::from_loaded_prefs(prefs)
    }

    fn from_loaded_prefs(prefs: AppPrefs) -> Self {
        Self::with_prefs(prefs)
    }

    fn with_prefs(mut prefs: AppPrefs) -> Self {
        prefs.normalize_path_fields();
        let entries = demo_entries();
        let catalog = load_printer_profiles();
        let mut prefs = prefs;
        let sort_by = sort_by_from_key(&prefs.sort_by);
        prefs.sort_by = sort_key(sort_by).to_string();
        prefs.accent_color = accent_key(&prefs.accent_color).to_string();
        let sort_ascending = prefs.sort_ascending;
        let estimate_printer_key = default_printer_key_for_prefs(&prefs, &catalog);
        let default_profile = default_printer_profile(&prefs, &catalog);
        Self {
            entries,
            displayed: Vec::new(),
            current_folder: None,
            prefs,
            pending_scan_queue: VecDeque::new(),
            undo_stack: Vec::new(),
            search_query: String::new(),
            filter: LibraryFilter::All,
            sort_by,
            sort_ascending,
            skipped: 0,
            settings_open: false,
            settings_tab: "general".to_string(),
            selected_index: Some(0),
            preview_orbit_yaw: -0.62,
            preview_orbit_pitch: -0.48,
            preview_mesh: None,
            preview_plates: None,
            preview_plate_index: Some(0),
            settings_printer_maker: printer_maker_label(&default_profile),
            settings_printer_model: printer_model_label(&default_profile),
            settings_printer_nozzle: String::new(),
            estimate_printer_key,
            sidecar_writes_enabled: false,
            streaming_scan_generation: None,
        }
    }
}

struct DemoModel {
    name: &'static str,
    folder: &'static str,
    size: u64,
    tris: Option<usize>,
    dims: Option<[f32; 3]>,
    stl_type: scanner::StlType,
    tags: &'static [&'static str],
    printed: u32,
    favorite: bool,
    author: &'static str,
}

fn demo_entries() -> Vec<scanner::StlFileInfo> {
    let root = PathBuf::from(DEMO_ROOT);
    demo_models()
        .into_iter()
        .enumerate()
        .map(|(index, model)| demo_entry(&root, index, model))
        .collect()
}

fn demo_entry(root: &Path, index: usize, model: DemoModel) -> scanner::StlFileInfo {
    let path = root.join(model.folder).join(model.name);
    let mut entry = scanner::StlFileInfo {
        path,
        filename: model.name.to_string(),
        size: model.size,
        hash: demo_hash(index),
        stl_type: model.stl_type,
        triangle_count: model.tris,
        dimensions: model.dims,
        three_mf_plate_count: None,
        modified: demo_modified(index),
        meta: Some(scanner::SidecarMeta {
            tags: model.tags.iter().map(|tag| (*tag).to_string()).collect(),
            printed: model.printed,
            print_history: demo_print_history(model.printed),
            favorite: model.favorite,
            author: model.author.to_string(),
            notes: if index == 0 {
                "Rackmount bracket validated for the current homelab layout.".to_string()
            } else {
                String::new()
            },
            ..scanner::SidecarMeta::default()
        }),
        thumbnail_path: None,
    };
    entry.thumbnail_path = crate::thumbnail_cache::ensure_thumbnail(&entry, None).ok();
    entry
}

fn demo_print_history(count: u32) -> Vec<scanner::PrintRecord> {
    let visible_count = count.min(3);
    (0..visible_count)
        .map(|index| scanner::PrintRecord {
            date: format!(
                "2026-05-{:02}",
                8_u32.saturating_sub(visible_count - 1 - index)
            ),
            material: if index % 2 == 0 { "PLA" } else { "PETG" }.to_string(),
            printer: if index % 2 == 0 {
                "Bambu P1S"
            } else {
                "Prusa MK4"
            }
            .to_string(),
            profile: if index % 2 == 0 {
                "0.20 Standard"
            } else {
                "0.15 Quality"
            }
            .to_string(),
            nozzle: "0.4 mm".to_string(),
            layer_height: if index % 2 == 0 { "0.20 mm" } else { "0.15 mm" }.to_string(),
            duration: if index % 2 == 0 { "2h 10m" } else { "3h 05m" }.to_string(),
            success: true,
            notes: if index == 0 {
                "0.20mm profile".to_string()
            } else {
                String::new()
            },
        })
        .collect()
}

fn demo_hash(index: usize) -> [u8; 32] {
    let mut hash = [(index as u8).wrapping_add(11); 32];
    if index == 2 {
        hash = [7; 32];
    }
    if index == 3 {
        hash = [7; 32];
    }
    hash
}

fn demo_modified(index: usize) -> Option<std::time::SystemTime> {
    let days = if index < 12 {
        index as u64
    } else {
        240 + index as u64
    };
    Some(std::time::SystemTime::now() - std::time::Duration::from_secs(days * 86_400))
}

fn demo_models() -> Vec<DemoModel> {
    use scanner::StlType::{Ascii, Binary, Unknown};

    vec![
        DemoModel {
            name: "raspberry_pi_5_poe_rackmount_v2_final.stl",
            folder: "homelab/rackmount",
            size: 2_840_000,
            tris: Some(48_230),
            dims: Some([120.4, 88.0, 25.5]),
            stl_type: Binary,
            tags: &["rackmount", "raspberry-pi", "poe", "homelab"],
            printed: 3,
            favorite: true,
            author: "makerworld",
        },
        DemoModel {
            name: "pi5_heatsink_clip.stl",
            folder: "homelab/rackmount",
            size: 142_000,
            tris: Some(1_820),
            dims: Some([42.0, 32.0, 12.0]),
            stl_type: Binary,
            tags: &["raspberry-pi", "cooling"],
            printed: 2,
            favorite: false,
            author: "printables",
        },
        DemoModel {
            name: "1U_blank_panel_19in.stl",
            folder: "homelab/rackmount",
            size: 380_400,
            tris: Some(240),
            dims: Some([482.6, 44.4, 2.0]),
            stl_type: Binary,
            tags: &["rackmount", "19inch"],
            printed: 1,
            favorite: false,
            author: "thingiverse",
        },
        DemoModel {
            name: "gmktec_nucbox_mount.stl",
            folder: "homelab/mini-pc",
            size: 1_120_000,
            tris: Some(18_920),
            dims: Some([128.0, 128.0, 18.0]),
            stl_type: Binary,
            tags: &["mini-pc", "gmktec", "mount"],
            printed: 1,
            favorite: false,
            author: "鈴木一郎",
        },
        DemoModel {
            name: "switch_8port_bracket.stl",
            folder: "homelab/network",
            size: 920_000,
            tris: Some(14_820),
            dims: Some([220.0, 70.0, 32.0]),
            stl_type: Binary,
            tags: &["network", "switch", "bracket", "queued"],
            printed: 0,
            favorite: false,
            author: "김지훈",
        },
        DemoModel {
            name: "ssd_2_5in_caddy_x4.stl",
            folder: "homelab/storage",
            size: 1_840_000,
            tris: Some(28_100),
            dims: Some([110.0, 105.0, 50.0]),
            stl_type: Binary,
            tags: &["storage", "ssd", "cage"],
            printed: 2,
            favorite: true,
            author: "github/cnc",
        },
        DemoModel {
            name: "spool_holder_universal.stl",
            folder: "printer/upgrades",
            size: 2_240_000,
            tris: Some(32_400),
            dims: Some([180.0, 95.0, 110.0]),
            stl_type: Binary,
            tags: &["printer", "spool", "functional"],
            printed: 5,
            favorite: true,
            author: "You",
        },
        DemoModel {
            name: "bambu_p1s_chamber_thermometer.stl",
            folder: "printer/upgrades",
            size: 480_000,
            tris: Some(6_200),
            dims: Some([60.0, 40.0, 18.0]),
            stl_type: Binary,
            tags: &["bambulab", "printer", "upgrade"],
            printed: 1,
            favorite: false,
            author: "makerworld",
        },
        DemoModel {
            name: "cable_chain_15x10.stl",
            folder: "printer/upgrades",
            size: 320_000,
            tris: Some(4_400),
            dims: Some([220.0, 15.0, 10.0]),
            stl_type: Binary,
            tags: &["cable", "functional"],
            printed: 4,
            favorite: false,
            author: "You",
        },
        DemoModel {
            name: "snapmaker_a350_drag_chain_link.stl",
            folder: "printer/upgrades",
            size: 220_000,
            tris: Some(1_840),
            dims: Some([38.0, 22.0, 10.0]),
            stl_type: Binary,
            tags: &["snapmaker", "cable"],
            printed: 8,
            favorite: false,
            author: "printables",
        },
        DemoModel {
            name: "라즈베리파이_5_케이스_v3.stl",
            folder: "한국어_프로젝트",
            size: 1_640_000,
            tris: Some(22_300),
            dims: Some([95.0, 65.0, 28.0]),
            stl_type: Binary,
            tags: &["raspberry-pi", "case"],
            printed: 2,
            favorite: true,
            author: "makerworld",
        },
        DemoModel {
            name: "책상정리_케이블_홀더.stl",
            folder: "한국어_프로젝트",
            size: 280_000,
            tris: Some(3_120),
            dims: Some([60.0, 40.0, 25.0]),
            stl_type: Binary,
            tags: &["desk", "cable"],
            printed: 6,
            favorite: false,
            author: "You",
        },
        DemoModel {
            name: "키캡_oem_r4_blank.stl",
            folder: "한국어_프로젝트/keycaps",
            size: 88_000,
            tris: Some(920),
            dims: Some([18.0, 18.0, 11.0]),
            stl_type: Binary,
            tags: &["keycap", "keyboard"],
            printed: 12,
            favorite: true,
            author: "You",
        },
        DemoModel {
            name: "low_poly_fox.stl",
            folder: "decorative",
            size: 4_200_000,
            tris: Some(78_400),
            dims: Some([85.0, 110.0, 60.0]),
            stl_type: Binary,
            tags: &["decorative", "lowpoly"],
            printed: 1,
            favorite: false,
            author: "thingiverse",
        },
        DemoModel {
            name: "voronoi_planter_120mm.stl",
            folder: "decorative",
            size: 6_800_000,
            tris: Some(124_000),
            dims: Some([120.0, 120.0, 95.0]),
            stl_type: Binary,
            tags: &["decorative", "planter", "voronoi", "ready-to-print"],
            printed: 0,
            favorite: true,
            author: "makerworld",
        },
        DemoModel {
            name: "geometric_vase_twisted.stl",
            folder: "decorative",
            size: 3_400_000,
            tris: Some(56_000),
            dims: Some([80.0, 80.0, 180.0]),
            stl_type: Binary,
            tags: &["decorative", "vase"],
            printed: 2,
            favorite: false,
            author: "You",
        },
        DemoModel {
            name: "articulated_dragon_v4.stl",
            folder: "decorative/articulated",
            size: 18_400_000,
            tris: Some(320_000),
            dims: Some([240.0, 80.0, 65.0]),
            stl_type: Binary,
            tags: &["decorative", "articulated", "ready-to-print"],
            printed: 0,
            favorite: false,
            author: "printables",
        },
        DemoModel {
            name: "benchy_3dbenchy.stl",
            folder: "test_prints",
            size: 1_540_000,
            tris: Some(22_500),
            dims: Some([60.0, 31.0, 48.0]),
            stl_type: Binary,
            tags: &["test", "benchmark", "favorite"],
            printed: 4,
            favorite: true,
            author: "You",
        },
        DemoModel {
            name: "calibration_cube_20mm.stl",
            folder: "test_prints",
            size: 12_400,
            tris: Some(12),
            dims: Some([20.0, 20.0, 20.0]),
            stl_type: Binary,
            tags: &["test", "calibration"],
            printed: 14,
            favorite: false,
            author: "You",
        },
        DemoModel {
            name: "all_in_one_test_v2.stl",
            folder: "test_prints",
            size: 880_000,
            tris: Some(14_200),
            dims: Some([60.0, 60.0, 30.0]),
            stl_type: Binary,
            tags: &["test", "calibration"],
            printed: 3,
            favorite: false,
            author: "You",
        },
        DemoModel {
            name: "broken_export_garbage.stl",
            folder: "downloads",
            size: 184_000,
            tris: None,
            dims: None,
            stl_type: Unknown,
            tags: &[],
            printed: 0,
            favorite: false,
            author: "unknown",
        },
        DemoModel {
            name: "weird_ascii_export.stl",
            folder: "downloads",
            size: 4_200_000,
            tris: Some(8_400),
            dims: Some([42.0, 42.0, 42.0]),
            stl_type: Ascii,
            tags: &["ready-to-print"],
            printed: 0,
            favorite: false,
            author: "You",
        },
        DemoModel {
            name: "hdd_3_5in_vibration_dampener.stl",
            folder: "homelab/storage",
            size: 240_000,
            tris: Some(2_800),
            dims: Some([102.0, 14.0, 26.0]),
            stl_type: Binary,
            tags: &["storage", "hdd", "damper"],
            printed: 4,
            favorite: false,
            author: "You",
        },
        DemoModel {
            name: "ups_battery_holder_18650_x8.stl",
            folder: "homelab/power",
            size: 1_280_000,
            tris: Some(18_400),
            dims: Some([180.0, 78.0, 22.0]),
            stl_type: Binary,
            tags: &["power", "battery", "18650"],
            printed: 1,
            favorite: false,
            author: "makerworld",
        },
        DemoModel {
            name: "fan_grill_120mm_honeycomb.stl",
            folder: "homelab/cooling",
            size: 480_000,
            tris: Some(12_200),
            dims: Some([120.0, 120.0, 4.0]),
            stl_type: Binary,
            tags: &["fan", "grill", "cooling"],
            printed: 6,
            favorite: true,
            author: "You",
        },
        DemoModel {
            name: "noctua_fan_shroud_140mm.stl",
            folder: "homelab/cooling",
            size: 620_000,
            tris: Some(9_800),
            dims: Some([140.0, 140.0, 30.0]),
            stl_type: Binary,
            tags: &["fan", "shroud", "cooling"],
            printed: 0,
            favorite: false,
            author: "printables",
        },
        DemoModel {
            name: "vesa_75_to_100_adapter.stl",
            folder: "mounts",
            size: 320_000,
            tris: Some(4_800),
            dims: Some([120.0, 120.0, 6.0]),
            stl_type: Binary,
            tags: &["vesa", "mount", "adapter"],
            printed: 0,
            favorite: false,
            author: "You",
        },
        DemoModel {
            name: "monitor_arm_cable_clip.stl",
            folder: "mounts",
            size: 88_000,
            tris: Some(1_200),
            dims: Some([42.0, 28.0, 18.0]),
            stl_type: Binary,
            tags: &["cable", "clip", "desk"],
            printed: 0,
            favorite: false,
            author: "You",
        },
        DemoModel {
            name: "wall_anchor_drywall_kit.stl",
            folder: "mounts",
            size: 64_000,
            tris: Some(600),
            dims: Some([25.0, 12.0, 12.0]),
            stl_type: Binary,
            tags: &["wall", "anchor"],
            printed: 16,
            favorite: false,
            author: "printables",
        },
        DemoModel {
            name: "stringing_test_tower.stl",
            folder: "test_prints",
            size: 180_000,
            tris: Some(1_800),
            dims: Some([60.0, 30.0, 50.0]),
            stl_type: Binary,
            tags: &["test", "calibration", "stringing"],
            printed: 2,
            favorite: false,
            author: "You",
        },
        DemoModel {
            name: "overhang_test_45_60_75.stl",
            folder: "test_prints",
            size: 220_000,
            tris: Some(2_400),
            dims: Some([80.0, 30.0, 40.0]),
            stl_type: Binary,
            tags: &["test", "calibration", "overhang"],
            printed: 1,
            favorite: false,
            author: "You",
        },
        DemoModel {
            name: "temp_tower_pla_180_220.stl",
            folder: "test_prints",
            size: 280_000,
            tris: Some(3_600),
            dims: Some([50.0, 30.0, 100.0]),
            stl_type: Binary,
            tags: &["test", "calibration", "temp"],
            printed: 2,
            favorite: false,
            author: "You",
        },
        DemoModel {
            name: "celtic_knot_coaster_set.stl",
            folder: "decorative",
            size: 920_000,
            tris: Some(14_800),
            dims: Some([95.0, 95.0, 6.0]),
            stl_type: Binary,
            tags: &["decorative", "coaster"],
            printed: 4,
            favorite: false,
            author: "makerworld",
        },
        DemoModel {
            name: "hex_organizer_drawer_module.stl",
            folder: "organization",
            size: 720_000,
            tris: Some(8_400),
            dims: Some([120.0, 120.0, 25.0]),
            stl_type: Binary,
            tags: &["organizer", "modular", "gridfinity"],
            printed: 9,
            favorite: false,
            author: "You",
        },
        DemoModel {
            name: "gridfinity_baseplate_4x4.stl",
            folder: "organization/gridfinity",
            size: 1_800_000,
            tris: Some(24_000),
            dims: Some([168.0, 168.0, 5.0]),
            stl_type: Binary,
            tags: &["organizer", "gridfinity", "modular"],
            printed: 12,
            favorite: true,
            author: "printables",
        },
        DemoModel {
            name: "gridfinity_bin_2x2x4_solid.stl",
            folder: "organization/gridfinity",
            size: 480_000,
            tris: Some(8_200),
            dims: Some([84.0, 84.0, 32.0]),
            stl_type: Binary,
            tags: &["organizer", "gridfinity"],
            printed: 24,
            favorite: false,
            author: "You",
        },
    ]
}

impl ShellState {
    fn snapshot_done(&mut self) -> AppViewSnapshot {
        let status = ScanStatus::Done {
            found: self.entries.len(),
            skipped: self.skipped,
        };
        let query = DisplayQuery {
            search_query: &self.search_query,
            library_filter: &self.filter,
            sort_by: self.sort_by,
            sort_ascending: self.sort_ascending,
            preserve_order: false,
        };
        self.displayed = crate::view_model::filtered_sorted_entries(&self.entries, query);
        AppViewSnapshot::from_parts_with_displayed_slice(
            &self.entries,
            &self.displayed,
            self.library_roots_for_snapshot(),
            &status,
            &self.prefs,
            query,
        )
    }

    fn snapshot_idle(&mut self) -> AppViewSnapshot {
        let query = DisplayQuery {
            search_query: &self.search_query,
            library_filter: &self.filter,
            sort_by: self.sort_by,
            sort_ascending: self.sort_ascending,
            preserve_order: false,
        };
        self.displayed = crate::view_model::filtered_sorted_entries(&self.entries, query);
        AppViewSnapshot::from_parts_with_displayed_slice(
            &self.entries,
            &self.displayed,
            self.library_roots_for_snapshot(),
            &ScanStatus::Idle,
            &self.prefs,
            query,
        )
    }

    fn begin_folder_scan(&mut self, folder: &Path, current: &str) -> AppViewSnapshot {
        self.remove_excluded_folder(folder);
        Self::merge_library_folder_into_prefs(&mut self.prefs, folder);
        self.entries
            .retain(|entry| !crate::view_model::entry_under_library_root(&entry.path, folder));
        self.retain_entries_under_library_roots();
        self.displayed.clear();
        self.current_folder = Some(folder.to_path_buf());
        self.prefs.last_folder = Some(folder.to_path_buf());
        self.skipped = 0;
        self.sidecar_writes_enabled = true;
        self.selected_index = None;
        self.streaming_scan_generation = None;
        let query = DisplayQuery {
            search_query: &self.search_query,
            library_filter: &self.filter,
            sort_by: self.sort_by,
            sort_ascending: self.sort_ascending,
            preserve_order: false,
        };
        AppViewSnapshot::from_parts(
            &self.entries,
            self.library_roots_for_snapshot(),
            &ScanStatus::Scanning {
                found: 0,
                scanned: 0,
                skipped: 0,
                current: current.to_string(),
            },
            &self.prefs,
            query,
        )
    }

    fn apply_scan_result(&mut self, result: ScanResult) -> AppViewSnapshot {
        self.apply_scan_parts(result.folder, result.entries, result.skipped)
    }

    fn apply_scan_entry_batches(
        &mut self,
        batches: Vec<ScanEntryBatch>,
        progress: Option<&ScanProgress>,
    ) -> Option<AppViewSnapshot> {
        let entries_len_before = self.entries.len();
        let mut updated = false;
        for batch in batches {
            if !crate::view_model::scan_message_folder_matches_active(
                self.current_folder.as_deref(),
                batch.folder.as_path(),
            ) {
                continue;
            }
            if self.streaming_scan_generation != Some(batch.generation) {
                self.entries.retain(|entry| {
                    !crate::view_model::entry_under_library_root(&entry.path, &batch.folder)
                });
                self.displayed.clear();
                self.skipped = 0;
                self.selected_index = None;
                self.streaming_scan_generation = Some(batch.generation);
            }

            let batch_had_entries = !batch.entries.is_empty();
            let entries = filter_excluded_entries(batch.entries, &self.prefs.excluded_folders);
            if !entries.is_empty() {
                if let Err(err) = crate::db::upsert_entries_for_library(&batch.folder, &entries) {
                    eprintln!("Warning: library DB upsert failed: {err:#}");
                }
            }
            updated |= batch_had_entries;
            self.entries.extend(entries);
        }

        if !updated {
            return None;
        }

        let len_after_append = self.entries.len();
        dedupe_entries_by_path(&mut self.entries);
        let dedupe_changed = self.entries.len() != len_after_append;
        let len_before_roots = self.entries.len();
        self.retain_entries_under_library_roots();
        let roots_changed = self.entries.len() != len_before_roots;

        if self.selected_index.is_none() && !self.entries.is_empty() {
            self.selected_index = Some(0);
        }

        let status = progress.map_or_else(
            || ScanStatus::Scanning {
                found: self.entries.len(),
                scanned: self.entries.len(),
                skipped: self.skipped,
                current: localized(
                    "loading models",
                    "모델 불러오는 중",
                    "モデルを読み込み中",
                    &self.prefs.language,
                )
                .to_string(),
            },
            |progress| ScanStatus::Scanning {
                found: self.entries.len(),
                scanned: progress.scanned,
                skipped: progress.skipped,
                current: progress.current.clone(),
            },
        );
        // During streaming, keep discovery order so the UI can append rows incrementally
        // (sorted order is applied again when the scan completes).
        let query_for_list = DisplayQuery {
            search_query: &self.search_query,
            library_filter: &self.filter,
            sort_by: self.sort_by,
            sort_ascending: self.sort_ascending,
            preserve_order: true,
        };

        let folder_matches_active_filter = matches!(
            (&self.filter, self.current_folder.as_deref()),
            (LibraryFilter::Folder(f), Some(p)) if f.as_path() == p
        );

        let incremental_ok = self.search_query.trim().is_empty()
            && (matches!(self.filter, LibraryFilter::All) || folder_matches_active_filter)
            && !dedupe_changed
            && !roots_changed
            && self.entries.len() > entries_len_before;

        if incremental_ok {
            self.displayed
                .extend(self.entries[entries_len_before..].iter().cloned());
        } else {
            self.displayed =
                crate::view_model::filtered_sorted_entries(&self.entries, query_for_list);
        }

        let query_for_snapshot = DisplayQuery {
            search_query: &self.search_query,
            library_filter: &self.filter,
            sort_by: self.sort_by,
            sort_ascending: self.sort_ascending,
            preserve_order: true,
        };

        Some(AppViewSnapshot::from_parts_with_displayed_slice(
            &self.entries,
            &self.displayed,
            self.library_roots_for_snapshot(),
            &status,
            &self.prefs,
            query_for_snapshot,
        ))
    }

    fn apply_scan_parts(
        &mut self,
        folder: PathBuf,
        entries: Vec<scanner::StlFileInfo>,
        skipped: usize,
    ) -> AppViewSnapshot {
        let cleaned = filter_excluded_entries(entries, &self.prefs.excluded_folders);
        if !cleaned.is_empty() {
            if let Err(err) = crate::db::upsert_entries_for_library(&folder, &cleaned) {
                eprintln!("Warning: library DB upsert failed: {err:#}");
            }
        }
        Self::merge_library_folder_into_prefs(&mut self.prefs, &folder);
        self.entries.retain(|entry| {
            !crate::view_model::entry_under_library_root(&entry.path, folder.as_path())
        });
        self.entries.extend(cleaned);
        dedupe_entries_by_path(&mut self.entries);
        self.retain_entries_under_library_roots();
        self.current_folder = Some(folder.clone());
        self.prefs.last_folder = Some(folder);
        self.skipped = skipped;
        self.sidecar_writes_enabled = true;
        self.streaming_scan_generation = None;
        self.selected_index = if self.entries.is_empty() {
            None
        } else {
            Some(0)
        };
        self.snapshot_done()
    }

    fn should_apply_completed_scan(&self, folder: &Path) -> bool {
        self.sidecar_writes_enabled
            && self.prefs.library_folders.iter().any(|root| {
                root == folder || crate::view_model::paths_equal_or_same_target(root, folder)
            })
    }

    fn library_roots_for_snapshot(&self) -> &[PathBuf] {
        if self.sidecar_writes_enabled {
            &self.prefs.library_folders
        } else {
            &[]
        }
    }

    fn merge_library_folder_into_prefs(prefs: &mut AppPrefs, folder: &Path) {
        let path = folder.to_path_buf();
        if !prefs.library_folders.iter().any(|existing| {
            existing == &path || crate::view_model::paths_equal_or_same_target(existing, &path)
        }) {
            prefs.library_folders.push(path);
        }
    }

    /// Drop bundled demo entries (and any stray paths) once at least one real library root exists.
    fn retain_entries_under_library_roots(&mut self) {
        if self.prefs.library_folders.is_empty() {
            return;
        }
        let roots = self.prefs.library_folders.clone();
        self.entries.retain(|e| {
            roots
                .iter()
                .any(|r| crate::view_model::entry_under_library_root(&e.path, r))
        });
    }

    fn restored_library_scan_queue(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = self
            .prefs
            .library_folders
            .iter()
            .map(|p| crate::view_model::expand_user_pref_path(p))
            .filter(|path| path.is_dir() && !is_excluded_path(path, &self.prefs.excluded_folders))
            .collect();
        out = crate::view_model::dedupe_paths_keep_order(out);
        if out.is_empty() {
            if let Some(path) = self.prefs.last_folder.clone() {
                if path.is_dir() && !is_excluded_path(&path, &self.prefs.excluded_folders) {
                    out.push(path);
                }
            }
        }
        out
    }

    fn replace_pending_scan_queue<I>(&mut self, folders: I)
    where
        I: IntoIterator<Item = PathBuf>,
    {
        self.pending_scan_queue =
            crate::view_model::dedupe_paths_keep_order(folders.into_iter().collect()).into();
    }

    fn queue_pending_scan_roots<I>(&mut self, folders: I)
    where
        I: IntoIterator<Item = PathBuf>,
    {
        let mut queued: Vec<PathBuf> = self.pending_scan_queue.iter().cloned().collect();
        queued.extend(folders);
        self.pending_scan_queue = crate::view_model::dedupe_paths_keep_order(queued).into();
    }

    fn pop_next_pending_scan_root(&mut self) -> Option<PathBuf> {
        while let Some(folder) = self.pending_scan_queue.pop_front() {
            if folder.is_dir() && !is_excluded_path(&folder, &self.prefs.excluded_folders) {
                return Some(folder);
            }
        }
        None
    }

    fn scan_root_for_folder(&self, folder: &Path) -> Option<PathBuf> {
        let folder = crate::view_model::expand_user_pref_path(folder);
        let mut roots = self.restored_library_scan_queue();
        roots.sort_by_key(|root| std::cmp::Reverse(root.components().count()));
        roots
            .into_iter()
            .find(|root| {
                crate::view_model::paths_equal_or_same_target(root, &folder)
                    || crate::view_model::entry_under_library_root(&folder, root)
            })
            .or_else(|| folder.is_dir().then_some(folder))
    }

    fn active_real_folder(&self) -> Option<PathBuf> {
        if !self.sidecar_writes_enabled {
            return None;
        }
        self.prefs
            .last_folder
            .clone()
            .filter(|path| path.is_dir())
            .or_else(|| {
                self.prefs
                    .library_folders
                    .iter()
                    .find(|path| path.is_dir())
                    .cloned()
            })
    }

    #[cfg(test)]
    fn restored_real_folder_candidate(&self) -> Option<PathBuf> {
        self.restored_library_scan_queue().into_iter().next()
    }

    fn remove_library_root(&mut self, folder: &Path) {
        self.prefs
            .library_folders
            .retain(|root| root.as_path() != folder);
        self.entries.retain(|entry| !entry.path.starts_with(folder));
        if self.prefs.last_folder.as_deref() == Some(folder) {
            self.prefs.last_folder = self.prefs.library_folders.last().cloned();
        }
        if self.current_folder.as_deref() == Some(folder) {
            self.current_folder = self.prefs.last_folder.clone();
        }
    }

    fn add_excluded_folder(&mut self, folder: &Path) {
        if self
            .prefs
            .excluded_folders
            .iter()
            .any(|excluded| folder.starts_with(excluded))
        {
            return;
        }

        self.prefs
            .excluded_folders
            .retain(|excluded| !excluded.starts_with(folder));
        self.prefs.excluded_folders.push(folder.to_path_buf());
    }

    fn remove_excluded_folder(&mut self, folder: &Path) {
        self.prefs
            .excluded_folders
            .retain(|excluded| excluded != folder);
    }

    fn remove_excluded_folder_tree(&mut self, folder: &Path) {
        self.prefs
            .excluded_folders
            .retain(|excluded| !excluded.starts_with(folder));
    }

    fn toggle_sidebar_folder(&mut self, folder: &Path) -> AppViewSnapshot {
        if self
            .prefs
            .collapsed_folders
            .iter()
            .any(|collapsed| collapsed == folder)
        {
            self.prefs
                .collapsed_folders
                .retain(|collapsed| collapsed != folder);
        } else {
            self.prefs
                .collapsed_folders
                .retain(|collapsed| !collapsed.starts_with(folder));
            self.prefs.collapsed_folders.push(folder.to_path_buf());
        }
        self.snapshot_done()
    }

    fn clear_library_state(&mut self) {
        self.entries.clear();
        self.displayed.clear();
        self.current_folder = None;
        self.prefs.last_folder = None;
        self.prefs.library_folders.clear();
        self.pending_scan_queue.clear();
        self.skipped = 0;
        self.sidecar_writes_enabled = false;
        self.selected_index = None;
        self.filter = LibraryFilter::All;
    }

    fn cycle_view_mode(&mut self) {
        self.prefs.view_mode = match ViewMode::from_str(&self.prefs.view_mode) {
            ViewMode::Grid => "list",
            ViewMode::List => "masonry",
            ViewMode::Masonry => "grid",
        }
        .to_string();
    }

    fn choose_view_mode(&mut self, mode: &str) {
        self.prefs.view_mode = ViewMode::from_str(mode).as_str().to_string();
    }

    fn cycle_density(&mut self) {
        self.prefs.density = match Density::from_str(&self.prefs.density) {
            Density::Small => "medium",
            Density::Medium => "large",
            Density::Large => "small",
        }
        .to_string();
    }

    fn choose_density(&mut self, density: &str) {
        self.prefs.density = Density::from_str(density).as_str().to_string();
    }

    fn choose_language(&mut self, language: &str) {
        self.prefs.language = match language {
            "ko" => "ko",
            "ja" => "ja",
            _ => "en",
        }
        .to_string();
    }

    fn cycle_language(&mut self) {
        self.prefs.language = match self.prefs.language.as_str() {
            "en" => "ko",
            "ko" => "ja",
            _ => "en",
        }
        .to_string();
    }

    fn choose_theme(&mut self, theme: &str) {
        self.prefs.theme = match theme {
            "light" => "light",
            _ => "dark",
        }
        .to_string();
    }

    fn choose_accent(&mut self, accent: &str) {
        self.prefs.accent_color = accent_key(accent).to_string();
    }

    fn toggle_theme(&mut self) {
        self.prefs.theme = if self.prefs.theme == "dark" {
            "light".to_string()
        } else {
            "dark".to_string()
        };
    }

    fn choose_sort(&mut self, sort: &str) {
        self.sort_by = sort_by_from_key(sort);
        self.prefs.sort_by = sort_key(self.sort_by).to_string();
    }

    fn choose_thumbnail_style(&mut self, style: &str) {
        self.prefs.thumbnail_style = match style {
            "wire" => "wire",
            "normal" => "normal",
            _ => "iso",
        }
        .to_string();
    }

    fn choose_thumbnail_lighting(&mut self, lighting: &str) {
        self.prefs.thumbnail_lighting = match lighting {
            "even" => "even",
            "rim" => "rim",
            _ => "studio",
        }
        .to_string();
    }

    fn choose_thumbnail_aa(&mut self, aa: &str) {
        self.prefs.thumbnail_aa = match aa {
            "off" => "off",
            "msaa2x" => "msaa2x",
            "msaa8x" => "msaa8x",
            _ => "msaa4x",
        }
        .to_string();
    }

    fn choose_card_label_mode(&mut self, mode: &str) {
        self.prefs.card_label_mode = view_model::CardLabelMode::from_str(mode)
            .as_str()
            .to_string();
    }

    fn choose_date_format_mode(&mut self, mode: &str) {
        self.prefs.date_format_mode = view_model::DateFormatMode::from_str(mode)
            .as_str()
            .to_string();
    }

    fn set_show_file_extensions(&mut self, on: bool) {
        self.prefs.show_file_extensions = on;
    }

    fn choose_startup_view(&mut self, key: &str) {
        self.prefs.startup_view = match key {
            "empty" => "empty",
            _ => "last",
        }
        .to_string();
    }

    fn selected_model_path(&self) -> Option<PathBuf> {
        let idx = self.selected_index?;
        self.displayed.get(idx).map(|entry| entry.path.clone())
    }

    fn displayed_model_path_from_str(&self, model_index: &str) -> Option<PathBuf> {
        let index = model_index.trim().parse::<usize>().ok()?;
        self.displayed.get(index).map(|entry| entry.path.clone())
    }

    fn rename_selected_model(&mut self, requested_name: &str) -> anyhow::Result<Option<PathBuf>> {
        let Some(path) = self.selected_model_path() else {
            return Ok(None);
        };
        persist_model_rename(
            &mut self.entries,
            &path,
            self.sidecar_writes_enabled,
            requested_name,
        )
    }

    fn reset_preview_orbit(&mut self) {
        self.preview_orbit_yaw = -0.62;
        self.preview_orbit_pitch = -0.48;
    }

    fn reset_preview_plate(&mut self) {
        self.preview_plate_index = Some(0);
    }

    fn choose_preview_plate(&mut self, index: i32) {
        self.preview_plate_index = usize::try_from(index).ok();
        self.reset_preview_orbit();
    }

    fn toggle_printer_profile(&mut self, key: &str) -> Result<(), &'static str> {
        let catalog = load_printer_profiles();
        let Some(profile) = printer_profile(&catalog, key) else {
            return Err("Unknown printer profile");
        };
        let mut active = active_printer_keys(&self.prefs, &catalog);
        if active.iter().any(|active_key| active_key == &profile.key) {
            if active.len() == 1 {
                return Err("Keep at least one printer enabled for estimates");
            }
            active.retain(|active_key| active_key != &profile.key);
        } else {
            active.push(profile.key.clone());
        }
        self.prefs.active_printer_keys = active;
        normalize_printer_prefs(&mut self.prefs, &catalog);
        self.ensure_estimate_printer_is_active(&catalog);
        Ok(())
    }

    fn choose_settings_printer_maker(&mut self, maker: &str) {
        let catalog = load_printer_profiles();
        let maker = maker.trim();
        if maker.is_empty() {
            return;
        }
        self.settings_printer_maker = maker.to_string();
        if let Some(profile) = catalog
            .iter()
            .find(|profile| printer_maker_label(profile) == self.settings_printer_maker)
        {
            self.settings_printer_model = printer_model_label(profile);
        }
        // Picking a new maker invalidates the pending nozzle.
        self.settings_printer_nozzle.clear();
    }

    fn choose_settings_printer_model(&mut self, model: &str) {
        let catalog = load_printer_profiles();
        let model = model.trim();
        if catalog.iter().any(|profile| {
            printer_maker_label(profile) == self.settings_printer_maker
                && printer_model_label(profile) == model
        }) {
            self.settings_printer_model = model.to_string();
            // Picking a new model invalidates the pending nozzle.
            self.settings_printer_nozzle.clear();
        }
    }

    /// Record the pending nozzle for the current maker/model picker. This
    /// does NOT add the printer to the active list — the user must press the
    /// "Add to my printers" button. Returns an error only if the nozzle
    /// doesn't match the current maker/model combo.
    fn choose_settings_printer_nozzle(&mut self, nozzle: &str) -> Result<(), &'static str> {
        let catalog = load_printer_profiles();
        let exists = catalog.iter().any(|profile| {
            printer_maker_label(profile) == self.settings_printer_maker
                && printer_model_label(profile) == self.settings_printer_model
                && format!("{:.1}mm", profile.nozzle_diameter_mm) == nozzle.trim()
        });
        if !exists {
            return Err("Unknown printer/nozzle combination");
        }
        self.settings_printer_nozzle = nozzle.trim().to_string();
        Ok(())
    }

    /// Returns the profile that the maker/model/nozzle picker currently
    /// resolves to (if all three are set and match a catalog entry).
    fn pending_printer_profile(&self) -> Option<PrinterProfile> {
        let catalog = load_printer_profiles();
        catalog.into_iter().find(|profile| {
            printer_maker_label(profile) == self.settings_printer_maker
                && printer_model_label(profile) == self.settings_printer_model
                && format!("{:.1}mm", profile.nozzle_diameter_mm) == self.settings_printer_nozzle
        })
    }

    /// Returns true when the pending picker selection is a *new* (not
    /// already-active) profile that can be added to the user's printer list.
    #[allow(dead_code)] // Used by tests + future direct callers; UI derives this in apply_settings.
    fn can_add_pending_printer(&self) -> bool {
        let Some(profile) = self.pending_printer_profile() else {
            return false;
        };
        !self
            .prefs
            .active_printer_keys
            .iter()
            .any(|key| key == &profile.key)
    }

    /// Commit the maker/model/nozzle selection: add it to active printers
    /// and, if this was the first printer, set it as default. Returns the
    /// added profile's key on success.
    fn add_pending_printer(&mut self) -> Result<String, &'static str> {
        let Some(profile) = self.pending_printer_profile() else {
            return Err("Pick maker, model, and nozzle first");
        };
        let catalog = load_printer_profiles();
        let mut active = active_printer_keys(&self.prefs, &catalog);
        if active.iter().any(|key| key == &profile.key) {
            return Err("Printer already in your list");
        }
        let was_empty = active.is_empty();
        active.push(profile.key.clone());
        self.prefs.active_printer_keys = active;
        if was_empty || self.prefs.default_printer_key.is_empty() {
            self.prefs.default_printer_key = profile.key.clone();
            self.estimate_printer_key = profile.key.clone();
        }
        normalize_printer_prefs(&mut self.prefs, &catalog);
        self.settings_printer_nozzle.clear();
        Ok(profile.key)
    }

    fn choose_default_printer(&mut self, key: &str) {
        let catalog = load_printer_profiles();
        if let Some(profile) = printer_profile(&catalog, key) {
            let mut active = active_printer_keys(&self.prefs, &catalog);
            if !active.iter().any(|active_key| active_key == &profile.key) {
                active.push(profile.key.clone());
            }
            self.prefs.active_printer_keys = active;
            self.prefs.default_printer_key = profile.key.clone();
            self.estimate_printer_key = profile.key.clone();
            normalize_printer_prefs(&mut self.prefs, &catalog);
        }
    }

    fn choose_estimate_printer(&mut self, key: &str) {
        let catalog = load_printer_profiles();
        if active_printer_profiles(&self.prefs, &catalog)
            .iter()
            .any(|profile| profile.key == key.trim())
        {
            self.estimate_printer_key = key.to_string();
        }
    }

    fn selected_estimate_profile(&mut self, catalog: &[PrinterProfile]) -> PrinterProfile {
        self.ensure_estimate_printer_is_active(catalog);
        printer_profile(catalog, &self.estimate_printer_key)
            .cloned()
            .unwrap_or_else(|| default_printer_profile(&self.prefs, catalog))
    }

    fn ensure_estimate_printer_is_active(&mut self, catalog: &[PrinterProfile]) {
        let active = active_printer_keys(&self.prefs, catalog);
        if !active
            .iter()
            .any(|active_key| active_key == &self.estimate_printer_key)
        {
            self.estimate_printer_key = default_printer_key_for_prefs(&self.prefs, catalog);
        }
    }

    fn orbit_preview(&mut self, delta_x: f32, delta_y: f32) {
        self.preview_orbit_yaw += delta_x * 0.012;
        self.preview_orbit_pitch = (self.preview_orbit_pitch + delta_y * 0.010).clamp(-1.25, 1.25);
    }

    fn selected_preview(&mut self, entry: &scanner::StlFileInfo) -> Option<PreviewSelection> {
        if entry.stl_type == scanner::StlType::ThreeMf {
            return self.selected_three_mf_preview(entry);
        }

        let selected_path = entry.path.clone();
        let needs_load = self
            .preview_mesh
            .as_ref()
            .is_none_or(|(path, _)| *path != selected_path);
        if needs_load {
            self.preview_mesh = scanner::parse_preview_mesh(&selected_path)
                .ok()
                .flatten()
                .map(|mesh| (selected_path.clone(), mesh));
        }
        self.preview_mesh
            .as_ref()
            .and_then(|(path, mesh)| (*path == selected_path).then_some(mesh.clone()))
            .map(|mesh| PreviewSelection {
                triangle_count: mesh.faces.len(),
                dimensions: scanner::mesh_dimensions(&mesh),
                volume_cm3: scanner::mesh_volume_cm3(&mesh),
                mesh,
                plate_count: 0,
                selected_label: String::new(),
                tab_rows: Vec::new(),
            })
    }

    fn selected_three_mf_preview(
        &mut self,
        entry: &scanner::StlFileInfo,
    ) -> Option<PreviewSelection> {
        let selected_path = entry.path.clone();
        let needs_load = self
            .preview_plates
            .as_ref()
            .is_none_or(|(path, _)| *path != selected_path);
        if needs_load {
            self.preview_plates = scanner::parse_preview_plates(&selected_path)
                .ok()
                .flatten()
                .map(|plates| (selected_path.clone(), plates));
        }
        let plates = self
            .preview_plates
            .as_ref()
            .and_then(|(path, plates)| (*path == selected_path).then_some(plates.clone()))?;
        if plates.is_empty() {
            return None;
        }

        if plates.len() == 1 {
            let mesh = plates[0].mesh.clone();
            return Some(PreviewSelection {
                triangle_count: mesh.faces.len(),
                dimensions: scanner::mesh_dimensions(&mesh),
                volume_cm3: scanner::mesh_volume_cm3(&mesh),
                mesh,
                plate_count: 1,
                selected_label: localized_plate_label(&plates[0].label, 0, &self.prefs.language),
                tab_rows: Vec::new(),
            });
        }

        if let Some(index) = self.preview_plate_index {
            if index >= plates.len() {
                self.preview_plate_index = Some(0);
            }
        }

        let selected_plate_index = self.preview_plate_index;
        let language = self.prefs.language.clone();
        let (mesh, selected_label) = if let Some(index) = selected_plate_index {
            (
                plates[index].mesh.clone(),
                localized_plate_label(&plates[index].label, index, &language),
            )
        } else {
            (
                scanner::arranged_three_mf_overview_mesh(&plates)?,
                localized("All plates", "전체 플레이트", "全プレート", &language).to_string(),
            )
        };
        let triangle_count = if selected_plate_index.is_some() {
            mesh.faces.len()
        } else {
            plates.iter().map(|plate| plate.mesh.faces.len()).sum()
        };
        let mut tab_rows = Vec::with_capacity(plates.len() + 1);
        tab_rows.push(PreviewPlateTab {
            label: localized("All plates", "전체 플레이트", "全プレート", &language).to_string(),
            index: -1,
            selected: selected_plate_index.is_none(),
        });
        tab_rows.extend(
            plates
                .iter()
                .enumerate()
                .map(|(index, plate)| PreviewPlateTab {
                    label: localized_plate_label(&plate.label, index, &language),
                    index: index as i32,
                    selected: selected_plate_index == Some(index),
                }),
        );

        Some(PreviewSelection {
            dimensions: scanner::mesh_dimensions(&mesh),
            volume_cm3: scanner::mesh_volume_cm3(&mesh),
            mesh,
            plate_count: plates.len(),
            selected_label,
            tab_rows,
            triangle_count,
        })
    }

    fn reselect_path(&mut self, path: &Path) {
        self.selected_index = self.displayed.iter().position(|entry| entry.path == path);
    }
}

fn apply_snapshot(ui: &ModelRackWindow, snapshot: &AppViewSnapshot) {
    ui.set_app_title(strings::APP_TITLE.into());
    ui.set_app_version(format!("v{}", env!("CARGO_PKG_VERSION")).into());
    ui.set_library_label(snapshot.library_label.clone().into());
    ui.set_status_text(snapshot.status_text.clone().into());
    ui.set_density_label(snapshot.density_label.clone().into());
    ui.set_view_mode_label(snapshot.view_mode_label.clone().into());
    ui.set_browser_message(snapshot.browser.empty_message.clone().into());
    ui.set_sort_label(snapshot.sort_label.clone().into());
    ui.set_browser_count_label(
        browser_count_label(
            snapshot.browser.displayed,
            snapshot.browser.total,
            &snapshot.language,
        )
        .into(),
    );
    ui.set_all_count(snapshot.sidebar.all as i32);
    ui.set_recent_count(snapshot.sidebar.recent as i32);
    ui.set_favorites_count(snapshot.sidebar.favorites as i32);
    ui.set_printed_count(snapshot.sidebar.printed as i32);
    ui.set_duplicates_count(snapshot.sidebar.duplicates as i32);
    ui.set_ready_count(snapshot.sidebar.ready as i32);
    ui.set_errors_count(snapshot.sidebar.errors as i32);
    ui.set_active_filter_key(snapshot.active_filter_key.clone().into());
    let cards = snapshot
        .cards
        .iter()
        .map(browser_card)
        .collect::<Vec<BrowserCard>>();
    sync_browser_cards(ui, cards);
    let folders = snapshot
        .folders
        .iter()
        .map(|folder| SidebarItem {
            key: format!("folder:{}", folder.path.display()).into(),
            label: folder.label.clone().into(),
            count: folder.count as i32,
            depth: folder.depth as i32,
            expandable: folder.expandable,
            expanded: folder.expanded,
            visible: folder.visible,
        })
        .collect::<Vec<SidebarItem>>();
    ui.set_folder_items(slint::ModelRc::new(slint::VecModel::from(folders)));
    let tags = snapshot
        .tags
        .iter()
        .map(|tag| SidebarItem {
            key: format!("tag:{}", tag.label).into(),
            label: tag.label.clone().into(),
            count: tag.count as i32,
            depth: 0,
            expandable: false,
            expanded: false,
            visible: true,
        })
        .collect::<Vec<SidebarItem>>();
    ui.set_tag_items(slint::ModelRc::new(slint::VecModel::from(tags)));
}

fn browser_count_label(displayed: usize, total: usize, language: &str) -> String {
    browser_count_label_for_language(displayed, total, language)
}

fn sync_browser_cards(ui: &ModelRackWindow, cards: Vec<BrowserCard>) {
    let current = ui.get_model_cards();
    let Some(model) = current
        .as_any()
        .downcast_ref::<slint::VecModel<BrowserCard>>()
    else {
        ui.set_model_cards(slint::ModelRc::new(slint::VecModel::from(cards)));
        return;
    };

    let old_len = model.row_count();

    if old_len == 0 && !cards.is_empty() {
        for card in cards {
            model.push(card);
        }
        return;
    }

    if cards.len() > old_len && old_len > 0 {
        let prefix_ok = (0..old_len).all(|row| {
            model
                .row_data(row)
                .zip(cards.get(row))
                .is_some_and(|(old, new)| old.stable_key == new.stable_key)
        });
        if prefix_ok {
            for row in 0..old_len {
                if let Some(new_card) = cards.get(row).cloned() {
                    model.set_row_data(row, new_card);
                }
            }
            for row in old_len..cards.len() {
                model.push(cards[row].clone());
            }
            return;
        }
    }

    let same_cards = model.row_count() == cards.len()
        && cards.iter().enumerate().all(|(row, card)| {
            model
                .row_data(row)
                .is_some_and(|old| old.stable_key == card.stable_key)
        });

    if !same_cards {
        model.set_vec(cards);
        return;
    }

    for (row, card) in cards.into_iter().enumerate() {
        model.set_row_data(row, card);
    }
}

fn detail_parent_label(entry: &scanner::StlFileInfo, state: &ShellState) -> String {
    let Some(parent) = entry.path.parent() else {
        return String::new();
    };

    if !state.sidecar_writes_enabled {
        let demo_root = Path::new(DEMO_ROOT);
        let relative = parent.strip_prefix(demo_root).unwrap_or(parent);
        if relative.as_os_str().is_empty() {
            "Sample library/".to_string()
        } else {
            format!("Sample library/{}/", relative.display())
        }
    } else {
        format!("{}/", display_path_label(parent))
    }
}

fn settings_folder_label(state: &ShellState) -> String {
    if state.sidecar_writes_enabled {
        let roots = &state.prefs.library_folders;
        if roots.is_empty() {
            localized(
                "No folder selected",
                "선택한 폴더 없음",
                "フォルダ未選択",
                &state.prefs.language,
            )
            .to_string()
        } else if roots.len() == 1 {
            display_path_label(&roots[0])
        } else {
            format!(
                "{} ({})",
                localized(
                    "Multiple folders",
                    "여러 폴더",
                    "複数フォルダ",
                    &state.prefs.language
                ),
                roots
                    .iter()
                    .map(|p| display_path_label(p))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    } else {
        localized(
            "Sample library (demo, memory-only)",
            "샘플 라이브러리 (데모, 메모리 전용)",
            "サンプルライブラリ（デモ、メモリのみ）",
            &state.prefs.language,
        )
        .to_string()
    }
}

fn apply_filter_key(ui: &ModelRackWindow, state: &Rc<RefCell<ShellState>>, key: &str) {
    let snapshot = {
        let mut state = state.borrow_mut();
        if let Some(filter) = smart_filter_from_key(key) {
            state.filter = filter;
        }
        state.selected_index = None;
        state.snapshot_done()
    };
    apply_snapshot(ui, &snapshot);
    apply_detail_rc(ui, state);
    apply_settings(ui, &state.borrow());
}

fn toggle_sidebar_folder(ui: &ModelRackWindow, state: &Rc<RefCell<ShellState>>, key: &str) {
    let Some(folder) = folder_path_from_filter_key(key) else {
        ui.set_status_text("No folder path available for that sidebar item".into());
        return;
    };
    let snapshot = {
        let mut state = state.borrow_mut();
        state.toggle_sidebar_folder(&folder)
    };
    apply_snapshot(ui, &snapshot);
    apply_detail_rc(ui, state);
    apply_settings(ui, &state.borrow());
    save_prefs_status(ui, &state.borrow());
}

fn apply_settings(ui: &ModelRackWindow, state: &ShellState) {
    apply_theme(ui, &state.prefs.theme, &state.prefs.accent_color);
    let discovered_slicers = discover_slicer_candidates();
    let slicer_rows = slicer_choice_rows(&state.prefs.slicer_path, &discovered_slicers);
    let (selected_slicer_icon, selected_slicer_icon_ready) = slicer_rows
        .iter()
        .find(|row| row.selected)
        .map(|row| load_ui_image(row.icon_path.as_deref()))
        .unwrap_or_else(|| (slint::Image::default(), false));
    ui.set_settings_open(state.settings_open);
    ui.set_settings_tab(state.settings_tab.clone().into());
    ui.set_settings_language_key(state.prefs.language.clone().into());
    ui.set_settings_language_label(language_label(&state.prefs.language).into());
    ui.set_settings_theme_key(state.prefs.theme.clone().into());
    ui.set_settings_theme_label(theme_label(&state.prefs.theme).into());
    ui.set_settings_accent_key(accent_key(&state.prefs.accent_color).into());
    ui.set_settings_sort_key(sort_key(state.sort_by).into());
    ui.set_settings_sort_ascending(state.sort_ascending);
    ui.set_settings_thumbnail_style(state.prefs.thumbnail_style.clone().into());
    ui.set_settings_thumbnail_lighting(state.prefs.thumbnail_lighting.clone().into());
    ui.set_settings_thumbnail_aa(state.prefs.thumbnail_aa.clone().into());
    ui.set_settings_card_label_key(
        CardLabelMode::from_str(&state.prefs.card_label_mode)
            .as_str()
            .into(),
    );
    ui.set_settings_date_format_key(
        DateFormatMode::from_str(&state.prefs.date_format_mode)
            .as_str()
            .into(),
    );
    ui.set_settings_show_file_extensions(state.prefs.show_file_extensions);
    ui.set_settings_startup_key(
        match state.prefs.startup_view.as_str() {
            "empty" => "empty",
            _ => "last",
        }
        .into(),
    );
    ui.set_settings_folder_label(settings_folder_label(state).into());
    ui.set_settings_density_label(Density::from_str(&state.prefs.density).as_str().into());
    ui.set_settings_slicer_label(
        slicer_label_for_path(&state.prefs.slicer_path, &slicer_rows).into(),
    );
    ui.set_settings_slicer_icon_image(selected_slicer_icon);
    ui.set_settings_slicer_icon_ready(selected_slicer_icon_ready);
    ui.set_settings_slicer_candidates(slint::ModelRc::new(slint::VecModel::from(
        slicer_rows
            .into_iter()
            .map(|row| {
                let (icon_image, icon_ready) = load_ui_image(row.icon_path.as_deref());
                SlicerCandidate {
                    label: row.label.into(),
                    path: row.path.into(),
                    detail: row.detail.into(),
                    icon_image,
                    icon_ready,
                    selected: row.selected,
                }
            })
            .collect::<Vec<SlicerCandidate>>(),
    )));
    let printer_catalog = load_printer_profiles();
    let printer_rows = settings_printer_choices(&state.prefs, &printer_catalog);
    ui.set_settings_printer_summary(
        settings_printer_summary(&state.prefs, &printer_catalog).into(),
    );
    ui.set_settings_default_printer_label(
        default_printer_profile(&state.prefs, &printer_catalog)
            .label
            .into(),
    );
    ui.set_settings_printer_active_count(
        active_printer_profiles(&state.prefs, &printer_catalog).len() as i32,
    );
    ui.set_settings_printer_profiles(slint::ModelRc::new(slint::VecModel::from(printer_rows)));
    let maker_values = unique_sorted(
        printer_catalog
            .iter()
            .map(printer_maker_label)
            .collect::<Vec<_>>(),
    );
    let model_values = unique_sorted(
        printer_catalog
            .iter()
            .filter(|profile| printer_maker_label(profile) == state.settings_printer_maker)
            .map(printer_model_label)
            .collect::<Vec<_>>(),
    );
    // Fixed quartet in Settings → Printer; catalog must define matching profiles per maker/model.
    const SETTINGS_PRINTER_NOZZLE_PICKER_LABELS: [&str; 4] = ["0.2mm", "0.4mm", "0.6mm", "0.8mm"];
    let nozzle_values: Vec<String> = SETTINGS_PRINTER_NOZZLE_PICKER_LABELS
        .iter()
        .map(|label| (*label).to_string())
        .collect();
    ui.set_settings_printer_maker_label(state.settings_printer_maker.clone().into());
    ui.set_settings_printer_model_label(state.settings_printer_model.clone().into());
    ui.set_settings_printer_makers(slint::ModelRc::new(slint::VecModel::from(choice_rows(
        maker_values,
        &state.settings_printer_maker,
    ))));
    ui.set_settings_printer_models(slint::ModelRc::new(slint::VecModel::from(choice_rows(
        model_values,
        &state.settings_printer_model,
    ))));
    ui.set_settings_printer_nozzles(slint::ModelRc::new(slint::VecModel::from(
        nozzle_values
            .into_iter()
            .map(|value| ChoiceRow {
                selected: value == state.settings_printer_nozzle,
                key: value.clone().into(),
                label: value.into(),
            })
            .collect::<Vec<ChoiceRow>>(),
    )));
    ui.set_settings_printer_nozzle_label(state.settings_printer_nozzle.clone().into());
    let pending_active = !state.settings_printer_nozzle.is_empty();
    let already_added = pending_active
        && state
            .pending_printer_profile()
            .map(|profile| {
                state
                    .prefs
                    .active_printer_keys
                    .iter()
                    .any(|key| key == &profile.key)
            })
            .unwrap_or(false);
    ui.set_settings_printer_pending_active(pending_active);
    ui.set_settings_printer_can_add(pending_active && !already_added);
    ui.set_settings_printer_pending_label(
        printer_pending_label(
            &state.prefs.language,
            &state.settings_printer_maker,
            &state.settings_printer_model,
            &state.settings_printer_nozzle,
            already_added,
        )
        .into(),
    );
    ui.set_settings_update_status(update_status_text_for_language(&state.prefs.language).into());
}

fn apply_theme(ui: &ModelRackWindow, theme: &str, accent: &str) {
    let globals = ui.global::<Theme>();
    if theme == "light" {
        globals.set_bg_0(rgb(0xf6, 0xf7, 0xf9));
        globals.set_bg_1(rgb(0xee, 0xf1, 0xf4));
        globals.set_bg_2(rgb(0xe5, 0xe9, 0xee));
        globals.set_bg_3(rgb(0xd8, 0xde, 0xe6));
        globals.set_bg_4(rgb(0xc8, 0xd1, 0xdc));
        globals.set_bg_5(rgb(0xb9, 0xc6, 0xd4));
        globals.set_fg_0(rgb(0x12, 0x18, 0x20));
        globals.set_fg_1(rgb(0x36, 0x40, 0x4d));
        globals.set_fg_2(rgb(0x66, 0x70, 0x7d));
        globals.set_fg_3(rgb(0x8a, 0x94, 0xa0));
        globals.set_line(rgba(0x00, 0x00, 0x00, 0x18));
        globals.set_line_2(rgba(0x00, 0x00, 0x00, 0x26));
        globals.set_line_3(rgba(0x00, 0x00, 0x00, 0x36));
    } else {
        globals.set_bg_0(rgb(0x15, 0x17, 0x1b));
        globals.set_bg_1(rgb(0x1b, 0x1e, 0x23));
        globals.set_bg_2(rgb(0x20, 0x23, 0x29));
        globals.set_bg_3(rgb(0x29, 0x2d, 0x34));
        globals.set_bg_4(rgb(0x34, 0x39, 0x43));
        globals.set_bg_5(rgb(0x46, 0x50, 0x5c));
        globals.set_fg_0(rgb(0xf0, 0xf2, 0xf5));
        globals.set_fg_1(rgb(0xc0, 0xc5, 0xcc));
        globals.set_fg_2(rgb(0x8c, 0x92, 0x9b));
        globals.set_fg_3(rgb(0x60, 0x67, 0x71));
        globals.set_line(rgba(0xff, 0xff, 0xff, 0x0f));
        globals.set_line_2(rgba(0xff, 0xff, 0xff, 0x1a));
        globals.set_line_3(rgba(0xff, 0xff, 0xff, 0x29));
    }
    let palette = accent_palette(accent);
    globals.set_accent(rgb(palette.r, palette.g, palette.b));
    globals.set_accent_dim(rgba(palette.r, palette.g, palette.b, 0x2e));
    globals.set_accent_line(rgba(palette.r, palette.g, palette.b, 0x59));
    globals.set_accent_dark(rgb(palette.dark_r, palette.dark_g, palette.dark_b));
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb_u8(r, g, b)
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_argb_u8(a, r, g, b)
}

#[derive(Clone, Copy)]
struct AccentPalette {
    r: u8,
    g: u8,
    b: u8,
    dark_r: u8,
    dark_g: u8,
    dark_b: u8,
}

fn accent_key(accent: &str) -> &'static str {
    match accent {
        "purple" => "purple",
        "orange" => "orange",
        "green" => "green",
        _ => "teal",
    }
}

fn accent_palette(accent: &str) -> AccentPalette {
    match accent_key(accent) {
        "purple" => AccentPalette {
            r: 0xa7,
            g: 0x79,
            b: 0xd9,
            dark_r: 0x31,
            dark_g: 0x24,
            dark_b: 0x44,
        },
        "orange" => AccentPalette {
            r: 0xd9,
            g: 0x90,
            b: 0x57,
            dark_r: 0x46,
            dark_g: 0x2c,
            dark_b: 0x19,
        },
        "green" => AccentPalette {
            r: 0x5f,
            g: 0xb8,
            b: 0x7a,
            dark_r: 0x1e,
            dark_g: 0x3e,
            dark_b: 0x28,
        },
        _ => AccentPalette {
            r: 0x5f,
            g: 0xb8,
            b: 0xd4,
            dark_r: 0x1a,
            dark_g: 0x3a,
            dark_b: 0x44,
        },
    }
}

fn language_key(language: &str) -> &str {
    match language {
        "ko" => "ko",
        "ja" => "ja",
        _ => "en",
    }
}

fn localized<'a>(en: &'a str, ko: &'a str, ja: &'a str, language: &str) -> &'a str {
    match language_key(language) {
        "ko" => ko,
        "ja" => ja,
        _ => en,
    }
}

fn localized_plate_label(label: &str, index: usize, language: &str) -> String {
    if label == format!("Plate {}", index + 1) {
        match language_key(language) {
            "ko" => format!("플레이트 {}", index + 1),
            "ja" => format!("プレート {}", index + 1),
            _ => label.to_string(),
        }
    } else {
        label.to_string()
    }
}

fn regenerate_thumbnails_status(language: &str, queued: usize, cleared: bool) -> String {
    if !cleared {
        return clear_cache_status(language);
    }
    match language_key(language) {
        "ko" => format!("썸네일 캐시 비움 · 재스캔 시 {}개 재생성", queued),
        "ja" => format!(
            "サムネイルキャッシュをクリア · 再スキャン時に {} 件を再生成",
            queued
        ),
        _ => format!(
            "Thumbnail cache cleared · {} thumbnails will regenerate on rescan",
            queued
        ),
    }
}

fn clear_cache_status(language: &str) -> String {
    match language_key(language) {
        "ko" => "썸네일 캐시를 비웠습니다".to_string(),
        "ja" => "サムネイルキャッシュをクリアしました".to_string(),
        _ => "Thumbnail cache cleared".to_string(),
    }
}

fn printer_pending_label(
    language: &str,
    maker: &str,
    model: &str,
    nozzle: &str,
    already_added: bool,
) -> String {
    if nozzle.is_empty() {
        return match language_key(language) {
            "ko" => "노즐을 선택하세요".to_string(),
            "ja" => "ノズルを選択してください".to_string(),
            _ => "Pick a nozzle to add a printer".to_string(),
        };
    }
    let combo = format!("{} {} · {}", maker, model, nozzle);
    if already_added {
        match language_key(language) {
            "ko" => format!("{} · 이미 추가됨", combo),
            "ja" => format!("{} · 追加済み", combo),
            _ => format!("{} · already added", combo),
        }
    } else {
        combo
    }
}

fn printer_add_error_text(language: &str, message: &str) -> String {
    let key = language_key(language);
    if message.contains("already") {
        match key {
            "ko" => "이미 추가된 프린터입니다".to_string(),
            "ja" => "すでに追加済みのプリンタです".to_string(),
            _ => message.to_string(),
        }
    } else if message.contains("Pick") || message.contains("nozzle") {
        match key {
            "ko" => "제조사 → 모델 → 노즐을 먼저 선택하세요".to_string(),
            "ja" => "まずメーカー → モデル → ノズルを選択してください".to_string(),
            _ => message.to_string(),
        }
    } else {
        message.to_string()
    }
}

fn update_status_text_for_language(language: &str) -> String {
    match language_key(language) {
        "ko" => format!(
            "릴리스 채널: GitHub Releases · 현재 {} · 아직 확인 안 함",
            env!("CARGO_PKG_VERSION")
        ),
        "ja" => format!(
            "リリースチャンネル: GitHub Releases · 現在 {} · 未確認",
            env!("CARGO_PKG_VERSION")
        ),
        _ => format!(
            "Release channel: GitHub Releases · current {} · not checked",
            env!("CARGO_PKG_VERSION")
        ),
    }
}

fn bundled_printer_profiles_json() -> &'static str {
    include_str!("../assets/data/printer_profiles.json")
}

fn load_printer_profiles() -> Vec<PrinterProfile> {
    load_printer_profiles_from_str(bundled_printer_profiles_json())
}

fn load_printer_profiles_from_str(json: &str) -> Vec<PrinterProfile> {
    let catalog = serde_json::from_str::<PrinterProfileCatalog>(json)
        .unwrap_or_else(|_| fallback_printer_catalog());
    let mut profiles = catalog
        .profiles
        .into_iter()
        .filter_map(normalize_printer_profile)
        .collect::<Vec<_>>();
    if profiles.is_empty() {
        profiles = fallback_printer_catalog()
            .profiles
            .into_iter()
            .filter_map(normalize_printer_profile)
            .collect();
    }
    profiles
}

fn fallback_printer_catalog() -> PrinterProfileCatalog {
    PrinterProfileCatalog {
        _version: 1,
        default_profile_key: "bambu-p1s-0.4".to_string(),
        profiles: vec![PrinterProfile {
            key: "bambu-p1s-0.4".to_string(),
            label: "Bambu P1S · 0.4mm".to_string(),
            printer_label: "Bambu P1S".to_string(),
            process_label: "0.20 Standard".to_string(),
            nozzle_diameter_mm: 0.4,
            build_volume: [256.0, 256.0, 256.0],
            layer_height_mm: 0.20,
            grams_per_minute: 0.60,
        }],
    }
}

fn normalize_printer_profile(mut profile: PrinterProfile) -> Option<PrinterProfile> {
    profile.key = profile.key.trim().to_string();
    profile.label = profile.label.trim().to_string();
    profile.printer_label = profile.printer_label.trim().to_string();
    profile.process_label = profile.process_label.trim().to_string();
    if profile.key.is_empty()
        || profile.label.is_empty()
        || !profile
            .build_volume
            .iter()
            .all(|value| value.is_finite() && *value > 0.0)
        || !profile.layer_height_mm.is_finite()
        || profile.layer_height_mm <= 0.0
        || !profile.grams_per_minute.is_finite()
        || profile.grams_per_minute <= 0.0
    {
        return None;
    }
    if profile.printer_label.is_empty() {
        profile.printer_label = profile.label.clone();
    }
    if profile.process_label.is_empty() {
        profile.process_label = format!("{:.2}mm layer", profile.layer_height_mm);
    }
    if !profile.nozzle_diameter_mm.is_finite() || profile.nozzle_diameter_mm <= 0.0 {
        profile.nozzle_diameter_mm = 0.4;
    }
    Some(profile)
}

fn bundled_default_printer_key(catalog: &[PrinterProfile]) -> String {
    serde_json::from_str::<PrinterProfileCatalog>(bundled_printer_profiles_json())
        .ok()
        .and_then(|catalog_json| {
            let key = catalog_json.default_profile_key.trim().to_string();
            catalog
                .iter()
                .any(|profile| profile.key == key)
                .then_some(key)
        })
        .or_else(|| catalog.first().map(|profile| profile.key.clone()))
        .unwrap_or_else(|| "bambu-p1s-0.4".to_string())
}

fn printer_profile<'a>(catalog: &'a [PrinterProfile], key: &str) -> Option<&'a PrinterProfile> {
    let trimmed = key.trim();
    catalog
        .iter()
        .find(|profile| profile.key == trimmed)
        .or_else(|| {
            legacy_printer_key(trimmed)
                .and_then(|key| catalog.iter().find(|profile| profile.key == key))
        })
}

fn legacy_printer_key(key: &str) -> Option<&'static str> {
    match key {
        "bambu-p1s" => Some("bambu-p1s-0.4"),
        "prusa-mk4" => Some("prusa-mk4-0.4"),
        "creality-k1" => Some("creality-k1-0.4"),
        "snapmaker-j1" => Some("snapmaker-j1-0.4"),
        _ => None,
    }
}

fn active_printer_keys(prefs: &AppPrefs, catalog: &[PrinterProfile]) -> Vec<String> {
    let mut keys = Vec::new();
    for key in prefs
        .active_printer_keys
        .iter()
        .filter_map(|key| printer_profile(catalog, key).map(|profile| profile.key.clone()))
    {
        if !keys.iter().any(|existing| existing == &key) {
            keys.push(key);
        }
    }
    if keys.is_empty() {
        keys.push(default_printer_key_for_prefs(prefs, catalog));
    }
    keys
}

fn active_printer_profiles(prefs: &AppPrefs, catalog: &[PrinterProfile]) -> Vec<PrinterProfile> {
    active_printer_keys(prefs, catalog)
        .iter()
        .filter_map(|key| printer_profile(catalog, key).cloned())
        .collect()
}

fn default_printer_key_for_prefs(prefs: &AppPrefs, catalog: &[PrinterProfile]) -> String {
    let active = prefs
        .active_printer_keys
        .iter()
        .filter_map(|key| printer_profile(catalog, key).map(|profile| profile.key.clone()))
        .collect::<Vec<_>>();
    if active.iter().any(|key| {
        printer_profile(catalog, &prefs.default_printer_key)
            .is_some_and(|profile| profile.key == *key)
    }) {
        printer_profile(catalog, &prefs.default_printer_key)
            .map(|profile| profile.key.clone())
            .unwrap_or_else(|| bundled_default_printer_key(catalog))
    } else {
        active
            .first()
            .cloned()
            .unwrap_or_else(|| bundled_default_printer_key(catalog))
    }
}

fn default_printer_profile(prefs: &AppPrefs, catalog: &[PrinterProfile]) -> PrinterProfile {
    printer_profile(catalog, &default_printer_key_for_prefs(prefs, catalog))
        .cloned()
        .or_else(|| catalog.first().cloned())
        .unwrap_or_else(|| fallback_printer_catalog().profiles.remove(0))
}

fn normalize_printer_prefs(prefs: &mut AppPrefs, catalog: &[PrinterProfile]) {
    prefs.active_printer_keys = active_printer_keys(prefs, catalog);
    prefs.default_printer_key = default_printer_key_for_prefs(prefs, catalog);
}

fn printer_detail(profile: &PrinterProfile) -> String {
    format!(
        "{} · {:.1}mm nozzle · {} · {} × {} × {}",
        profile.printer_label,
        profile.nozzle_diameter_mm,
        profile.process_label,
        profile.build_volume[0] as u32,
        profile.build_volume[1] as u32,
        profile.build_volume[2] as u32
    )
}

fn printer_model_label(profile: &PrinterProfile) -> String {
    let label = profile.printer_label.trim();
    // Keep this list in sync with the makers represented in
    // assets/data/printer_profiles.json. Each entry's `printer_label` must
    // start with one of these maker tokens — that's how maker/model are
    // split for the hierarchical picker in Settings.
    for maker in [
        "Bambu",
        "Prusa",
        "Creality",
        "Anycubic",
        "Elegoo",
        "Sovol",
        "Snapmaker",
        "FlashForge",
        "Voron",
    ] {
        if let Some(model) = label.strip_prefix(&format!("{maker} ")) {
            return model.to_string();
        }
    }
    label.to_string()
}

fn printer_maker_label(profile: &PrinterProfile) -> String {
    profile
        .printer_label
        .split_whitespace()
        .next()
        .unwrap_or(profile.printer_label.as_str())
        .to_string()
}

fn choice_rows(values: Vec<String>, selected: &str) -> Vec<ChoiceRow> {
    values
        .into_iter()
        .map(|value| ChoiceRow {
            key: value.clone().into(),
            label: value.clone().into(),
            selected: value == selected,
        })
        .collect()
}

fn unique_sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    values.dedup();
    values
}

fn settings_printer_summary(prefs: &AppPrefs, catalog: &[PrinterProfile]) -> String {
    let active = active_printer_profiles(prefs, catalog);
    if active.len() == 1 {
        active[0].label.clone()
    } else {
        format!("{} profiles", active.len())
    }
}

fn settings_printer_status(prefs: &AppPrefs, catalog: &[PrinterProfile]) -> String {
    let active = active_printer_profiles(prefs, catalog);
    if active.len() == 1 {
        format!("Print estimates use {}", active[0].label)
    } else {
        format!(
            "{} printer profiles enabled · default {}",
            active.len(),
            default_printer_profile(prefs, catalog).label
        )
    }
}

fn settings_printer_choices(
    prefs: &AppPrefs,
    catalog: &[PrinterProfile],
) -> Vec<PrinterProfileChoice> {
    // "My printers" — only the active set, in user order, with default flagged.
    let default_key = default_printer_key_for_prefs(prefs, catalog);
    let mut rows: Vec<PrinterProfileChoice> = Vec::new();
    for key in &prefs.active_printer_keys {
        if let Some(profile) = printer_profile(catalog, key) {
            rows.push(PrinterProfileChoice {
                key: profile.key.clone().into(),
                label: profile.label.clone().into(),
                detail: printer_detail(profile).into(),
                selected: true,
                defaulted: default_key == profile.key,
            });
        }
    }
    rows
}

fn estimate_printer_choices(
    prefs: &AppPrefs,
    catalog: &[PrinterProfile],
    selected_key: &str,
) -> Vec<PrinterProfileChoice> {
    let default_key = default_printer_key_for_prefs(prefs, catalog);
    active_printer_profiles(prefs, catalog)
        .iter()
        .map(|profile| PrinterProfileChoice {
            key: profile.key.clone().into(),
            label: profile.label.clone().into(),
            detail: printer_detail(profile).into(),
            selected: profile.key == selected_key,
            defaulted: default_key == profile.key,
        })
        .collect()
}

fn estimate_print_for_dimensions(
    dimensions: [f32; 3],
    mesh_volume_cm3: Option<f32>,
    profile: &PrinterProfile,
) -> PrintEstimate {
    let [x, y, z] = dimensions;
    let bbox_cm3 = x.max(0.0) * y.max(0.0) * z.max(0.0) / 1000.0;
    let part_volume_cm3 = mesh_volume_cm3
        .filter(|volume| volume.is_finite() && *volume > 0.0)
        .unwrap_or(bbox_cm3 * 0.12);
    let grams = part_volume_cm3 * 1.24 * 1.08;
    let minutes = (grams / profile.grams_per_minute).max(6.0).ceil() as u32;
    let hours = minutes / 60;
    let mins = minutes % 60;
    PrintEstimate {
        time_label: if hours > 0 {
            format!("{}h {}m", hours, mins)
        } else {
            format!("{}m", mins)
        },
        grams_label: format!("{}g", grams.ceil() as u32),
        layers_label: format!("{}", (z / profile.layer_height_mm).ceil().max(1.0) as u32),
        bed_fit: x <= profile.build_volume[0]
            && y <= profile.build_volume[1]
            && z <= profile.build_volume[2],
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DiscoveredSlicer {
    label: String,
    path: PathBuf,
    detail: String,
    icon_path: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SlicerChoiceRow {
    label: String,
    path: String,
    detail: String,
    icon_path: Option<PathBuf>,
    selected: bool,
}

fn slicer_status_text(path: &str) -> String {
    if path.trim().is_empty() {
        "Slicer set to system default STL opener".to_string()
    } else {
        format!("Slicer set to {}", display_slicer_path(path))
    }
}

fn slicer_label_for_path(path: &str, rows: &[SlicerChoiceRow]) -> String {
    rows.iter()
        .find(|row| row.path == path.trim())
        .map(|row| row.label.clone())
        .unwrap_or_else(|| {
            if path.trim().is_empty() {
                "System default STL opener".to_string()
            } else {
                display_slicer_path(path)
            }
        })
}

fn slicer_choice_rows(
    selected_path: &str,
    discovered: &[DiscoveredSlicer],
) -> Vec<SlicerChoiceRow> {
    let selected = selected_path.trim();
    let mut rows = Vec::with_capacity(discovered.len() + 2);
    rows.push(SlicerChoiceRow {
        label: "System default STL opener".to_string(),
        path: String::new(),
        detail: default_slicer_detail(),
        icon_path: None,
        selected: selected.is_empty(),
    });

    for slicer in discovered {
        let path = slicer.path.display().to_string();
        rows.push(SlicerChoiceRow {
            label: slicer.label.clone(),
            path: path.clone(),
            detail: slicer.detail.clone(),
            icon_path: slicer.icon_path.clone(),
            selected: selected == path,
        });
    }

    if !selected.is_empty() && !rows.iter().any(|row| row.path == selected) {
        let manual_path = PathBuf::from(selected);
        rows.push(SlicerChoiceRow {
            label: display_slicer_path(selected),
            path: selected.to_string(),
            detail: "Manual selection".to_string(),
            icon_path: slicer_icon_png_path(&manual_path),
            selected: true,
        });
    }

    rows
}

fn display_slicer_path(path: &str) -> String {
    let trimmed = path.trim();
    Path::new(trimmed)
        .file_stem()
        .or_else(|| Path::new(trimmed).file_name())
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(trimmed)
        .to_string()
}

fn sort_key(sort_by: SortBy) -> &'static str {
    match sort_by {
        SortBy::Name => "name",
        SortBy::Modified => "modified",
        SortBy::Added => "added",
        SortBy::Format => "format",
        SortBy::Size => "size",
        SortBy::Triangles => "triangles",
        SortBy::Dimensions => "dimensions",
        SortBy::Volume => "volume",
    }
}

fn sort_by_from_key(key: &str) -> SortBy {
    match key {
        "date" | "modified" => SortBy::Modified,
        "added" => SortBy::Added,
        "format" => SortBy::Format,
        "size" => SortBy::Size,
        "triangles" => SortBy::Triangles,
        "dimensions" => SortBy::Dimensions,
        "volume" => SortBy::Volume,
        _ => SortBy::Name,
    }
}

#[cfg(target_os = "macos")]
fn default_slicer_detail() -> String {
    "Uses macOS default app for .stl files".to_string()
}

#[cfg(target_os = "windows")]
fn default_slicer_detail() -> String {
    "Uses Windows default app for .stl files".to_string()
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn default_slicer_detail() -> String {
    "Uses xdg-open default app for .stl files".to_string()
}

fn discover_slicer_candidates() -> Vec<DiscoveredSlicer> {
    #[cfg(target_os = "macos")]
    {
        let mut roots = vec![PathBuf::from("/Applications")];
        if let Some(home) = std::env::var_os("HOME") {
            roots.push(PathBuf::from(home).join("Applications"));
        }
        return discover_macos_slicer_candidates_in_roots(roots);
    }

    #[cfg(target_os = "windows")]
    {
        discover_windows_slicer_candidates()
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        discover_unix_slicer_candidates()
    }
}

fn push_unique_slicer(out: &mut Vec<DiscoveredSlicer>, label: &str, path: PathBuf, detail: &str) {
    if path.exists() && !out.iter().any(|candidate| candidate.path == path) {
        let icon_path = slicer_icon_png_path(&path);
        out.push(DiscoveredSlicer {
            label: label.to_string(),
            path,
            detail: detail.to_string(),
            icon_path,
        });
    }
}

#[cfg(target_os = "macos")]
fn slicer_icon_png_path(app: &Path) -> Option<PathBuf> {
    let is_app_bundle = app
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("app"));
    if !is_app_bundle {
        return None;
    }

    let icns = macos_app_icon_icns(app)?;
    let cache_dir = platform_cache_root().join("slicer-icons-v1");
    let hash = blake3::hash(app.to_string_lossy().as_bytes())
        .to_hex()
        .to_string();
    let output = cache_dir.join(format!("{hash}.png"));
    if output.exists() {
        return Some(output);
    }

    fs::create_dir_all(&cache_dir).ok()?;
    let tmp_output = cache_dir.join(format!("{hash}.tmp.png"));
    let status = Command::new("sips")
        .args(["-s", "format", "png"])
        .arg(&icns)
        .arg("--out")
        .arg(&tmp_output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()?;
    if !status.success() || !tmp_output.exists() {
        let _ = fs::remove_file(&tmp_output);
        return None;
    }

    fs::rename(&tmp_output, &output).ok()?;
    Some(output)
}

#[cfg(not(target_os = "macos"))]
fn slicer_icon_png_path(_path: &Path) -> Option<PathBuf> {
    None
}

#[cfg(target_os = "macos")]
fn macos_app_icon_icns(app: &Path) -> Option<PathBuf> {
    let resources = app.join("Contents").join("Resources");
    let mut icons = fs::read_dir(&resources)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("icns"))
        })
        .collect::<Vec<_>>();
    if icons.is_empty() {
        return None;
    }

    let app_key = app
        .file_stem()
        .and_then(|name| name.to_str())
        .map(compact_ascii_key)
        .unwrap_or_default();
    icons.sort_by_key(|path| {
        std::cmp::Reverse(
            path.file_stem()
                .and_then(|name| name.to_str())
                .map(|stem| macos_icon_score(stem, &app_key))
                .unwrap_or(0),
        )
    });
    icons.into_iter().next()
}

#[cfg(target_os = "macos")]
fn macos_icon_score(stem: &str, app_key: &str) -> i32 {
    let key = compact_ascii_key(stem);
    if key == "icon" {
        100
    } else if !app_key.is_empty() && key == app_key {
        90
    } else if !app_key.is_empty() && (key.contains(app_key) || app_key.contains(&key)) {
        80
    } else if key.contains("appicon") {
        70
    } else if key.contains("icon") {
        60
    } else {
        0
    }
}

#[cfg(target_os = "macos")]
fn compact_ascii_key(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn platform_cache_root() -> PathBuf {
    if let Ok(path) = std::env::var("MODELRACK_CACHE_DIR") {
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Caches")
                .join("ModelRack");
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local_app_data)
                .join("ModelRack")
                .join("Cache");
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Some(cache_home) = std::env::var_os("XDG_CACHE_HOME") {
            return PathBuf::from(cache_home).join("modelrack");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".cache").join("modelrack");
        }
    }

    std::env::temp_dir().join("modelrack-cache")
}

#[cfg(target_os = "macos")]
fn discover_macos_slicer_candidates_in_roots<I>(roots: I) -> Vec<DiscoveredSlicer>
where
    I: IntoIterator<Item = PathBuf>,
{
    let mut out = Vec::new();
    for root in roots {
        for app in macos_app_bundles(&root, 2) {
            if let Some(label) = macos_slicer_label(&app) {
                push_unique_slicer(&mut out, &label, app, "Detected installed macOS app");
            }
        }
    }
    out.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
    out
}

#[cfg(target_os = "macos")]
fn macos_app_bundles(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    fn visit(dir: &Path, depth: usize, max_depth: usize, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let is_app = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("app"));
            if is_app {
                out.push(path);
            } else if depth < max_depth {
                visit(&path, depth + 1, max_depth, out);
            }
        }
    }

    let mut out = Vec::new();
    visit(root, 0, max_depth, &mut out);
    out
}

#[cfg(target_os = "macos")]
fn macos_slicer_label(app: &Path) -> Option<String> {
    let stem = app.file_stem()?.to_str()?.trim();
    let compact = stem
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect::<String>();
    let label = match compact.as_str() {
        "bambustudio" => "Bambu Studio",
        "orcaslicer" => "OrcaSlicer",
        "prusaslicer" => "PrusaSlicer",
        "ultimakercura" => "UltiMaker Cura",
        "cura" => "Cura",
        "superslicer" => "SuperSlicer",
        "ideamaker" => "ideaMaker",
        "snapmakerorca" => "Snapmaker Orca",
        "crealityprint" => "Creality Print",
        "chitubox" => "CHITUBOX",
        "lycheeslicer" => "Lychee Slicer",
        "simplify3d" => "Simplify3D",
        "flashprint" => "FlashPrint",
        "mattercontrol" => "MatterControl",
        "anycubicslicer" => "Anycubic Slicer",
        "qidislicer" => "QIDI Slicer",
        "raise3dideamaker" => "ideaMaker",
        _ if compact.contains("slicer") => stem,
        _ if compact.contains("creality") && compact.contains("print") => stem,
        _ if compact.contains("snapmaker") && compact.contains("orca") => stem,
        _ if compact.contains("bambu") && compact.contains("studio") => stem,
        _ if compact.contains("orca") && !compact.contains("chrome") => stem,
        _ if compact.contains("prusa") => stem,
        _ if compact.contains("cura") => stem,
        _ if compact.contains("chitubox") => stem,
        _ if compact.contains("lychee") => stem,
        _ if compact.contains("simplify3d") => stem,
        _ if compact.contains("flashprint") => stem,
        _ => return None,
    };
    Some(label.to_string())
}

#[cfg(target_os = "windows")]
fn discover_windows_slicer_candidates() -> Vec<DiscoveredSlicer> {
    let roots = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
        .iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let candidates = [
        ("OrcaSlicer", ["OrcaSlicer", "orca-slicer.exe"]),
        ("Bambu Studio", ["Bambu Studio", "bambu-studio.exe"]),
        ("PrusaSlicer", ["Prusa3D\\PrusaSlicer", "prusa-slicer.exe"]),
        ("UltiMaker Cura", ["UltiMaker Cura", "UltiMaker-Cura.exe"]),
        ("SuperSlicer", ["SuperSlicer", "superslicer.exe"]),
    ];

    let mut out = Vec::new();
    for root in roots {
        for (label, parts) in candidates {
            push_unique_slicer(
                &mut out,
                label,
                root.join(parts[0]).join(parts[1]),
                "Detected Windows executable",
            );
        }
    }
    out
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn discover_unix_slicer_candidates() -> Vec<DiscoveredSlicer> {
    let candidates = [
        ("OrcaSlicer", "orca-slicer"),
        ("Bambu Studio", "bambu-studio"),
        ("PrusaSlicer", "prusa-slicer"),
        ("UltiMaker Cura", "cura"),
        ("SuperSlicer", "superslicer"),
    ];
    let path_dirs = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .unwrap_or_default();
    let mut out = Vec::new();
    for dir in path_dirs {
        for (label, binary) in candidates {
            push_unique_slicer(
                &mut out,
                label,
                dir.join(binary),
                "Detected executable on PATH",
            );
        }
    }
    out
}

fn language_label(language: &str) -> &'static str {
    match language {
        "ko" => "한국어",
        "ja" => "日本語",
        _ => "English",
    }
}

fn theme_label(theme: &str) -> &'static str {
    if theme == "light" {
        "Light"
    } else {
        "Dark"
    }
}

fn scan_folder_entries(
    folder: &Path,
    generation: u64,
    total: usize,
    tx: &mpsc::Sender<ScanMessage>,
) -> (Vec<scanner::StlFileInfo>, usize) {
    let (scan_tx, rx) = crossbeam_channel::unbounded();
    let scan_folder = folder.to_path_buf();
    let scanner_thread = thread::spawn(move || scanner::scan_folder_stream(&scan_folder, scan_tx));

    let mut entries = Vec::new();
    let mut batch = Vec::new();
    let mut skipped = 0usize;
    let mut last_flush = Instant::now();
    let mut last_walk_scanned = 0usize;
    let mut last_walk_skipped = 0usize;
    for event in rx {
        match event {
            scanner::ScanEvent::Progress {
                scanned,
                skipped,
                current,
            } => {
                last_walk_scanned = scanned;
                last_walk_skipped = skipped;
                let _ = tx.send(ScanMessage::Progress(ScanProgress {
                    generation,
                    folder: folder.to_path_buf(),
                    found: entries.len(),
                    scanned,
                    total,
                    skipped,
                    current,
                }));
            }
            scanner::ScanEvent::Entry { mut info, mesh } => {
                let filename_for_status = info.filename.clone();
                info.thumbnail_path = crate::thumbnail_cache::thumbnail_path_if_cached(&info);
                if info.thumbnail_path.is_none()
                    && entries.len() < SCAN_INLINE_THUMBNAIL_MAX_LIBRARY_ENTRIES
                {
                    info.thumbnail_path =
                        crate::thumbnail_cache::ensure_thumbnail(&info, mesh.as_ref()).ok();
                }
                entries.push((*info).clone());
                batch.push(*info);
                if batch.len() >= SCAN_ENTRY_BATCH_SIZE
                    || entries.len() == 1
                    || last_flush.elapsed() >= SCAN_ENTRY_BATCH_INTERVAL
                {
                    flush_scan_entry_batch(tx, generation, folder, &mut batch);
                    last_flush = Instant::now();
                }
                let _ = tx.send(ScanMessage::Progress(ScanProgress {
                    generation,
                    folder: folder.to_path_buf(),
                    found: entries.len(),
                    scanned: last_walk_scanned,
                    total,
                    skipped: last_walk_skipped,
                    current: format!("{} · thumbnails · 썸네일", filename_for_status),
                }));
            }
            scanner::ScanEvent::Done {
                skipped: done_skipped,
            } => {
                skipped = done_skipped;
                break;
            }
        }
    }

    let _ = scanner_thread.join();
    flush_scan_entry_batch(tx, generation, folder, &mut batch);
    entries.sort_by(|a, b| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()));
    (entries, skipped)
}

fn flush_scan_entry_batch(
    tx: &mpsc::Sender<ScanMessage>,
    generation: u64,
    folder: &Path,
    batch: &mut Vec<scanner::StlFileInfo>,
) {
    if batch.is_empty() {
        return;
    }

    let entries = std::mem::take(batch);
    let _ = tx.send(ScanMessage::EntryBatch(ScanEntryBatch {
        generation,
        folder: folder.to_path_buf(),
        entries,
    }));
}

fn count_supported_model_files(folder: &Path) -> usize {
    walkdir::WalkDir::new(folder)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| is_supported_model_path(entry.path()))
        .count()
}

fn is_supported_model_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "stl" | "3mf" | "obj" | "step" | "stp" | "scad"
            )
        })
        .unwrap_or(false)
}

fn browser_card(card: &BrowserCardVm) -> BrowserCard {
    let (thumb_image, thumb_ready) = load_thumbnail_image(card.thumb_path.as_deref());
    BrowserCard {
        stable_key: card.stable_key.clone().into(),
        slot_index: card.slot_index as i32,
        title: card.title.clone().into(),
        subtitle: card.subtitle.clone().into(),
        author: card.author.clone().into(),
        relative_modified: card.relative_modified.clone().into(),
        thumb_key: card.thumb_key.clone().into(),
        thumb_image,
        thumb_ready,
        badge: card.badge.clone().into(),
        printed_count: card.printed_count as i32,
        favorite: card.favorite,
        printed: card.printed,
        error: card.error,
    }
}

fn load_thumbnail_image(path: Option<&Path>) -> (slint::Image, bool) {
    load_ui_image(path)
}

fn load_ui_image(path: Option<&Path>) -> (slint::Image, bool) {
    let Some(path) = path else {
        return (slint::Image::default(), false);
    };
    match slint::Image::load_from_path(path) {
        Ok(image) => (image, true),
        Err(err) => {
            eprintln!(
                "Warning: failed to load image {}: {:?}",
                path.display(),
                err
            );
            (slint::Image::default(), false)
        }
    }
}

fn render_detail_preview_image(
    entry: &scanner::StlFileInfo,
    mesh: &scanner::MeshData,
    yaw: f32,
    pitch: f32,
    quality: DetailPreviewQuality,
) -> (slint::Image, bool) {
    let (width, height, face_budget) = match quality {
        DetailPreviewQuality::High => (640, 498, 80_000),
        DetailPreviewQuality::Interactive => (360, 280, 18_000),
    };
    let pixels = crate::thumbnail_cache::render_preview_rgba_with_face_budget(
        entry,
        Some(mesh),
        width,
        height,
        yaw,
        pitch,
        face_budget,
    );
    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&pixels, width, height);
    (slint::Image::from_rgba8(buffer), true)
}

fn tag_chip(tag: &String) -> TagChip {
    TagChip {
        label: tag.clone().into(),
    }
}

fn print_history_row(record: &scanner::PrintRecord) -> PrintHistoryRow {
    PrintHistoryRow {
        date: record.date.clone().into(),
        material: record.material.clone().into(),
        printer: record.printer.clone().into(),
        profile: record.profile.clone().into(),
        nozzle: record.nozzle.clone().into(),
        layer_height: record.layer_height.clone().into(),
        duration: record.duration.clone().into(),
        notes: record.notes.clone().into(),
        success: record.success,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let unique = format!(
            "modelrack-{}-{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        std::env::temp_dir().join(unique)
    }

    fn test_entry(path: &Path) -> scanner::StlFileInfo {
        scanner::StlFileInfo {
            path: path.to_path_buf(),
            filename: path.file_name().unwrap().to_string_lossy().to_string(),
            size: 1,
            hash: [1; 32],
            stl_type: scanner::StlType::Binary,
            triangle_count: Some(1),
            dimensions: Some([1.0, 1.0, 1.0]),
            three_mf_plate_count: None,
            modified: None,
            thumbnail_path: None,
            meta: None,
        }
    }

    fn prefs_with_root(root: &Path) -> AppPrefs {
        AppPrefs {
            library_folders: vec![root.to_path_buf()],
            ..AppPrefs::default()
        }
    }

    #[test]
    fn restored_library_scan_queue_includes_every_configured_root_dir() {
        let root = temp_path("mr-two-lib-roots");
        let a = root.join("one");
        let b = root.join("two");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let prefs = AppPrefs {
            library_folders: vec![a.clone(), b.clone()],
            ..Default::default()
        };
        let state = ShellState::with_prefs(prefs);
        assert_eq!(state.restored_library_scan_queue(), vec![a, b]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn pending_scan_queue_drains_every_remaining_root() {
        let root = temp_path("mr-pending-roots");
        let first = root.join("one");
        let second = root.join("two");
        let third = root.join("three");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        std::fs::create_dir_all(&third).unwrap();
        let mut state = ShellState::with_prefs(AppPrefs {
            library_folders: vec![first.clone(), second.clone(), third.clone()],
            ..Default::default()
        });

        state.replace_pending_scan_queue(vec![second.clone(), third.clone()]);

        assert_eq!(state.pop_next_pending_scan_root(), Some(second));
        assert_eq!(state.pop_next_pending_scan_root(), Some(third));
        assert_eq!(state.pop_next_pending_scan_root(), None);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn scan_root_for_sidebar_folder_uses_containing_library_root() {
        let root = temp_path("mr-sidebar-rescan-root");
        let downloads = root.join("Downloads");
        let models = root.join("3D Files");
        let nested = models.join("parts");
        std::fs::create_dir_all(&downloads).unwrap();
        std::fs::create_dir_all(&nested).unwrap();
        let state = ShellState::with_prefs(AppPrefs {
            last_folder: Some(downloads.clone()),
            library_folders: vec![downloads, models.clone()],
            ..Default::default()
        });

        assert_eq!(state.scan_root_for_folder(&nested), Some(models));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn watcher_relevance_includes_models_and_sidecars_only() {
        assert!(is_refresh_relevant_path(Path::new("part.stl")));
        assert!(is_refresh_relevant_path(Path::new("assembly.3mf")));
        assert!(is_refresh_relevant_path(Path::new("mount.obj")));
        assert!(is_refresh_relevant_path(Path::new("bracket.step")));
        assert!(is_refresh_relevant_path(Path::new("bracket.stp")));
        assert!(is_refresh_relevant_path(Path::new("fixture.scad")));
        assert!(is_refresh_relevant_path(Path::new(
            "part.stl.modelrack.json"
        )));

        assert!(!is_refresh_relevant_path(Path::new("notes.json")));
        assert!(!is_refresh_relevant_path(Path::new("render.png")));
        assert!(!is_refresh_relevant_path(Path::new("README.md")));
    }

    #[test]
    fn watcher_debounce_coalesces_relevant_bursts() {
        let mut debounce = WatchDebounce::default();
        let start = Instant::now();
        debounce.record(start);
        debounce.record(start + Duration::from_millis(100));

        assert!(
            !debounce.consume_if_due(start + Duration::from_millis(749), LIBRARY_WATCH_DEBOUNCE)
        );
        assert!(
            !debounce.consume_if_due(start + Duration::from_millis(849), LIBRARY_WATCH_DEBOUNCE)
        );
        assert!(debounce.consume_if_due(start + Duration::from_millis(850), LIBRARY_WATCH_DEBOUNCE));
        assert!(
            !debounce.consume_if_due(start + Duration::from_millis(1500), LIBRARY_WATCH_DEBOUNCE)
        );
    }

    #[test]
    fn watcher_runtime_ignores_unrelated_events_and_debounces_relevant_events() {
        let mut runtime = LibraryWatcherRuntime::new();
        let start = Instant::now();
        let root = temp_path("watcher-synthetic");
        fs::create_dir_all(&root).unwrap();
        runtime.watch_folder(&root).unwrap();
        let generation = runtime.generation;

        runtime
            .tx
            .send(WatchMessage::Changed {
                generation,
                paths: vec![PathBuf::from("notes.txt")],
            })
            .unwrap();
        assert_eq!(
            runtime.poll(start + Duration::from_millis(1000), LIBRARY_WATCH_DEBOUNCE),
            WatchPoll::default()
        );

        runtime
            .tx
            .send(WatchMessage::Changed {
                generation,
                paths: vec![PathBuf::from("part.stl.modelrack.json")],
            })
            .unwrap();
        assert_eq!(
            runtime.poll(start, LIBRARY_WATCH_DEBOUNCE),
            WatchPoll::default()
        );
        assert_eq!(
            runtime.poll(start + Duration::from_millis(700), LIBRARY_WATCH_DEBOUNCE),
            WatchPoll::default()
        );
        assert_eq!(
            runtime.poll(start + Duration::from_millis(800), LIBRARY_WATCH_DEBOUNCE),
            WatchPoll {
                refresh_due: true,
                error: None
            }
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn watcher_runtime_surfaces_notify_errors_without_refreshing() {
        let mut runtime = LibraryWatcherRuntime::new();
        let start = Instant::now();
        let root = temp_path("watcher-error-recreate");
        fs::create_dir_all(&root).unwrap();
        runtime.watch_folder(&root).unwrap();
        assert!(runtime.watcher.is_some());
        assert_eq!(runtime.watched_folder.as_deref(), Some(root.as_path()));

        runtime
            .tx
            .send(WatchMessage::Error {
                generation: runtime.generation,
                message: "backend disconnected".to_string(),
            })
            .unwrap();

        assert_eq!(
            runtime.poll(start, LIBRARY_WATCH_DEBOUNCE),
            WatchPoll {
                refresh_due: false,
                error: Some("backend disconnected".to_string())
            }
        );
        assert!(runtime.watcher.is_none());
        assert!(runtime.watched_folder.is_none());

        runtime.watch_folder(&root).unwrap();
        assert!(runtime.watcher.is_some());
        assert_eq!(runtime.watched_folder.as_deref(), Some(root.as_path()));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn watcher_runtime_resets_pending_messages_when_folder_changes() {
        let mut runtime = LibraryWatcherRuntime::new();
        let start = Instant::now();
        let first = temp_path("watcher-first");
        let second = temp_path("watcher-second");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();

        runtime.watch_folder(&first).unwrap();
        let first_generation = runtime.generation;
        runtime
            .tx
            .send(WatchMessage::Changed {
                generation: first_generation,
                paths: vec![first.join("part.stl")],
            })
            .unwrap();
        runtime.watch_folder(&second).unwrap();

        assert_eq!(
            runtime.poll(start + Duration::from_millis(1000), LIBRARY_WATCH_DEBOUNCE),
            WatchPoll::default()
        );
        let _ = fs::remove_dir_all(first);
        let _ = fs::remove_dir_all(second);
    }

    #[test]
    fn watcher_runtime_ignores_late_messages_from_old_generation() {
        let mut runtime = LibraryWatcherRuntime::new();
        let start = Instant::now();
        let first = temp_path("watcher-old-gen");
        let second = temp_path("watcher-new-gen");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();

        runtime.watch_folder(&first).unwrap();
        let old_generation = runtime.generation;
        runtime.watch_folder(&second).unwrap();

        runtime
            .tx
            .send(WatchMessage::Changed {
                generation: old_generation,
                paths: vec![first.join("part.stl")],
            })
            .unwrap();
        runtime
            .tx
            .send(WatchMessage::Error {
                generation: old_generation,
                message: "old watcher failed late".to_string(),
            })
            .unwrap();

        assert_eq!(
            runtime.poll(start + Duration::from_millis(1000), LIBRARY_WATCH_DEBOUNCE),
            WatchPoll::default()
        );
        assert!(runtime.watcher.is_some());
        assert_eq!(runtime.watched_folder.as_deref(), Some(second.as_path()));
        let _ = fs::remove_dir_all(first);
        let _ = fs::remove_dir_all(second);
    }

    #[test]
    fn watcher_runtime_observes_real_notify_event_for_model_file() {
        let mut runtime = LibraryWatcherRuntime::new();
        let root = temp_path("watcher-real-event");
        fs::create_dir_all(&root).unwrap();
        runtime.watch_folder(&root).unwrap();

        fs::write(root.join("part.stl"), b"solid test\nendsolid test\n").unwrap();
        let deadline = Instant::now() + Duration::from_secs(4);
        let mut observed = false;
        while Instant::now() < deadline {
            if runtime.poll(Instant::now(), Duration::ZERO).refresh_due {
                observed = true;
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        let _ = fs::remove_dir_all(root);
        assert!(
            observed,
            "notify did not deliver a model file event before timeout"
        );
    }

    #[test]
    fn library_scan_runtime_returns_background_scan_result() {
        let mut runtime = LibraryScanRuntime::new();
        let root = temp_path("async-scan");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("part.stl"), b"solid test\nendsolid test\n").unwrap();

        assert_eq!(runtime.request_scan(&root), ScanRequest::Started);
        let deadline = Instant::now() + Duration::from_secs(4);
        let mut result = None;
        while Instant::now() < deadline {
            result = runtime.poll().result;
            if result.is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }

        let result = result.expect("background scan did not finish before timeout");
        assert_eq!(result.folder, root);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].filename, "part.stl");
        let _ = fs::remove_dir_all(result.folder);
    }

    #[test]
    fn scan_folder_entries_emits_partial_entry_batches() {
        let root = temp_path("streaming-scan-batches");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.stl"), b"solid a\nendsolid a\n").unwrap();
        fs::write(root.join("b.stl"), b"solid b\nendsolid b\n").unwrap();
        let (tx, rx) = mpsc::channel();

        let (entries, _skipped) = scan_folder_entries(&root, 7, 2, &tx);

        assert_eq!(entries.len(), 2);
        let mut batch_count = 0;
        let mut batched_entries = 0;
        while let Ok(message) = rx.try_recv() {
            if let ScanMessage::EntryBatch(batch) = message {
                assert_eq!(batch.generation, 7);
                assert_eq!(batch.folder, root);
                batch_count += 1;
                batched_entries += batch.entries.len();
            }
        }

        assert!(batch_count > 0, "scan should emit incremental batches");
        assert_eq!(batched_entries, 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn library_scan_runtime_ignores_stale_generations() {
        let mut runtime = LibraryScanRuntime::new();
        runtime.active_generation = Some(2);
        runtime.active_folder = Some(PathBuf::from("/tmp/new"));
        runtime
            .tx
            .send(ScanMessage::Result(ScanResult {
                generation: 1,
                folder: PathBuf::from("/tmp/old"),
                entries: Vec::new(),
                skipped: 0,
            }))
            .unwrap();
        runtime
            .tx
            .send(ScanMessage::Result(ScanResult {
                generation: 2,
                folder: PathBuf::from("/tmp/new"),
                entries: Vec::new(),
                skipped: 0,
            }))
            .unwrap();

        let result = runtime
            .poll()
            .result
            .expect("latest generation should be returned");
        assert_eq!(result.generation, 2);
        assert_eq!(result.folder, PathBuf::from("/tmp/new"));
        assert_eq!(runtime.active_generation, None);
        assert_eq!(runtime.active_folder, None);
    }

    #[test]
    fn library_scan_runtime_invalidates_inflight_scan_on_folder_switch() {
        let mut runtime = LibraryScanRuntime::new();
        assert_eq!(
            runtime.request_scan(Path::new("/tmp/old-library")),
            ScanRequest::Started
        );
        runtime.invalidate();
        runtime
            .tx
            .send(ScanMessage::Result(ScanResult {
                generation: 1,
                folder: PathBuf::from("/tmp/old-library"),
                entries: Vec::new(),
                skipped: 0,
            }))
            .unwrap();

        assert!(runtime.poll().result.is_none());
        assert_eq!(runtime.active_generation, None);
        assert_eq!(runtime.active_folder, None);
    }

    #[test]
    fn library_scan_runtime_coalesces_duplicate_active_folder_requests() {
        let mut runtime = LibraryScanRuntime::new();
        let folder = Path::new("/tmp/current-library");

        assert_eq!(runtime.request_scan(folder), ScanRequest::Started);
        assert_eq!(runtime.request_scan(folder), ScanRequest::AlreadyRunning);
        assert_eq!(runtime.next_generation, 1);
    }

    #[test]
    fn demo_fallback_has_no_active_real_folder_for_refresh_or_watch() {
        let mut state = ShellState::with_prefs(AppPrefs::default());
        let snapshot = state.snapshot_idle();

        assert_eq!(state.current_folder, None);
        assert!(!state.sidecar_writes_enabled);
        assert_eq!(state.active_real_folder(), None);
        assert_eq!(snapshot.library_label, "Sample library");
        assert!(snapshot.folders.is_empty());
        assert!(!snapshot.library_label.contains(DEMO_ROOT));
        assert_eq!(
            settings_folder_label(&state),
            "Sample library (demo, memory-only)"
        );
        assert!(detail_parent_label(&state.entries[0], &state).starts_with("Sample library/"));
    }

    #[test]
    fn restored_real_folder_has_active_real_folder_and_watcher_intent() {
        let root = temp_path("watcher-restored-folder");
        let models = root.join("models");
        fs::create_dir_all(&models).unwrap();
        fs::write(models.join("part.stl"), b"solid test\nendsolid test\n").unwrap();

        let prefs = AppPrefs {
            last_folder: Some(models.clone()),
            ..AppPrefs::default()
        };
        let path = root.join("prefs.json");
        save_app_prefs_to_path(&path, &prefs).unwrap();

        let mut state = ShellState::load_from_path(&path);
        assert_eq!(state.restored_real_folder_candidate(), Some(models.clone()));
        assert_eq!(state.active_real_folder(), None);

        let snapshot = state.begin_folder_scan(&models, "Restoring last library");
        assert_eq!(snapshot.library_label, models.display().to_string());
        assert!(snapshot.cards.is_empty());
        assert!(snapshot.status_text.contains("Restoring last library"));
        assert_eq!(state.active_real_folder(), Some(models.clone()));

        let mut runtime = LibraryWatcherRuntime::new();
        runtime.watch_folder(&models).unwrap();
        assert_eq!(runtime.watched_folder.as_deref(), Some(models.as_path()));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn excluded_folders_filter_scan_results_and_can_be_restored() {
        let root = PathBuf::from("/tmp/modelrack-library");
        let archived = root.join("archived");
        let visible = root.join("visible.stl");
        let hidden = archived.join("hidden.stl");
        let entries = vec![test_entry(&visible), test_entry(&hidden)];

        let mut state = ShellState::with_prefs(AppPrefs::default());
        state.add_excluded_folder(&archived);
        let snapshot = state.apply_scan_parts(root.clone(), entries.clone(), 0);

        assert_eq!(state.entries.len(), 1);
        assert_eq!(state.entries[0].path, visible);
        assert_eq!(snapshot.browser.total, 1);
        assert_eq!(state.prefs.excluded_folders, vec![archived.clone()]);

        state.remove_excluded_folder(&archived);
        let snapshot = state.apply_scan_parts(root, entries, 0);

        assert_eq!(state.entries.len(), 2);
        assert_eq!(snapshot.browser.total, 2);
        assert!(state.prefs.excluded_folders.is_empty());
    }

    #[test]
    fn first_scan_entry_batch_replaces_stale_entries() {
        let root = PathBuf::from("/tmp/modelrack-library");
        let old = root.join("old.stl");
        let fresh = root.join("fresh.stl");
        let mut state = ShellState::with_prefs(AppPrefs::default());
        state.entries = vec![test_entry(&old)];
        state.current_folder = Some(root.clone());
        state.sidecar_writes_enabled = true;

        let snapshot = state
            .apply_scan_entry_batches(
                vec![ScanEntryBatch {
                    generation: 42,
                    folder: root,
                    entries: vec![test_entry(&fresh)],
                }],
                None,
            )
            .expect("first streaming batch should update the snapshot");

        assert_eq!(state.entries.len(), 1);
        assert_eq!(state.entries[0].path, fresh);
        assert_eq!(snapshot.browser.total, 1);
        assert!(snapshot.status_text.contains("found 1"));
    }

    #[test]
    fn excluded_last_folder_is_not_restored_on_startup() {
        let root = temp_path("excluded-restored-folder");
        let models = root.join("models");
        fs::create_dir_all(&models).unwrap();

        let prefs = AppPrefs {
            last_folder: Some(models.clone()),
            excluded_folders: vec![models.clone()],
            ..AppPrefs::default()
        };
        let state = ShellState::with_prefs(prefs);

        assert_eq!(state.restored_real_folder_candidate(), None);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn app_prefs_save_and_load_from_explicit_path() {
        let root = temp_path("prefs-roundtrip");
        let path = root.join("prefs.json");
        let prefs = AppPrefs {
            density: "large".to_string(),
            view_mode: "masonry".to_string(),
            theme: "light".to_string(),
            accent_color: "orange".to_string(),
            language: "ko".to_string(),
            slicer_path: "/Applications/PrusaSlicer.app".to_string(),
            sort_by: "size".to_string(),
            sort_ascending: false,
            thumbnail_style: "normal".to_string(),
            thumbnail_lighting: "even".to_string(),
            thumbnail_aa: "msaa2x".to_string(),
            active_printer_keys: vec!["bambu-p1s-0.4".to_string()],
            default_printer_key: "bambu-p1s-0.4".to_string(),
            card_label_mode: "titled".to_string(),
            date_format_mode: "us".to_string(),
            show_file_extensions: false,
            startup_view: "empty".to_string(),
            last_folder: Some(root.join("models")),
            library_folders: vec![root.join("models")],
            excluded_folders: vec![root.join("models/archived")],
            collapsed_folders: vec![root.join("models/nested")],
        };

        save_app_prefs_to_path(&path, &prefs).unwrap();
        let loaded = load_app_prefs_from_path(&path).unwrap();

        assert_eq!(loaded, prefs);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn app_prefs_missing_or_invalid_falls_back_for_shell_startup() {
        let root = temp_path("prefs-invalid");
        let missing = root.join("missing.json");
        let invalid = root.join("invalid.json");
        fs::create_dir_all(&root).unwrap();
        fs::write(&invalid, "{not json").unwrap();

        assert_eq!(
            load_app_prefs_from_path(&missing).unwrap(),
            AppPrefs::default()
        );
        assert!(load_app_prefs_from_path(&invalid).is_err());
        assert_eq!(
            ShellState::load_from_path(&invalid).prefs,
            AppPrefs::default()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn accent_key_is_normalized_in_shell_state() {
        let mut prefs = AppPrefs {
            accent_color: "unknown".to_string(),
            ..AppPrefs::default()
        };
        let state = ShellState::with_prefs(prefs.clone());
        assert_eq!(state.prefs.accent_color, "teal");

        prefs.accent_color = "purple".to_string();
        let mut state = ShellState::with_prefs(prefs);
        assert_eq!(state.prefs.accent_color, "purple");

        state.choose_accent("green");
        assert_eq!(state.prefs.accent_color, "green");
        state.choose_accent("not-a-color");
        assert_eq!(state.prefs.accent_color, "teal");
    }

    #[test]
    fn slicer_choice_rows_preserve_system_default_and_manual_selection() {
        let manual = "/opt/slicers/custom-slicer";
        let rows = slicer_choice_rows(manual, &[]);

        assert_eq!(rows[0].label, "System default STL opener");
        assert_eq!(rows[0].path, "");
        assert!(!rows[0].selected);
        assert_eq!(rows[1].label, "custom-slicer");
        assert_eq!(rows[1].path, manual);
        assert_eq!(rows[1].detail, "Manual selection");
        assert!(rows[1].selected);
    }

    #[test]
    fn slicer_choice_rows_select_discovered_candidate() {
        let discovered = vec![DiscoveredSlicer {
            label: "OrcaSlicer".to_string(),
            path: PathBuf::from("/Applications/OrcaSlicer.app"),
            detail: "Detected macOS app bundle".to_string(),
            icon_path: None,
        }];
        let rows = slicer_choice_rows("/Applications/OrcaSlicer.app", &discovered);

        assert_eq!(rows.len(), 2);
        assert!(!rows[0].selected);
        assert_eq!(rows[1].label, "OrcaSlicer");
        assert!(rows[1].selected);
        assert_eq!(
            slicer_label_for_path("/Applications/OrcaSlicer.app", &rows),
            "OrcaSlicer"
        );
    }

    #[test]
    fn printer_prefs_keep_default_inside_active_selection() {
        let catalog = load_printer_profiles();
        let mut prefs = AppPrefs {
            active_printer_keys: vec![
                "unknown".to_string(),
                "prusa-mk4-0.4".to_string(),
                "bambu-p1s-0.4".to_string(),
                "prusa-mk4-0.4".to_string(),
            ],
            default_printer_key: "missing".to_string(),
            ..AppPrefs::default()
        };

        normalize_printer_prefs(&mut prefs, &catalog);

        assert_eq!(
            prefs.active_printer_keys,
            vec!["prusa-mk4-0.4".to_string(), "bambu-p1s-0.4".to_string()]
        );
        assert_eq!(prefs.default_printer_key, "prusa-mk4-0.4");
        assert_eq!(settings_printer_summary(&prefs, &catalog), "2 profiles");
    }

    #[test]
    fn settings_printer_selector_initializes_as_maker_model_nozzle_hierarchy() {
        let mut state = ShellState::with_prefs(AppPrefs::default());

        assert_eq!(state.settings_printer_maker, "Bambu");
        assert_eq!(state.settings_printer_model, "P1S");

        state.choose_settings_printer_maker("Prusa");
        assert_eq!(state.settings_printer_maker, "Prusa");
        assert_eq!(state.settings_printer_model, "MK4");

        // Choosing a nozzle stages it as the pending pick — explicit add required.
        state.choose_settings_printer_nozzle("0.4mm").unwrap();
        assert_eq!(state.settings_printer_nozzle, "0.4mm");
        assert!(state.can_add_pending_printer());

        // Adding promotes it into the active list and (since the user only had
        // the bundled default) keeps the default in sync.
        let added = state.add_pending_printer().unwrap();
        assert_eq!(added, "prusa-mk4-0.4");
        assert!(state
            .prefs
            .active_printer_keys
            .contains(&"prusa-mk4-0.4".to_string()));
        // Adding clears the pending nozzle so the UI button disables.
        assert!(state.settings_printer_nozzle.is_empty());
        assert!(!state.can_add_pending_printer());

        // Trying to add the same printer again is a no-op (Err returned).
        state.choose_settings_printer_nozzle("0.4mm").unwrap();
        assert!(state.add_pending_printer().is_err());
    }

    #[test]
    fn settings_printer_nozzle_picker_resolves_catalog_nozzle_diameters() {
        let mut state = ShellState::with_prefs(AppPrefs::default());
        state.choose_settings_printer_maker("Prusa");
        state.choose_settings_printer_model("MK4");
        state.choose_settings_printer_nozzle("0.2mm").unwrap();
        assert_eq!(
            state.pending_printer_profile().unwrap().key,
            "prusa-mk4-0.2"
        );
        state.choose_settings_printer_nozzle("0.6mm").unwrap();
        assert_eq!(
            state.pending_printer_profile().unwrap().key,
            "prusa-mk4-0.6"
        );
        state.choose_settings_printer_nozzle("0.8mm").unwrap();
        assert_eq!(
            state.pending_printer_profile().unwrap().key,
            "prusa-mk4-0.8"
        );
    }

    #[test]
    fn toggling_printers_never_removes_the_last_active_profile() {
        let mut state = ShellState::with_prefs(AppPrefs::default());

        let catalog = load_printer_profiles();

        assert!(state.toggle_printer_profile("bambu-p1s-0.4").is_err());
        state.toggle_printer_profile("prusa-mk4-0.4").unwrap();
        state.choose_default_printer("prusa-mk4-0.4");
        state.toggle_printer_profile("bambu-p1s-0.4").unwrap();

        assert_eq!(
            state.prefs.active_printer_keys,
            vec!["prusa-mk4-0.4".to_string()]
        );
        assert_eq!(state.prefs.default_printer_key, "prusa-mk4-0.4");
        assert_eq!(
            state.selected_estimate_profile(&catalog).key,
            "prusa-mk4-0.4"
        );
    }

    #[test]
    fn print_estimate_uses_selected_printer_volume_and_speed() {
        let catalog = load_printer_profiles();
        let dimensions = [240.0, 220.0, 230.0];
        let bambu = printer_profile(&catalog, "bambu-p1s-0.4").unwrap();
        let prusa = printer_profile(&catalog, "prusa-mk4-0.4").unwrap();

        let bambu_estimate = estimate_print_for_dimensions(dimensions, Some(120.0), bambu);
        let prusa_estimate = estimate_print_for_dimensions(dimensions, Some(120.0), prusa);

        assert!(bambu_estimate.bed_fit);
        assert!(!prusa_estimate.bed_fit);
        assert_ne!(bambu_estimate.time_label, prusa_estimate.time_label);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_slicer_discovery_finds_known_app_bundles() {
        let root = temp_path("slicer-discovery");
        let apps = root.join("Applications");
        fs::create_dir_all(apps.join("OrcaSlicer.app")).unwrap();
        fs::create_dir_all(apps.join("PrusaSlicer.app")).unwrap();
        fs::create_dir_all(apps.join("Snapmaker Orca.app")).unwrap();
        fs::create_dir_all(apps.join("Creality Print.app")).unwrap();
        fs::create_dir_all(apps.join("Google Chrome.app")).unwrap();

        let found = discover_macos_slicer_candidates_in_roots(vec![apps]);

        assert_eq!(
            found
                .iter()
                .map(|candidate| candidate.label.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Creality Print",
                "OrcaSlicer",
                "PrusaSlicer",
                "Snapmaker Orca"
            ]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn shell_startup_restores_existing_last_folder_without_clearing_missing_path() {
        let root = temp_path("prefs-last-folder");
        let existing = root.join("models");
        let missing = root.join("missing");
        fs::create_dir_all(&existing).unwrap();

        let existing_prefs = AppPrefs {
            last_folder: Some(existing.clone()),
            ..AppPrefs::default()
        };
        let existing_path = root.join("existing-prefs.json");
        save_app_prefs_to_path(&existing_path, &existing_prefs).unwrap();
        let existing_state = ShellState::load_from_path(&existing_path);
        assert_eq!(existing_state.prefs.last_folder, Some(existing.clone()));
        assert_eq!(
            existing_state.restored_real_folder_candidate(),
            Some(existing)
        );
        assert_eq!(existing_state.current_folder, None);
        assert!(!existing_state.sidecar_writes_enabled);

        let missing_prefs = AppPrefs {
            last_folder: Some(missing.clone()),
            ..AppPrefs::default()
        };
        let missing_path = root.join("missing-prefs.json");
        save_app_prefs_to_path(&missing_path, &missing_prefs).unwrap();
        let missing_state = ShellState::load_from_path(&missing_path);
        assert_eq!(missing_state.prefs.last_folder, Some(missing));
        assert_eq!(missing_state.restored_real_folder_candidate(), None);
        assert_eq!(missing_state.current_folder, None);
        assert!(!missing_state.sidecar_writes_enabled);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn favorite_toggle_writes_sidecar_and_preserves_metadata() {
        let root = temp_path("favorite-sidecar");
        fs::create_dir_all(&root).unwrap();
        let model = root.join("part.stl");
        fs::write(&model, b"solid test\nendsolid test\n").unwrap();
        let mut entries = vec![test_entry(&model)];
        entries[0].meta = Some(scanner::SidecarMeta {
            tags: vec!["rack".to_string()],
            notes: "keep me".to_string(),
            printed: 2,
            ..scanner::SidecarMeta::default()
        });

        assert_eq!(
            persist_favorite_toggle(&prefs_with_root(&root), &mut entries, &model, true).unwrap(),
            Some(true)
        );

        let sidecar = model.with_file_name("part.stl.modelrack.json");
        let saved: scanner::SidecarMeta =
            serde_json::from_str(&fs::read_to_string(sidecar).unwrap()).unwrap();
        assert!(saved.favorite);
        assert_eq!(saved.tags, vec!["rack"]);
        assert_eq!(saved.notes, "keep me");
        assert_eq!(saved.printed, 2);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn favorite_toggle_creates_sidecar_for_real_model_without_existing_metadata() {
        let root = temp_path("favorite-new-sidecar");
        fs::create_dir_all(&root).unwrap();
        let model = root.join("part.stl");
        fs::write(&model, b"solid test\nendsolid test\n").unwrap();
        let mut entries = vec![test_entry(&model)];

        assert_eq!(
            persist_favorite_toggle(&prefs_with_root(&root), &mut entries, &model, true).unwrap(),
            Some(true)
        );

        let sidecar = model.with_file_name("part.stl.modelrack.json");
        let saved: scanner::SidecarMeta =
            serde_json::from_str(&fs::read_to_string(sidecar).unwrap()).unwrap();
        assert!(saved.favorite);
        assert!(saved.tags.is_empty());
        assert_eq!(saved.notes, "");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn favorite_toggle_with_no_write_policy_does_not_create_sidecar() {
        let root = temp_path("favorite-demo");
        let model = root.join("missing.stl");
        let mut entries = vec![test_entry(&model)];

        assert_eq!(
            persist_favorite_toggle(&AppPrefs::default(), &mut entries, &model, false).unwrap(),
            Some(true)
        );
        assert!(entries[0].meta.as_ref().unwrap().favorite);
        assert!(!model.with_file_name("missing.stl.modelrack.json").exists());
    }

    #[test]
    fn favorite_toggle_write_policy_is_not_demo_path_based() {
        let root = temp_path("favorite-policy");
        let model = root.join("existing-demo.stl");
        fs::create_dir_all(&root).unwrap();
        fs::write(&model, b"solid test\nendsolid test\n").unwrap();
        let mut entries = vec![test_entry(&model)];

        assert_eq!(
            persist_favorite_toggle(&AppPrefs::default(), &mut entries, &model, false).unwrap(),
            Some(true)
        );
        assert!(entries[0].meta.as_ref().unwrap().favorite);
        assert!(!model
            .with_file_name("existing-demo.stl.modelrack.json")
            .exists());

        assert_eq!(
            persist_favorite_toggle(&prefs_with_root(&root), &mut entries, &model, true).unwrap(),
            Some(false)
        );
        let sidecar = model.with_file_name("existing-demo.stl.modelrack.json");
        let saved: scanner::SidecarMeta =
            serde_json::from_str(&fs::read_to_string(sidecar).unwrap()).unwrap();
        assert!(!saved.favorite);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn metadata_tag_input_trims_deduplicates_and_drops_empty_tags() {
        assert_eq!(
            parse_tag_input("rack, printer, rack, , PLA"),
            vec!["rack".to_string(), "printer".to_string(), "PLA".to_string()]
        );
    }

    #[test]
    fn tag_chip_add_persists_unique_tags_and_preserves_metadata() {
        let root = temp_path("tag-chip-add");
        fs::create_dir_all(&root).unwrap();
        let model = root.join("part.stl");
        fs::write(&model, b"solid test\nendsolid test\n").unwrap();
        let mut entries = vec![test_entry(&model)];
        entries[0].meta = Some(scanner::SidecarMeta {
            tags: vec!["rack".to_string()],
            notes: "keep me".to_string(),
            favorite: true,
            printed: 2,
            print_history: vec![scanner::PrintRecord {
                date: "2026-05-08".to_string(),
                material: "PLA".to_string(),
                printer: "Bambu P1S".to_string(),
                profile: "0.20 Standard".to_string(),
                nozzle: "0.4 mm".to_string(),
                layer_height: "0.20 mm".to_string(),
                duration: "2h 10m".to_string(),
                success: true,
                notes: "clean".to_string(),
            }],
            ..scanner::SidecarMeta::default()
        });

        assert_eq!(
            persist_add_tags(
                &prefs_with_root(&root),
                &mut entries,
                &model,
                true,
                "rack, jig, printer"
            )
            .unwrap(),
            Some(3)
        );

        let sidecar = model.with_file_name("part.stl.modelrack.json");
        let saved: scanner::SidecarMeta =
            serde_json::from_str(&fs::read_to_string(sidecar).unwrap()).unwrap();
        assert_eq!(saved.tags, vec!["rack", "jig", "printer"]);
        assert_eq!(saved.notes, "keep me");
        assert!(saved.favorite);
        assert_eq!(saved.printed, 2);
        assert_eq!(saved.print_history.len(), 1);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn tag_drop_adds_missing_tag_and_persists_sidecar() {
        let root = temp_path("tag-drop-add");
        fs::create_dir_all(&root).unwrap();
        let model = root.join("part.stl");
        fs::write(&model, b"solid test\nendsolid test\n").unwrap();
        let mut entries = vec![test_entry(&model)];
        entries[0].meta = Some(scanner::SidecarMeta {
            tags: vec!["rack".to_string()],
            notes: "preserved".to_string(),
            ..scanner::SidecarMeta::default()
        });

        assert_eq!(
            persist_add_existing_tag(
                &prefs_with_root(&root),
                &mut entries,
                &model,
                true,
                "printer"
            )
            .unwrap(),
            Some(TagDropOutcome::Added {
                tag: "printer".to_string(),
                count: 2,
            })
        );

        let sidecar = model.with_file_name("part.stl.modelrack.json");
        let saved: scanner::SidecarMeta =
            serde_json::from_str(&fs::read_to_string(sidecar).unwrap()).unwrap();
        assert_eq!(saved.tags, vec!["rack", "printer"]);
        assert_eq!(saved.notes, "preserved");
        assert_eq!(
            entries[0].meta.as_ref().unwrap().tags,
            vec!["rack", "printer"]
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn tag_drop_skips_existing_tag_without_duplication() {
        let root = temp_path("tag-drop-existing");
        fs::create_dir_all(&root).unwrap();
        let model = root.join("part.stl");
        fs::write(&model, b"solid test\nendsolid test\n").unwrap();
        let mut entries = vec![test_entry(&model)];
        entries[0].meta = Some(scanner::SidecarMeta {
            tags: vec!["rack".to_string(), "printer".to_string()],
            ..scanner::SidecarMeta::default()
        });

        assert_eq!(
            persist_add_existing_tag(
                &prefs_with_root(&root),
                &mut entries,
                &model,
                true,
                "printer"
            )
            .unwrap(),
            Some(TagDropOutcome::AlreadyPresent {
                tag: "printer".to_string(),
                count: 2,
            })
        );

        let sidecar = model.with_file_name("part.stl.modelrack.json");
        assert!(!sidecar.exists());
        assert_eq!(
            entries[0].meta.as_ref().unwrap().tags,
            vec!["rack", "printer"]
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn tag_chip_remove_persists_selected_tag() {
        let root = temp_path("tag-chip-remove");
        fs::create_dir_all(&root).unwrap();
        let model = root.join("part.stl");
        fs::write(&model, b"solid test\nendsolid test\n").unwrap();
        let mut entries = vec![test_entry(&model)];
        entries[0].meta = Some(scanner::SidecarMeta {
            tags: vec!["rack".to_string(), "jig".to_string(), "printer".to_string()],
            ..scanner::SidecarMeta::default()
        });

        assert_eq!(
            persist_remove_tag(&prefs_with_root(&root), &mut entries, &model, true, 1).unwrap(),
            Some(2)
        );

        let sidecar = model.with_file_name("part.stl.modelrack.json");
        let saved: scanner::SidecarMeta =
            serde_json::from_str(&fs::read_to_string(sidecar).unwrap()).unwrap();
        assert_eq!(saved.tags, vec!["rack", "printer"]);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn tag_chip_no_write_policy_updates_memory_only() {
        let root = temp_path("tag-chip-demo");
        let model = root.join("missing.stl");
        let mut entries = vec![test_entry(&model)];

        assert_eq!(
            persist_add_tags(
                &AppPrefs::default(),
                &mut entries,
                &model,
                false,
                "demo, tag"
            )
            .unwrap(),
            Some(2)
        );
        assert_eq!(
            persist_remove_tag(&AppPrefs::default(), &mut entries, &model, false, 0).unwrap(),
            Some(1)
        );
        assert_eq!(entries[0].meta.as_ref().unwrap().tags, vec!["tag"]);
        assert!(!model.with_file_name("missing.stl.modelrack.json").exists());
    }

    #[test]
    fn tag_chip_write_policy_rejects_missing_real_model_without_mutation() {
        let root = temp_path("tag-chip-missing");
        let model = root.join("missing.stl");
        let mut entries = vec![test_entry(&model)];

        let err = match persist_add_tags(&prefs_with_root(&root), &mut entries, &model, true, "tag")
        {
            Ok(_) => panic!("missing real model should be rejected"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("model does not exist"));
        assert!(entries[0].meta.is_none());
    }

    #[test]
    fn metadata_edit_writes_sidecar_and_preserves_unedited_fields() {
        let root = temp_path("metadata-sidecar");
        fs::create_dir_all(&root).unwrap();
        let model = root.join("part.stl");
        fs::write(&model, b"solid test\nendsolid test\n").unwrap();
        let mut entries = vec![test_entry(&model)];
        entries[0].meta = Some(scanner::SidecarMeta {
            favorite: true,
            printed: 4,
            print_history: vec![scanner::PrintRecord {
                date: "2026-05-08".to_string(),
                material: "PLA".to_string(),
                printer: "Bambu P1S".to_string(),
                profile: "0.20 Standard".to_string(),
                nozzle: "0.4 mm".to_string(),
                layer_height: "0.20 mm".to_string(),
                duration: "2h 10m".to_string(),
                success: true,
                notes: "clean".to_string(),
            }],
            added: Some("2026-05-01".to_string()),
            ..scanner::SidecarMeta::default()
        });

        persist_metadata_fields(
            &prefs_with_root(&root),
            &mut entries,
            &model,
            true,
            "rack, printer, rack",
            "  makerworld  ",
            "Fits the rack shelf.",
        )
        .unwrap();

        let sidecar = model.with_file_name("part.stl.modelrack.json");
        let saved: scanner::SidecarMeta =
            serde_json::from_str(&fs::read_to_string(sidecar).unwrap()).unwrap();
        assert_eq!(saved.tags, vec!["rack", "printer"]);
        assert_eq!(saved.author, "makerworld");
        assert_eq!(saved.notes, "Fits the rack shelf.");
        assert!(saved.favorite);
        assert_eq!(saved.printed, 4);
        assert_eq!(saved.print_history.len(), 1);
        assert_eq!(saved.added.as_deref(), Some("2026-05-01"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn metadata_edit_with_no_write_policy_updates_memory_only() {
        let root = temp_path("metadata-demo");
        let model = root.join("missing.stl");
        let mut entries = vec![test_entry(&model)];

        persist_metadata_fields(
            &AppPrefs::default(),
            &mut entries,
            &model,
            false,
            "demo, tag",
            "You",
            "memo",
        )
        .unwrap();

        let meta = entries[0].meta.as_ref().unwrap();
        assert_eq!(meta.tags, vec!["demo", "tag"]);
        assert_eq!(meta.author, "You");
        assert_eq!(meta.notes, "memo");
        assert!(!model.with_file_name("missing.stl.modelrack.json").exists());
    }

    #[test]
    fn rename_selected_model_renames_file_and_sidecar() {
        let root = temp_path("rename-model");
        fs::create_dir_all(&root).unwrap();
        let old_path = root.join("old-part.stl");
        fs::write(&old_path, b"solid old\nendsolid old\n").unwrap();
        scanner::write_sidecar(
            &old_path,
            &scanner::SidecarMeta {
                tags: vec!["rack".to_string()],
                ..scanner::SidecarMeta::default()
            },
        )
        .unwrap();

        let mut state = ShellState::with_prefs(AppPrefs::default());
        state.entries = vec![test_entry(&old_path)];
        state.current_folder = Some(root.clone());
        state.sidecar_writes_enabled = true;
        state.snapshot_done();
        state.selected_index = Some(0);

        let new_path = state
            .rename_selected_model("renamed-part")
            .unwrap()
            .unwrap();
        assert_eq!(new_path, root.join("renamed-part.stl"));
        assert!(!old_path.exists());
        assert!(new_path.exists());
        assert!(!scanner::sidecar_path(&old_path).exists());
        assert!(scanner::sidecar_path(&new_path).exists());
        assert_eq!(state.entries[0].filename, "renamed-part.stl");
        assert_eq!(state.entries[0].path, new_path);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rename_selected_model_rejects_folder_escape_and_existing_targets() {
        let root = temp_path("rename-invalid");
        fs::create_dir_all(&root).unwrap();
        let old_path = root.join("part.stl");
        let existing_path = root.join("existing.stl");
        fs::write(&old_path, b"solid old\nendsolid old\n").unwrap();
        fs::write(&existing_path, b"solid existing\nendsolid existing\n").unwrap();

        let mut state = ShellState::with_prefs(AppPrefs::default());
        state.entries = vec![test_entry(&old_path)];
        state.current_folder = Some(root.clone());
        state.sidecar_writes_enabled = true;
        state.snapshot_done();
        state.selected_index = Some(0);

        assert!(state.rename_selected_model("../escape.stl").is_err());
        assert!(state.rename_selected_model("existing.stl").is_err());
        assert!(old_path.exists());
        assert_eq!(state.entries[0].path, old_path);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn print_count_delta_persists_and_floors_at_zero() {
        let root = temp_path("print-count");
        fs::create_dir_all(&root).unwrap();
        let model = root.join("part.stl");
        fs::write(&model, b"solid test\nendsolid test\n").unwrap();
        let mut entries = vec![test_entry(&model)];

        assert_eq!(
            persist_print_count_delta(&prefs_with_root(&root), &mut entries, &model, true, 2)
                .unwrap(),
            Some(2)
        );
        assert_eq!(
            persist_print_count_delta(&prefs_with_root(&root), &mut entries, &model, true, -5)
                .unwrap(),
            Some(0)
        );

        let sidecar = model.with_file_name("part.stl.modelrack.json");
        let saved: scanner::SidecarMeta =
            serde_json::from_str(&fs::read_to_string(sidecar).unwrap()).unwrap();
        assert_eq!(saved.printed, 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn print_record_add_persists_history_and_preserves_metadata() {
        let root = temp_path("print-history-add");
        fs::create_dir_all(&root).unwrap();
        let model = root.join("part.stl");
        fs::write(&model, b"solid test\nendsolid test\n").unwrap();
        let mut entries = vec![test_entry(&model)];
        entries[0].meta = Some(scanner::SidecarMeta {
            tags: vec!["rack".to_string()],
            favorite: true,
            author: "makerworld".to_string(),
            notes: "keep me".to_string(),
            ..scanner::SidecarMeta::default()
        });

        assert_eq!(
            persist_add_print_record(
                &prefs_with_root(&root),
                &mut entries,
                &model,
                true,
                " PLA ",
                " Prusa MK4 ",
                " 0.20 Quality ",
                " 0.4 mm ",
                " 0.20 mm ",
                " 2h 15m ",
                " good fit ",
                "2026-05-08"
            )
            .unwrap(),
            Some(1)
        );

        let sidecar = model.with_file_name("part.stl.modelrack.json");
        let saved: scanner::SidecarMeta =
            serde_json::from_str(&fs::read_to_string(sidecar).unwrap()).unwrap();
        assert_eq!(saved.printed, 1);
        assert_eq!(saved.print_history.len(), 1);
        assert_eq!(saved.print_history[0].date, "2026-05-08");
        assert_eq!(saved.print_history[0].material, "PLA");
        assert_eq!(saved.print_history[0].printer, "Prusa MK4");
        assert_eq!(saved.print_history[0].profile, "0.20 Quality");
        assert_eq!(saved.print_history[0].nozzle, "0.4 mm");
        assert_eq!(saved.print_history[0].layer_height, "0.20 mm");
        assert_eq!(saved.print_history[0].duration, "2h 15m");
        assert_eq!(saved.print_history[0].notes, "good fit");
        assert!(saved.print_history[0].success);
        assert_eq!(saved.tags, vec!["rack"]);
        assert!(saved.favorite);
        assert_eq!(saved.author, "makerworld");
        assert_eq!(saved.notes, "keep me");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn print_record_remove_persists_and_decrements_latest_display_row() {
        let root = temp_path("print-history-remove");
        fs::create_dir_all(&root).unwrap();
        let model = root.join("part.stl");
        fs::write(&model, b"solid test\nendsolid test\n").unwrap();
        let mut entries = vec![test_entry(&model)];
        entries[0].meta = Some(scanner::SidecarMeta {
            printed: 2,
            print_history: vec![
                scanner::PrintRecord {
                    date: "2026-05-07".to_string(),
                    material: "PLA".to_string(),
                    printer: "Bambu P1S".to_string(),
                    profile: "0.20 Standard".to_string(),
                    nozzle: "0.4 mm".to_string(),
                    layer_height: "0.20 mm".to_string(),
                    duration: "2h 10m".to_string(),
                    success: true,
                    notes: "older".to_string(),
                },
                scanner::PrintRecord {
                    date: "2026-05-08".to_string(),
                    material: "PETG".to_string(),
                    printer: "Prusa MK4".to_string(),
                    profile: "0.15 Quality".to_string(),
                    nozzle: "0.4 mm".to_string(),
                    layer_height: "0.15 mm".to_string(),
                    duration: "3h 05m".to_string(),
                    success: true,
                    notes: "latest".to_string(),
                },
            ],
            ..scanner::SidecarMeta::default()
        });

        assert_eq!(
            persist_remove_print_record(&prefs_with_root(&root), &mut entries, &model, true, 0)
                .unwrap(),
            Some(1)
        );

        let sidecar = model.with_file_name("part.stl.modelrack.json");
        let saved: scanner::SidecarMeta =
            serde_json::from_str(&fs::read_to_string(sidecar).unwrap()).unwrap();
        assert_eq!(saved.printed, 1);
        assert_eq!(saved.print_history.len(), 1);
        assert_eq!(saved.print_history[0].date, "2026-05-07");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn print_record_no_write_policy_updates_memory_only() {
        let root = temp_path("print-history-demo");
        let model = root.join("missing.stl");
        let mut entries = vec![test_entry(&model)];

        persist_add_print_record(
            &AppPrefs::default(),
            &mut entries,
            &model,
            false,
            "PLA",
            "Bambu P1S",
            "0.20 Standard",
            "0.4 mm",
            "0.20 mm",
            "2h",
            "demo",
            "2026-05-08",
        )
        .unwrap();
        assert_eq!(entries[0].meta.as_ref().unwrap().printed, 1);
        assert_eq!(entries[0].meta.as_ref().unwrap().print_history.len(), 1);
        assert!(!model.with_file_name("missing.stl.modelrack.json").exists());
    }

    #[test]
    fn print_record_write_policy_rejects_missing_real_model_without_mutation() {
        let root = temp_path("print-history-missing");
        let model = root.join("missing.stl");
        let mut entries = vec![test_entry(&model)];

        let err = match persist_add_print_record(
            &prefs_with_root(&root),
            &mut entries,
            &model,
            true,
            "PLA",
            "Bambu P1S",
            "0.20 Standard",
            "0.4 mm",
            "0.20 mm",
            "2h",
            "should fail",
            "2026-05-08",
        ) {
            Ok(_) => panic!("missing real model should be rejected"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("model does not exist"));
        assert!(entries[0].meta.is_none());
    }

    #[test]
    fn print_record_legacy_json_defaults_profile_fields() {
        let json = r#"{
            "date": "2026-05-08",
            "material": "PLA",
            "success": true,
            "notes": "legacy"
        }"#;

        let record: scanner::PrintRecord = serde_json::from_str(json).unwrap();

        assert_eq!(record.material, "PLA");
        assert_eq!(record.printer, "");
        assert_eq!(record.profile, "");
        assert_eq!(record.nozzle, "");
        assert_eq!(record.layer_height, "");
        assert_eq!(record.duration, "");
        assert_eq!(record.notes, "legacy");
        assert!(record.success);
    }

    #[test]
    fn civil_date_formats_unix_epoch() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn metadata_write_policy_rejects_missing_real_model() {
        let root = temp_path("metadata-missing");
        let model = root.join("missing.stl");
        let mut entries = vec![test_entry(&model)];

        let err = match persist_metadata_fields(
            &prefs_with_root(&root),
            &mut entries,
            &model,
            true,
            "tag",
            "author",
            "notes",
        ) {
            Ok(_) => panic!("missing real model should be rejected"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("model does not exist"));
    }

    #[test]
    fn launcher_command_uses_default_or_configured_slicer() {
        let model = Path::new("/tmp/model.stl");
        let default = launcher_command(model, "");

        #[cfg(target_os = "macos")]
        assert_eq!(
            default,
            LaunchCommand {
                program: "open".to_string(),
                args: vec!["/tmp/model.stl".to_string()],
                wait_for_exit: true,
            }
        );

        #[cfg(target_os = "windows")]
        assert_eq!(
            default,
            LaunchCommand {
                program: "cmd".to_string(),
                args: vec![
                    "/C".to_string(),
                    "start".to_string(),
                    "".to_string(),
                    "/tmp/model.stl".to_string(),
                ],
                wait_for_exit: true,
            }
        );

        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        assert_eq!(
            default,
            LaunchCommand {
                program: "xdg-open".to_string(),
                args: vec!["/tmp/model.stl".to_string()],
                wait_for_exit: true,
            }
        );

        assert_eq!(
            launcher_command(model, "/usr/local/bin/slicer"),
            LaunchCommand {
                program: "/usr/local/bin/slicer".to_string(),
                args: vec!["/tmp/model.stl".to_string()],
                wait_for_exit: false,
            }
        );
    }

    #[test]
    fn launcher_preflight_rejects_missing_model_or_slicer() {
        let root = temp_path("launcher-preflight");
        let model = root.join("part.stl");
        let slicer = root.join("slicer");

        assert!(validate_launch_request(&model, "").is_err());

        fs::create_dir_all(&root).unwrap();
        fs::write(&model, b"solid test\nendsolid test\n").unwrap();
        assert!(validate_launch_request(&model, slicer.to_str().unwrap()).is_err());

        fs::write(&slicer, b"#!/bin/sh\n").unwrap();
        validate_launch_request(&model, slicer.to_str().unwrap()).unwrap();

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn launcher_waits_and_reports_nonzero_helper_exit() {
        let command = LaunchCommand {
            program: "sh".to_string(),
            args: vec!["-c".to_string(), "exit 7".to_string()],
            wait_for_exit: true,
        };

        let err = run_launch_command(command).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn launcher_reports_early_nonzero_exit_for_configured_slicer() {
        let command = LaunchCommand {
            program: "sh".to_string(),
            args: vec!["-c".to_string(), "exit 7".to_string()],
            wait_for_exit: false,
        };

        let err = run_launch_command(command).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn browser_count_label_matches_mockup_language() {
        assert_eq!(browser_count_label(36, 36, "en"), "36 items");
        assert_eq!(browser_count_label(9, 36, "en"), "9 of 36 items");
        assert_eq!(browser_count_label(36, 36, "ko"), "36개 항목");
        assert_eq!(browser_count_label(9, 36, "ko"), "9 / 36개 항목");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn launcher_command_uses_open_a_for_macos_app_bundles() {
        assert_eq!(
            launcher_command(Path::new("/tmp/model.stl"), "/Applications/PrusaSlicer.app"),
            LaunchCommand {
                program: "open".to_string(),
                args: vec![
                    "-a".to_string(),
                    "/Applications/PrusaSlicer.app".to_string(),
                    "/tmp/model.stl".to_string(),
                ],
                wait_for_exit: true,
            }
        );
    }
}
