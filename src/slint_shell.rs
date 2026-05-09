use std::cell::RefCell;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::scanner;
use crate::strings;
use crate::view_model::{
    display_path_label, smart_filter_from_key, AppPrefs, AppViewSnapshot,
    BrowserCard as BrowserCardVm, Density, DisplayQuery, LibraryFilter, ScanStatus, SortBy,
    ViewMode,
};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use slint::winit_030::winit::dpi::PhysicalSize;
use slint::winit_030::WinitWindowAccessor;
use slint::Model;

slint::include_modules!();

const DEFAULT_WINDOW_WIDTH: u32 = 1480;
const DEFAULT_WINDOW_HEIGHT: u32 = 920;
const MIN_WINDOW_WIDTH: u32 = 960;
const MIN_WINDOW_HEIGHT: u32 = 640;
const LIBRARY_WATCH_DEBOUNCE: Duration = Duration::from_millis(750);
const LIBRARY_WATCH_POLL_INTERVAL: Duration = Duration::from_millis(250);

const DEMO_ROOT: &str = "/Users/hwankishin/Library/3d";

pub fn run() -> Result<(), slint::PlatformError> {
    configure_slint_backend()?;

    let ui = ModelRackWindow::new()?;
    crate::fonts::install_slint_fonts();
    let state = Rc::new(RefCell::new(ShellState::load()));
    let watcher_runtime = Rc::new(RefCell::new(LibraryWatcherRuntime::new()));
    let scan_runtime = Rc::new(RefCell::new(LibraryScanRuntime::new()));
    let snapshot = state.borrow_mut().snapshot_idle();

    apply_snapshot(&ui, &snapshot);
    apply_detail(&ui, &state.borrow());
    apply_settings(&ui, &state.borrow());
    if let Some(folder) = state.borrow().restored_real_folder_candidate() {
        start_folder_scan(
            &ui,
            &state,
            &scan_runtime,
            &folder,
            "Restoring last library",
        );
        set_watch_status(&ui, &watcher_runtime, &folder);
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
            if let Some(folder) = request_active_real_folder_scan(
                &ui,
                &refresh_state,
                &refresh_scan,
                "Refreshing library",
            ) {
                set_watch_status(&ui, &refresh_watcher, &folder);
            }
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
            apply_detail(&ui, &search_state.borrow());
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
                state.selected_index = None;
                state.snapshot_done()
            };
            apply_snapshot(&ui, &snapshot);
            apply_detail(&ui, &sort_state.borrow());
            apply_settings(&ui, &sort_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let filter_state = state.clone();
    ui.on_choose_filter(move |key| {
        if let Some(ui) = weak.upgrade() {
            let snapshot = {
                let mut state = filter_state.borrow_mut();
                if let Some(filter) = smart_filter_from_key(key.as_str()) {
                    state.filter = filter;
                }
                state.selected_index = None;
                state.snapshot_done()
            };
            apply_snapshot(&ui, &snapshot);
            apply_detail(&ui, &filter_state.borrow());
            apply_settings(&ui, &filter_state.borrow());
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
    let select_state = state.clone();
    ui.on_select_model(move |index| {
        if let Some(ui) = weak.upgrade() {
            let mut state = select_state.borrow_mut();
            state.selected_index = Some(index as usize);
            apply_detail(&ui, &state);
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
                    match persist_favorite_toggle(&mut state.entries, &path, allow_sidecar_writes) {
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
                apply_detail(&ui, &state);
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
            match persist_metadata_fields(
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
            apply_detail(&ui, &state);
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
            match persist_add_tags(
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
            apply_detail(&ui, &state);
            apply_settings(&ui, &state);
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
            match persist_remove_tag(&mut state.entries, &path, allow_sidecar_writes, index) {
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
            apply_detail(&ui, &state);
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
            match persist_print_count_delta(&mut state.entries, &path, allow_sidecar_writes, delta)
            {
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
            apply_detail(&ui, &state);
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
                match persist_add_print_record(
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
                apply_detail(&ui, &state);
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
            match persist_remove_print_record(
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
            apply_detail(&ui, &state);
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
                let _ = request_active_real_folder_scan(
                    &ui,
                    &auto_state,
                    &auto_scan,
                    "Auto-refreshing library after file change",
                );
            }
            if let Some(error) = watch_poll.error {
                ui.set_status_text(
                    format!("File watcher error: {error}; use Refresh manually").into(),
                );
            }
            if let Some(result) = auto_scan.borrow_mut().poll() {
                apply_scan_result(&ui, &auto_state, result);
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
        },
    );

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

struct LibraryScanRuntime {
    next_generation: u64,
    active_generation: Option<u64>,
    active_folder: Option<PathBuf>,
    tx: mpsc::Sender<ScanResult>,
    rx: mpsc::Receiver<ScanResult>,
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
        if self.active_generation.is_some() && self.active_folder.as_deref() == Some(folder) {
            return ScanRequest::AlreadyRunning;
        }

        self.next_generation += 1;
        let generation = self.next_generation;
        self.active_generation = Some(generation);
        self.active_folder = Some(folder.to_path_buf());
        let folder = folder.to_path_buf();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let (entries, skipped) = scan_folder_entries(&folder);
            let _ = tx.send(ScanResult {
                generation,
                folder,
                entries,
                skipped,
            });
        });
        ScanRequest::Started
    }

    fn poll(&mut self) -> Option<ScanResult> {
        let mut latest = None;
        while let Ok(result) = self.rx.try_recv() {
            if Some(result.generation) == self.active_generation
                && Some(result.folder.as_path()) == self.active_folder.as_deref()
            {
                self.active_generation = None;
                self.active_folder = None;
                latest = Some(result);
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
                "stl" | "3mf" | "obj" | "step" | "stp"
            )
        })
        .unwrap_or(false)
}

fn set_watch_status(
    ui: &ModelRackWindow,
    watcher_runtime: &Rc<RefCell<LibraryWatcherRuntime>>,
    folder: &Path,
) {
    match watcher_runtime.borrow_mut().watch_folder(folder) {
        Ok(()) => ui.set_status_text("Watching library for changes".into()),
        Err(err) => ui.set_status_text(format!("{err}; use Refresh manually").into()),
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
        scan_runtime.borrow_mut().invalidate();
        start_folder_scan(ui, state, scan_runtime, &folder, "Scanning selected folder");
        set_watch_status(ui, watcher_runtime, &folder);
    }
}

fn start_folder_scan(
    ui: &ModelRackWindow,
    state: &Rc<RefCell<ShellState>>,
    scan_runtime: &Rc<RefCell<LibraryScanRuntime>>,
    folder: &Path,
    status: &str,
) {
    let snapshot = state.borrow_mut().begin_folder_scan(folder, status);
    apply_snapshot(ui, &snapshot);
    apply_detail(ui, &state.borrow());
    apply_settings(ui, &state.borrow());
    save_prefs_status(ui, &state.borrow());
    match scan_runtime.borrow_mut().request_scan(folder) {
        ScanRequest::Started => {}
        ScanRequest::AlreadyRunning => {
            ui.set_status_text("Scan already running for this library".into())
        }
    }
}

fn request_active_real_folder_scan(
    ui: &ModelRackWindow,
    state: &Rc<RefCell<ShellState>>,
    scan_runtime: &Rc<RefCell<LibraryScanRuntime>>,
    status: &str,
) -> Option<PathBuf> {
    let Some(folder) = state.borrow().active_real_folder() else {
        ui.set_status_text("Choose a real library folder before refreshing".into());
        return None;
    };

    match scan_runtime.borrow_mut().request_scan(&folder) {
        ScanRequest::Started => ui.set_status_text(status.into()),
        ScanRequest::AlreadyRunning => {
            ui.set_status_text("Refresh already running for this library".into())
        }
    }
    Some(folder)
}

fn apply_scan_result(ui: &ModelRackWindow, state: &Rc<RefCell<ShellState>>, result: ScanResult) {
    if state.borrow().active_real_folder().as_deref() != Some(result.folder.as_path()) {
        return;
    }
    let snapshot = state.borrow_mut().apply_scan_result(result);
    apply_snapshot(ui, &snapshot);
    apply_detail(ui, &state.borrow());
    apply_settings(ui, &state.borrow());
    save_prefs_status(ui, &state.borrow());
}

fn apply_detail(ui: &ModelRackWindow, state: &ShellState) {
    ui.set_selected_card_index(state.selected_index.map(|i| i as i32).unwrap_or(-1));
    if let Some(idx) = state.selected_index {
        if let Some(entry) = state.displayed.get(idx) {
            ui.set_has_selection(true);
            ui.set_selected_thumb_key(crate::view_model::thumbnail_key(&entry.filename).into());
            let (thumb_image, thumb_ready) = load_thumbnail_image(entry.thumbnail_path.as_deref());
            ui.set_selected_thumb_image(thumb_image);
            ui.set_selected_thumb_ready(thumb_ready);
            ui.set_detail_name(entry.filename.clone().into());
            ui.set_detail_path(detail_parent_label(entry, state).into());
            ui.set_detail_format(
                match entry.stl_type {
                    scanner::StlType::Binary => "Binary STL",
                    scanner::StlType::Ascii => "ASCII STL",
                    scanner::StlType::ThreeMf => "3MF",
                    scanner::StlType::Obj => "OBJ",
                    scanner::StlType::Step => "STEP",
                    scanner::StlType::LargeStl => "Large STL",
                    scanner::StlType::Unknown => "Unknown",
                }
                .into(),
            );
            ui.set_detail_tris(
                entry
                    .triangle_count
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
                entry
                    .dimensions
                    .map(|[x, y, z]| format!("{:.1} × {:.1} × {:.1} mm", x, y, z))
                    .unwrap_or_else(|| "—".to_string())
                    .into(),
            );
            ui.set_detail_volume(
                entry
                    .dimensions
                    .map(|[x, y, z]| format!("{:.1} cm³", x * y * z / 1000.0 * 0.30))
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
            ui.set_detail_added("—".into());
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
            let watertight = manifold && entry.triangle_count.unwrap_or(0) > 100;
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
            if let Some([x, y, z]) = entry.dimensions {
                let bbox_cm3 = x * y * z / 1000.0;
                let part_vol = bbox_cm3 * 0.30;
                let grams = (part_vol * 0.45 + part_vol * 0.55 * 0.15) * 1.24;
                let minutes = (grams / 0.6).max(6.0) as u32;
                let hours = minutes / 60;
                let mins = minutes % 60;
                ui.set_detail_estimate_time(
                    if hours > 0 {
                        format!("{}h {}m", hours, mins)
                    } else {
                        format!("{}m", mins)
                    }
                    .into(),
                );
                ui.set_detail_estimate_grams(format!("{}g", grams as u32).into());
                ui.set_detail_estimate_layers(format!("{}", (z / 0.20) as u32).into());
                ui.set_detail_bed_fit(x <= 256.0 && y <= 256.0 && z <= 256.0);
            } else {
                ui.set_detail_estimate_time("".into());
                ui.set_detail_estimate_grams("".into());
                ui.set_detail_estimate_layers("".into());
                ui.set_detail_bed_fit(false);
            }
        } else {
            ui.set_has_selection(false);
            ui.set_selected_thumb_key("rack".into());
            ui.set_selected_thumb_image(slint::Image::default());
            ui.set_selected_thumb_ready(false);
            clear_detail_tag_chips(ui);
            clear_detail_print_history(ui);
        }
    } else {
        ui.set_has_selection(false);
        ui.set_selected_thumb_key("rack".into());
        ui.set_selected_thumb_image(slint::Image::default());
        ui.set_selected_thumb_ready(false);
        clear_detail_tag_chips(ui);
        clear_detail_print_history(ui);
    }
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
        Ok(data) => serde_json::from_str::<AppPrefs>(&data)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err)),
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

fn persist_favorite_toggle(
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
    entries: &mut [scanner::StlFileInfo],
    path: &Path,
    allow_sidecar_writes: bool,
    tags: &str,
    author: &str,
    notes: &str,
) -> anyhow::Result<Option<scanner::SidecarMeta>> {
    update_model_meta(entries, path, allow_sidecar_writes, |meta| {
        meta.tags = parse_tag_input(tags);
        meta.author = author.trim().to_string();
        meta.notes = notes.to_string();
    })
}

fn persist_add_tags(
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

    let updated = update_model_meta(entries, path, allow_sidecar_writes, |meta| {
        for tag in additions {
            if !meta.tags.iter().any(|existing| existing == &tag) {
                meta.tags.push(tag);
            }
        }
    })?;
    Ok(updated.map(|meta| meta.tags.len()))
}

fn persist_remove_tag(
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

    let updated = update_model_meta(entries, path, allow_sidecar_writes, |meta| {
        meta.tags.remove(tag_index);
    })?;
    Ok(updated.map(|meta| meta.tags.len()))
}

fn persist_print_count_delta(
    entries: &mut [scanner::StlFileInfo],
    path: &Path,
    allow_sidecar_writes: bool,
    delta: i32,
) -> anyhow::Result<Option<u32>> {
    let updated = update_model_meta(entries, path, allow_sidecar_writes, |meta| {
        let next = meta.printed as i64 + delta as i64;
        meta.printed = next.max(0).min(u32::MAX as i64) as u32;
    })?;
    Ok(updated.map(|meta| meta.printed))
}

fn persist_add_print_record(
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
    let updated = update_model_meta(entries, path, allow_sidecar_writes, |meta| {
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

    let updated = update_model_meta(entries, path, allow_sidecar_writes, |meta| {
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
    Ok(Some(meta))
}

#[derive(Clone)]
struct ShellState {
    entries: Vec<scanner::StlFileInfo>,
    displayed: Vec<scanner::StlFileInfo>,
    current_folder: Option<PathBuf>,
    prefs: AppPrefs,
    search_query: String,
    filter: LibraryFilter,
    sort_by: SortBy,
    sort_ascending: bool,
    skipped: usize,
    settings_open: bool,
    settings_tab: String,
    selected_index: Option<usize>,
    sidecar_writes_enabled: bool,
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

    fn with_prefs(prefs: AppPrefs) -> Self {
        let entries = demo_entries();
        Self {
            entries,
            displayed: Vec::new(),
            current_folder: None,
            prefs,
            search_query: String::new(),
            filter: LibraryFilter::All,
            sort_by: SortBy::Name,
            sort_ascending: true,
            skipped: 0,
            settings_open: false,
            settings_tab: "general".to_string(),
            selected_index: Some(0),
            sidecar_writes_enabled: false,
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
        let query = DisplayQuery {
            search_query: &self.search_query,
            library_filter: &self.filter,
            sort_by: self.sort_by,
            sort_ascending: self.sort_ascending,
            preserve_order: false,
        };
        AppViewSnapshot::from_parts(
            &self.entries,
            self.current_folder.as_deref(),
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
        AppViewSnapshot::from_parts(
            &self.entries,
            self.current_folder.as_deref(),
            &ScanStatus::Idle,
            &self.prefs,
            DisplayQuery {
                search_query: &self.search_query,
                library_filter: &self.filter,
                sort_by: self.sort_by,
                sort_ascending: self.sort_ascending,
                preserve_order: false,
            },
        )
    }

    fn begin_folder_scan(&mut self, folder: &Path, current: &str) -> AppViewSnapshot {
        self.entries.clear();
        self.displayed.clear();
        self.current_folder = Some(folder.to_path_buf());
        self.prefs.last_folder = Some(folder.to_path_buf());
        self.skipped = 0;
        self.sidecar_writes_enabled = true;
        self.selected_index = None;
        let query = DisplayQuery {
            search_query: &self.search_query,
            library_filter: &self.filter,
            sort_by: self.sort_by,
            sort_ascending: self.sort_ascending,
            preserve_order: false,
        };
        AppViewSnapshot::from_parts(
            &self.entries,
            self.current_folder.as_deref(),
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

    fn apply_scan_parts(
        &mut self,
        folder: PathBuf,
        entries: Vec<scanner::StlFileInfo>,
        skipped: usize,
    ) -> AppViewSnapshot {
        self.entries = entries;
        self.current_folder = Some(folder.clone());
        self.prefs.last_folder = Some(folder);
        self.skipped = skipped;
        self.sidecar_writes_enabled = true;
        self.selected_index = if self.entries.is_empty() {
            None
        } else {
            Some(0)
        };
        self.snapshot_done()
    }

    fn active_real_folder(&self) -> Option<PathBuf> {
        self.sidecar_writes_enabled
            .then(|| self.current_folder.clone())
            .flatten()
            .filter(|folder| folder.is_dir())
    }

    fn restored_real_folder_candidate(&self) -> Option<PathBuf> {
        self.prefs
            .last_folder
            .clone()
            .filter(|folder| folder.is_dir())
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

    fn cycle_language(&mut self) {
        self.prefs.language = match self.prefs.language.as_str() {
            "en" => "ko",
            "ko" => "ja",
            _ => "en",
        }
        .to_string();
    }

    fn toggle_theme(&mut self) {
        self.prefs.theme = if self.prefs.theme == "dark" {
            "light".to_string()
        } else {
            "dark".to_string()
        };
    }

    fn selected_model_path(&self) -> Option<PathBuf> {
        let idx = self.selected_index?;
        self.displayed.get(idx).map(|entry| entry.path.clone())
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
        browser_count_label(snapshot.browser.displayed, snapshot.browser.total).into(),
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
        })
        .collect::<Vec<SidebarItem>>();
    ui.set_tag_items(slint::ModelRc::new(slint::VecModel::from(tags)));
}

fn browser_count_label(displayed: usize, total: usize) -> String {
    if displayed == total {
        format!("{} items", total)
    } else {
        format!("{} of {} items", displayed, total)
    }
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
        state
            .current_folder
            .as_ref()
            .map(|folder| display_path_label(folder))
            .unwrap_or_else(|| "No folder selected".to_string())
    } else {
        "Sample library (demo, memory-only)".to_string()
    }
}

fn apply_settings(ui: &ModelRackWindow, state: &ShellState) {
    let discovered_slicers = discover_slicer_candidates();
    let slicer_rows = slicer_choice_rows(&state.prefs.slicer_path, &discovered_slicers);
    ui.set_settings_open(state.settings_open);
    ui.set_settings_tab(state.settings_tab.clone().into());
    ui.set_settings_language_label(language_label(&state.prefs.language).into());
    ui.set_settings_theme_label(theme_label(&state.prefs.theme).into());
    ui.set_settings_folder_label(settings_folder_label(state).into());
    ui.set_settings_density_label(Density::from_str(&state.prefs.density).as_str().into());
    ui.set_settings_slicer_label(
        slicer_label_for_path(&state.prefs.slicer_path, &slicer_rows).into(),
    );
    ui.set_settings_slicer_candidates(slint::ModelRc::new(slint::VecModel::from(
        slicer_rows
            .into_iter()
            .map(|row| SlicerCandidate {
                label: row.label.into(),
                path: row.path.into(),
                detail: row.detail.into(),
                selected: row.selected,
            })
            .collect::<Vec<SlicerCandidate>>(),
    )));
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DiscoveredSlicer {
    label: String,
    path: PathBuf,
    detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SlicerChoiceRow {
    label: String,
    path: String,
    detail: String,
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
        selected: selected.is_empty(),
    });

    for slicer in discovered {
        let path = slicer.path.display().to_string();
        rows.push(SlicerChoiceRow {
            label: slicer.label.clone(),
            path: path.clone(),
            detail: slicer.detail.clone(),
            selected: selected == path,
        });
    }

    if !selected.is_empty() && !rows.iter().any(|row| row.path == selected) {
        rows.push(SlicerChoiceRow {
            label: display_slicer_path(selected),
            path: selected.to_string(),
            detail: "Manual selection".to_string(),
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
        out.push(DiscoveredSlicer {
            label: label.to_string(),
            path,
            detail: detail.to_string(),
        });
    }
}

#[cfg(target_os = "macos")]
fn discover_macos_slicer_candidates_in_roots<I>(roots: I) -> Vec<DiscoveredSlicer>
where
    I: IntoIterator<Item = PathBuf>,
{
    const MAC_SLICERS: &[(&str, &str)] = &[
        ("OrcaSlicer", "OrcaSlicer.app"),
        ("Bambu Studio", "BambuStudio.app"),
        ("PrusaSlicer", "PrusaSlicer.app"),
        ("UltiMaker Cura", "UltiMaker Cura.app"),
        ("Cura", "Cura.app"),
        ("SuperSlicer", "SuperSlicer.app"),
        ("ideaMaker", "ideaMaker.app"),
    ];

    let mut out = Vec::new();
    for root in roots {
        for (label, bundle) in MAC_SLICERS {
            push_unique_slicer(
                &mut out,
                label,
                root.join(bundle),
                "Detected macOS app bundle",
            );
        }
    }
    out
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
        "ko" => "Korean",
        "ja" => "Japanese",
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

fn scan_folder_entries(folder: &Path) -> (Vec<scanner::StlFileInfo>, usize) {
    let (tx, rx) = crossbeam_channel::unbounded();
    scanner::scan_folder_stream(folder, tx);

    let mut entries = Vec::new();
    let mut skipped = 0usize;
    for event in rx {
        match event {
            scanner::ScanEvent::Progress {
                scanned,
                skipped,
                current,
            } => {
                let _ = (scanned, skipped, current);
            }
            scanner::ScanEvent::Entry { mut info, mesh } => {
                info.thumbnail_path =
                    crate::thumbnail_cache::ensure_thumbnail(&info, mesh.as_ref()).ok();
                entries.push(*info);
            }
            scanner::ScanEvent::Done {
                skipped: done_skipped,
            } => {
                skipped = done_skipped;
                break;
            }
        }
    }

    entries.sort_by(|a, b| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()));
    (entries, skipped)
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
    let Some(path) = path else {
        return (slint::Image::default(), false);
    };
    match slint::Image::load_from_path(path) {
        Ok(image) => (image, true),
        Err(err) => {
            eprintln!(
                "Warning: failed to load thumbnail {}: {:?}",
                path.display(),
                err
            );
            (slint::Image::default(), false)
        }
    }
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
            modified: None,
            thumbnail_path: None,
            meta: None,
        }
    }

    #[test]
    fn watcher_relevance_includes_models_and_sidecars_only() {
        assert!(is_refresh_relevant_path(Path::new("part.stl")));
        assert!(is_refresh_relevant_path(Path::new("assembly.3mf")));
        assert!(is_refresh_relevant_path(Path::new("mount.obj")));
        assert!(is_refresh_relevant_path(Path::new("bracket.step")));
        assert!(is_refresh_relevant_path(Path::new("bracket.stp")));
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
            result = runtime.poll();
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
    fn library_scan_runtime_ignores_stale_generations() {
        let mut runtime = LibraryScanRuntime::new();
        runtime.active_generation = Some(2);
        runtime.active_folder = Some(PathBuf::from("/tmp/new"));
        runtime
            .tx
            .send(ScanResult {
                generation: 1,
                folder: PathBuf::from("/tmp/old"),
                entries: Vec::new(),
                skipped: 0,
            })
            .unwrap();
        runtime
            .tx
            .send(ScanResult {
                generation: 2,
                folder: PathBuf::from("/tmp/new"),
                entries: Vec::new(),
                skipped: 0,
            })
            .unwrap();

        let result = runtime
            .poll()
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
            .send(ScanResult {
                generation: 1,
                folder: PathBuf::from("/tmp/old-library"),
                entries: Vec::new(),
                skipped: 0,
            })
            .unwrap();

        assert!(runtime.poll().is_none());
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
    fn app_prefs_save_and_load_from_explicit_path() {
        let root = temp_path("prefs-roundtrip");
        let path = root.join("prefs.json");
        let prefs = AppPrefs {
            density: "large".to_string(),
            view_mode: "masonry".to_string(),
            theme: "light".to_string(),
            language: "ko".to_string(),
            slicer_path: "/Applications/PrusaSlicer.app".to_string(),
            last_folder: Some(root.join("models")),
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

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_slicer_discovery_finds_known_app_bundles() {
        let root = temp_path("slicer-discovery");
        let apps = root.join("Applications");
        fs::create_dir_all(apps.join("OrcaSlicer.app")).unwrap();
        fs::create_dir_all(apps.join("PrusaSlicer.app")).unwrap();

        let found = discover_macos_slicer_candidates_in_roots(vec![apps]);

        assert_eq!(
            found
                .iter()
                .map(|candidate| candidate.label.as_str())
                .collect::<Vec<_>>(),
            vec!["OrcaSlicer", "PrusaSlicer"]
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
            persist_favorite_toggle(&mut entries, &model, true).unwrap(),
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
            persist_favorite_toggle(&mut entries, &model, true).unwrap(),
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
            persist_favorite_toggle(&mut entries, &model, false).unwrap(),
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
            persist_favorite_toggle(&mut entries, &model, false).unwrap(),
            Some(true)
        );
        assert!(entries[0].meta.as_ref().unwrap().favorite);
        assert!(!model
            .with_file_name("existing-demo.stl.modelrack.json")
            .exists());

        assert_eq!(
            persist_favorite_toggle(&mut entries, &model, true).unwrap(),
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
            persist_add_tags(&mut entries, &model, true, "rack, jig, printer").unwrap(),
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
            persist_remove_tag(&mut entries, &model, true, 1).unwrap(),
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
            persist_add_tags(&mut entries, &model, false, "demo, tag").unwrap(),
            Some(2)
        );
        assert_eq!(
            persist_remove_tag(&mut entries, &model, false, 0).unwrap(),
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

        let err = match persist_add_tags(&mut entries, &model, true, "tag") {
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

        persist_metadata_fields(&mut entries, &model, false, "demo, tag", "You", "memo").unwrap();

        let meta = entries[0].meta.as_ref().unwrap();
        assert_eq!(meta.tags, vec!["demo", "tag"]);
        assert_eq!(meta.author, "You");
        assert_eq!(meta.notes, "memo");
        assert!(!model.with_file_name("missing.stl.modelrack.json").exists());
    }

    #[test]
    fn print_count_delta_persists_and_floors_at_zero() {
        let root = temp_path("print-count");
        fs::create_dir_all(&root).unwrap();
        let model = root.join("part.stl");
        fs::write(&model, b"solid test\nendsolid test\n").unwrap();
        let mut entries = vec![test_entry(&model)];

        assert_eq!(
            persist_print_count_delta(&mut entries, &model, true, 2).unwrap(),
            Some(2)
        );
        assert_eq!(
            persist_print_count_delta(&mut entries, &model, true, -5).unwrap(),
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
            persist_remove_print_record(&mut entries, &model, true, 0).unwrap(),
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

        let err =
            match persist_metadata_fields(&mut entries, &model, true, "tag", "author", "notes") {
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
        assert_eq!(browser_count_label(36, 36), "36 items");
        assert_eq!(browser_count_label(9, 36), "9 of 36 items");
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
