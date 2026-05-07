use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

use std::time::{Duration, Instant};

use eframe::{CreationContext, Frame};
use egui::{
    CentralPanel, Context, FontData, FontDefinitions, FontFamily, ScrollArea, SidePanel,
    TextureHandle, TopBottomPanel,
};
use notify::{EventKind, RecursiveMode, Watcher};

use crate::renderer::OffscreenRenderer;
use crate::scanner;
use crate::strings;
use crate::ui::empty_state::EmptyState;
use crate::ui::grid::{self, GridWidget};
use crate::ui::list::ListWidget;
use crate::ui::masonry::MasonryWidget;
use crate::ui::metadata::MetadataPanel;
use crate::ui::settings::{SettingsDialog, SettingsTab};
use crate::ui::sidebar::SidebarWidget;
use crate::ui::status::StatusBar;
use crate::ui::toolbar::ToolbarWidget;
use crate::view_model::{
    AppPrefs, Density, DisplayQuery, FocusZone, LibraryFilter, ScanStatus, SortBy, ViewMode,
};
use crate::worker::{self, ThumbnailJob, ThumbnailResult};

pub struct AppState {
    pub entries: Vec<scanner::StlFileInfo>,
    pub textures: HashMap<[u8; 32], TextureHandle>,
    pub meshes: HashMap<[u8; 32], scanner::MeshData>,
    pub selected_hash: Option<[u8; 32]>,
    pub scan_status: ScanStatus,
    pub current_folder: Option<PathBuf>,
    pub search_query: String,
    pub library_filter: LibraryFilter,
    pub view_mode: ViewMode,
    pub density: Density,
    pub sort_by: SortBy,
    pub sort_ascending: bool,
    placeholder_tex: Option<TextureHandle>,
    scan_requested_folder: Option<PathBuf>,
    folder_picker_rx: Option<mpsc::Receiver<Option<PathBuf>>>,
    thumbnail_result_rx: Option<crossbeam_channel::Receiver<ThumbnailResult>>,
    scan_event_rx: Option<crossbeam_channel::Receiver<scanner::ScanEvent>>,
    pending_thumbnail_meshes: Vec<scanner::MeshData>,
    thumbnail_total: usize,
    thumbnail_done: usize,
    thumbnail_errors: usize,
    gpu_renderer: Option<Arc<OffscreenRenderer>>,
    cancel_token: Option<Arc<AtomicBool>>,
    collapsed_detail: bool,
    collapsed_sidebar: bool,
    user_expanded_detail: bool,
    user_expanded_sidebar: bool,
    focus_zone: FocusZone,
    settings_open: bool,
    settings_tab: SettingsTab,
    gpu_thumbnails_enabled: bool,
    theme: String,
    language: String,
    thumbnail_workers: usize,
    slicer_path: String,
    detail_preview_yaw: f32,
    maximized: bool,
    saved_prefs: AppPrefs,
    meta_dirty_since: Option<Instant>,
    meta_dirty_path: Option<std::path::PathBuf>,
    folder_watcher: Option<notify::RecommendedWatcher>,
    folder_watch_rx: Option<mpsc::Receiver<()>>,
    folder_change_seen_at: Option<Instant>,
    suppress_folder_events_until: Option<Instant>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            textures: HashMap::new(),
            meshes: HashMap::new(),
            selected_hash: None,
            scan_status: ScanStatus::Idle,
            current_folder: None,
            search_query: String::new(),
            library_filter: LibraryFilter::All,
            view_mode: ViewMode::Grid,
            density: Density::Medium,
            sort_by: SortBy::Name,
            sort_ascending: true,
            placeholder_tex: None,
            scan_requested_folder: None,
            folder_picker_rx: None,
            thumbnail_result_rx: None,
            scan_event_rx: None,
            pending_thumbnail_meshes: Vec::new(),
            thumbnail_total: 0,
            thumbnail_done: 0,
            thumbnail_errors: 0,
            gpu_renderer: None,
            cancel_token: None,
            collapsed_detail: false,
            collapsed_sidebar: false,
            user_expanded_detail: false,
            user_expanded_sidebar: false,
            focus_zone: FocusZone::Grid,
            settings_open: false,
            settings_tab: SettingsTab::General,
            gpu_thumbnails_enabled: true,
            theme: "dark".to_string(),
            language: "en".to_string(),
            thumbnail_workers: 4,
            slicer_path: String::new(),
            detail_preview_yaw: 0.0,
            maximized: false,
            saved_prefs: AppPrefs::default(),
            meta_dirty_since: None,
            meta_dirty_path: None,
            folder_watcher: None,
            folder_watch_rx: None,
            folder_change_seen_at: None,
            suppress_folder_events_until: None,
        }
    }
}

impl AppState {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        let prefs = load_preferences();
        install_fonts(&cc.egui_ctx);
        configure_egui(&cc.egui_ctx, &prefs.theme);
        crate::macos::install_app_menu();
        let restored_folder = prefs.last_folder.clone().filter(|p| p.exists());
        Self {
            density: Density::from_str(&prefs.density),
            view_mode: ViewMode::from_str(&prefs.view_mode),
            gpu_thumbnails_enabled: prefs.gpu_thumbnails_enabled,
            theme: prefs.theme.clone(),
            language: prefs.language.clone(),
            thumbnail_workers: prefs.thumbnail_workers.clamp(1, 8),
            slicer_path: prefs.slicer_path.clone(),
            current_folder: restored_folder.clone(),
            scan_requested_folder: restored_folder,
            saved_prefs: prefs,
            ..Self::default()
        }
    }

    fn start_scan(&mut self, ctx: &Context, folder: PathBuf) {
        self.current_folder = Some(folder.clone());
        self.library_filter = LibraryFilter::All;
        self.selected_hash = None;
        self.detail_preview_yaw = 0.0;
        self.folder_change_seen_at = None;

        // (#2) Check folder existence BEFORE spawning scan thread
        if !folder.exists() {
            self.scan_status = ScanStatus::Error(strings::MSG_FOLDER_NOT_FOUND.to_string());
            ctx.request_repaint();
            return;
        }
        self.start_folder_watcher(&folder);

        // (#4) Clear old texture cache to prevent memory leaks
        self.textures.clear();
        self.meshes.clear();
        self.entries.clear();
        self.pending_thumbnail_meshes.clear();
        self.thumbnail_total = 0;
        self.thumbnail_done = 0;
        self.thumbnail_errors = 0;

        // (#3) Cancel any running workers from previous scan
        if let Some(ref token) = self.cancel_token {
            token.store(true, Ordering::SeqCst);
        }
        self.thumbnail_result_rx = None;

        self.scan_status = ScanStatus::Scanning {
            found: 0,
            scanned: 0,
            skipped: 0,
            current: "starting scan".to_string(),
        };
        ctx.request_repaint();

        // (#1) Move scan to worker thread — eliminate UI freeze
        let (tx, rx) = crossbeam_channel::unbounded();
        std::thread::spawn(move || {
            scanner::scan_folder_stream(&folder, tx);
        });

        self.scan_event_rx = Some(rx);
    }

    fn process_scan_event(&mut self, ctx: &Context, event: scanner::ScanEvent) {
        match event {
            scanner::ScanEvent::Progress {
                scanned,
                skipped,
                current,
            } => {
                self.scan_status = ScanStatus::Scanning {
                    found: self.entries.len(),
                    scanned,
                    skipped,
                    current,
                };
                ctx.request_repaint();
            }
            scanner::ScanEvent::Entry { info, mesh } => {
                let info = *info;
                let cached_image = crate::thumbnail::load_cached_thumbnail(&info.hash);
                let has_cached_thumbnail = cached_image.is_some();
                let color_image = cached_image.unwrap_or_else(|| {
                    crate::thumbnail::generate_file_placeholder(info.stl_type, 256, 256)
                });
                let handle = ctx.load_texture(
                    format!("thumb_{:x?}", &info.hash[..8]),
                    color_image,
                    egui::TextureOptions::LINEAR,
                );
                self.textures.insert(info.hash, handle);

                if let Some(mesh) = mesh {
                    self.meshes.insert(mesh.hash, mesh.clone());
                    if !has_cached_thumbnail {
                        self.pending_thumbnail_meshes.push(mesh);
                    }
                }

                self.entries.push(info);
                self.scan_status = ScanStatus::Scanning {
                    found: self.entries.len(),
                    scanned: match self.scan_status {
                        ScanStatus::Scanning { scanned, .. } => scanned,
                        _ => self.entries.len(),
                    },
                    skipped: match self.scan_status {
                        ScanStatus::Scanning { skipped, .. } => skipped,
                        _ => 0,
                    },
                    current: match &self.scan_status {
                        ScanStatus::Scanning { current, .. } => current.clone(),
                        _ => "scanning".to_string(),
                    },
                };
                ctx.request_repaint();
            }
            scanner::ScanEvent::Done { skipped } => {
                self.entries
                    .sort_by(|a, b| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()));
                self.start_thumbnail_workers();
                let found = self.entries.len();
                self.scan_status = ScanStatus::Done { found, skipped };
                self.scan_event_rx = None;
                ctx.request_repaint();
            }
        }
    }

    fn start_thumbnail_workers(&mut self) {
        if self.pending_thumbnail_meshes.is_empty() {
            self.cancel_token = None;
            self.thumbnail_total = 0;
            return;
        }

        if self.gpu_thumbnails_enabled && self.gpu_renderer.is_none() {
            match OffscreenRenderer::new() {
                Ok(r) => {
                    self.gpu_renderer = Some(Arc::new(r));
                }
                Err(e) => {
                    eprintln!("GPU renderer unavailable: {}", e);
                }
            }
        }

        let token = Arc::new(AtomicBool::new(false));
        let meshes = std::mem::take(&mut self.pending_thumbnail_meshes);
        self.thumbnail_total = meshes.len();
        self.thumbnail_done = 0;
        self.thumbnail_errors = 0;
        let (job_tx, job_rx) = crossbeam_channel::bounded::<ThumbnailJob>(meshes.len());
        let (result_tx, result_rx) = crossbeam_channel::bounded::<ThumbnailResult>(meshes.len());

        for mesh in meshes {
            let _ = job_tx.send(ThumbnailJob::from(mesh));
        }
        drop(job_tx);

        let num_workers = self
            .thumbnail_workers
            .clamp(1, 8)
            .min(rayon::current_num_threads().max(1));
        worker::spawn_thumbnail_workers(
            job_rx,
            result_tx,
            num_workers,
            token.clone(),
            if self.gpu_thumbnails_enabled {
                self.gpu_renderer.clone()
            } else {
                None
            },
        );

        self.cancel_token = Some(token);
        self.thumbnail_result_rx = Some(result_rx);
    }

    fn selected_entry(&self) -> Option<&scanner::StlFileInfo> {
        self.selected_hash
            .and_then(|h| self.entries.iter().find(|e| e.hash == h))
    }

    fn mark_meta_dirty(&mut self) {
        if let Some(entry) = self.selected_entry() {
            self.meta_dirty_path = Some(entry.path.clone());
            self.meta_dirty_since = Some(Instant::now());
        }
    }

    fn request_folder_picker(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.folder_picker_rx = Some(rx);
        std::thread::spawn(move || {
            let folder = rfd::FileDialog::new().pick_folder();
            let _ = tx.send(folder);
        });
    }

    fn start_folder_watcher(&mut self, folder: &std::path::Path) {
        let (tx, rx) = mpsc::channel();
        let watch_tx = tx.clone();
        let mut watcher =
            match notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                let Ok(event) = event else {
                    return;
                };
                if !matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                ) {
                    return;
                }
                if event.paths.iter().any(|path| is_watch_relevant_path(path)) {
                    let _ = watch_tx.send(());
                }
            }) {
                Ok(watcher) => watcher,
                Err(err) => {
                    eprintln!("Failed to create folder watcher: {}", err);
                    self.folder_watcher = None;
                    self.folder_watch_rx = None;
                    return;
                }
            };

        if let Err(err) = watcher.watch(folder, RecursiveMode::Recursive) {
            eprintln!("Failed to watch {}: {}", folder.display(), err);
            self.folder_watcher = None;
            self.folder_watch_rx = None;
            return;
        }

        self.folder_watcher = Some(watcher);
        self.folder_watch_rx = Some(rx);
    }

    fn filter_label(&self) -> Option<String> {
        crate::view_model::filter_label(&self.library_filter)
    }

    fn current_prefs(&self) -> AppPrefs {
        AppPrefs {
            density: self.density.as_str().to_string(),
            view_mode: self.view_mode.as_str().to_string(),
            gpu_thumbnails_enabled: self.gpu_thumbnails_enabled,
            theme: self.theme.clone(),
            language: self.language.clone(),
            thumbnail_workers: self.thumbnail_workers.clamp(1, 8),
            slicer_path: self.slicer_path.clone(),
            last_folder: self.current_folder.clone(),
        }
    }

    fn save_preferences_if_needed(&mut self) {
        let prefs = self.current_prefs();
        if prefs != self.saved_prefs {
            if let Err(err) = save_preferences(&prefs) {
                eprintln!("Failed to save preferences: {}", err);
            } else {
                self.saved_prefs = prefs;
            }
        }
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        configure_egui(ctx, &self.theme);

        if self.placeholder_tex.is_none() {
            self.placeholder_tex = Some(grid::make_placeholder_texture(ctx));
        }

        // Responsive layout breakpoints — only auto-collapse when resizing below threshold,
        // but respect user toggles that explicitly re-open panels.
        let win_width = ctx.screen_rect().width();
        let below_detail_bp = win_width < 1024.0;
        let below_sidebar_bp = win_width < 800.0;

        if !below_detail_bp {
            // Window wide enough — reset everything
            self.collapsed_detail = false;
            self.user_expanded_detail = false;
        } else if !self.user_expanded_detail {
            self.collapsed_detail = true;
        }

        if !below_sidebar_bp {
            self.collapsed_sidebar = false;
            self.user_expanded_sidebar = false;
        } else if !self.user_expanded_sidebar {
            self.collapsed_sidebar = true;
        }

        // Check for folder picker result
        if let Some(rx) = &self.folder_picker_rx {
            if let Ok(result) = rx.try_recv() {
                self.folder_picker_rx = None;
                if let Some(folder) = result {
                    self.scan_requested_folder = Some(folder);
                }
            }
        }

        if let Some(rx) = &self.folder_watch_rx {
            if rx.try_iter().next().is_some() {
                let now = Instant::now();
                if self
                    .suppress_folder_events_until
                    .is_none_or(|until| now >= until)
                {
                    self.folder_change_seen_at = Some(now);
                }
            }
        }
        if self
            .suppress_folder_events_until
            .is_some_and(|until| Instant::now() >= until)
        {
            self.suppress_folder_events_until = None;
        }
        if self
            .folder_change_seen_at
            .is_some_and(|seen| seen.elapsed().as_millis() >= 750)
        {
            self.folder_change_seen_at = None;
            if self.scan_event_rx.is_none() {
                if let Some(folder) = self.current_folder.clone() {
                    self.scan_requested_folder = Some(folder);
                }
            }
        }

        // Process pending scan request
        if let Some(folder) = self.scan_requested_folder.take() {
            self.start_scan(ctx, folder);
        }

        // Poll streamed scan entries from worker thread so large folders populate immediately.
        let scan_events: Vec<_> = self
            .scan_event_rx
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        for event in scan_events {
            self.process_scan_event(ctx, event);
        }

        // Poll for completed thumbnails from workers
        if let Some(ref rx) = self.thumbnail_result_rx {
            let mut updated = false;
            for result in rx.try_iter() {
                match result {
                    ThumbnailResult::Success { hash, image } => {
                        self.thumbnail_done += 1;
                        if let Err(err) = crate::thumbnail::save_cached_thumbnail(&hash, &image) {
                            eprintln!("Failed to cache thumbnail {:x?}: {}", &hash[..8], err);
                        }
                        let handle = ctx.load_texture(
                            format!("thumb_{:x?}", &hash[..8]),
                            image,
                            egui::TextureOptions::LINEAR,
                        );
                        self.textures.insert(hash, handle);
                        updated = true;
                    }
                    ThumbnailResult::Error { hash, message } => {
                        self.thumbnail_done += 1;
                        self.thumbnail_errors += 1;
                        eprintln!("Thumbnail error for {:x?}: {}", &hash[..8], message);
                        let error_img = crate::thumbnail::generate_error_placeholder(160, 112);
                        let handle = ctx.load_texture(
                            format!("err_{:x?}", &hash[..8]),
                            error_img,
                            egui::TextureOptions::LINEAR,
                        );
                        self.textures.insert(hash, handle);
                        updated = true;
                    }
                }
            }
            if updated {
                ctx.request_repaint();
            }
        }

        // Cmd-F / Ctrl-F focuses search
        if ctx.input(|i| i.key_pressed(egui::Key::F) && (i.modifiers.command || i.modifiers.ctrl)) {
            self.focus_zone = FocusZone::Search;
            ctx.memory_mut(|mem| mem.request_focus(egui::Id::new("search_input")));
        }
        if ctx
            .input(|i| i.key_pressed(egui::Key::Comma) && (i.modifiers.command || i.modifiers.ctrl))
        {
            self.settings_open = true;
        }
        if crate::macos::take_settings_request() {
            self.settings_open = true;
        }
        if crate::macos::take_open_library_request() {
            self.request_folder_picker();
        }
        if self.settings_open && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.settings_open = false;
        }

        // Debounced sidecar save (500ms)
        if let Some(since) = self.meta_dirty_since {
            if since.elapsed().as_millis() >= 500 {
                if let Some(ref path) = self.meta_dirty_path.take() {
                    if let Some(entry) = self.entries.iter().find(|e| e.path == *path) {
                        if let Some(ref meta) = entry.meta {
                            if let Err(e) = scanner::write_sidecar(&entry.path, meta) {
                                eprintln!(
                                    "Failed to save sidecar for {}: {}",
                                    entry.path.display(),
                                    e
                                );
                            } else {
                                self.suppress_folder_events_until =
                                    Some(Instant::now() + Duration::from_secs(2));
                            }
                        }
                    }
                }
                self.meta_dirty_since = None;
            }
        }

        TopBottomPanel::top("title_bar")
            .exact_height(36.0)
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(25, 26, 29))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 14),
                    )),
            )
            .show(ctx, |ui| {
                show_titlebar(
                    ui,
                    ctx,
                    self.current_folder.as_deref(),
                    &mut self.settings_open,
                    &mut self.maximized,
                );
            });

        // Status bar
        TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            StatusBar::show(
                ui,
                &self.scan_status,
                self.thumbnail_total,
                self.thumbnail_done,
                self.thumbnail_errors,
            );
        });

        if !self.collapsed_sidebar {
            SidePanel::left("library_sidebar")
                .resizable(true)
                .min_width(180.0)
                .default_width(220.0)
                .show(ctx, |ui| {
                    let mut open_folder = false;
                    SidebarWidget::show(
                        ui,
                        &self.entries,
                        self.current_folder.as_deref(),
                        &mut self.library_filter,
                        &mut open_folder,
                    );
                    if open_folder {
                        self.request_folder_picker();
                    }
                });
        } else if self.user_expanded_sidebar {
            egui::Area::new("sidebar_overlay".into())
                .anchor(egui::Align2::LEFT_TOP, egui::Vec2::new(0.0, 36.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_width(220.0);
                        ui.horizontal(|ui| {
                            ui.heading("Library");
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("\u{2715}").clicked() {
                                        self.user_expanded_sidebar = false;
                                        self.collapsed_sidebar = true;
                                    }
                                },
                            );
                        });
                        ui.separator();
                        let mut open_folder = false;
                        SidebarWidget::show(
                            ui,
                            &self.entries,
                            self.current_folder.as_deref(),
                            &mut self.library_filter,
                            &mut open_folder,
                        );
                        if open_folder {
                            self.request_folder_picker();
                        }
                    });
                });
        }

        // Metadata panel — normal when window >= 1024px, overlay when toggled below 1024px
        if !self.collapsed_detail {
            SidePanel::right("metadata_panel")
                .resizable(false)
                .min_width(200.0)
                .default_width(320.0)
                .show(ctx, |ui| {
                    ScrollArea::vertical().show(ui, |ui| {
                        let selected_hash = self.selected_hash;
                        let preview_mesh = selected_hash.and_then(|h| self.meshes.get(&h));
                        let selected = selected_hash
                            .and_then(|h| self.entries.iter_mut().find(|e| e.hash == h));
                        let changed = MetadataPanel::show(
                            ui,
                            selected,
                            preview_mesh,
                            &mut self.detail_preview_yaw,
                            slicer_path_opt(&self.slicer_path),
                        );
                        if changed {
                            self.mark_meta_dirty();
                        }
                    });
                });
        } else if self.user_expanded_detail {
            egui::Area::new("detail_overlay".into())
                .anchor(egui::Align2::RIGHT_TOP, egui::Vec2::new(0.0, 36.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_max_width(320.0);
                        ui.set_min_width(280.0);
                        ui.horizontal(|ui| {
                            ui.heading("Details");
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("\u{2715}").clicked() {
                                        self.user_expanded_detail = false;
                                        self.collapsed_detail = true;
                                    }
                                },
                            );
                        });
                        ui.separator();
                        ScrollArea::vertical()
                            .max_height(ui.ctx().screen_rect().height() - 120.0)
                            .show(ui, |ui| {
                                let selected_hash = self.selected_hash;
                                let preview_mesh = selected_hash.and_then(|h| self.meshes.get(&h));
                                let selected = selected_hash
                                    .and_then(|h| self.entries.iter_mut().find(|e| e.hash == h));
                                let changed = MetadataPanel::show(
                                    ui,
                                    selected,
                                    preview_mesh,
                                    &mut self.detail_preview_yaw,
                                    slicer_path_opt(&self.slicer_path),
                                );
                                if changed {
                                    self.mark_meta_dirty();
                                }
                            });
                    });
                });
        }

        let displayed = crate::view_model::filtered_sorted_entries(
            &self.entries,
            DisplayQuery {
                search_query: &self.search_query,
                library_filter: &self.library_filter,
                sort_by: self.sort_by,
                sort_ascending: self.sort_ascending,
            },
        );

        let detail_width = if self.collapsed_detail { 0.0 } else { 320.0 };
        let grid_width = (win_width - detail_width).max(400.0);
        let cols = ((grid_width / self.density.card_width()).floor() as usize).max(2);

        ctx.input(|i| {
            use egui::Key;
            // Tab / Shift-Tab: cycle focus zones
            if i.key_pressed(Key::Tab) {
                let reverse = i.modifiers.shift;
                self.focus_zone = match (self.focus_zone, reverse) {
                    (FocusZone::Sidebar, false) => FocusZone::Search,
                    (FocusZone::Search, false) => FocusZone::Grid,
                    (FocusZone::Grid, false) => FocusZone::Detail,
                    (FocusZone::Detail, false) => FocusZone::Sidebar,
                    (FocusZone::Sidebar, true) => FocusZone::Detail,
                    (FocusZone::Search, true) => FocusZone::Sidebar,
                    (FocusZone::Grid, true) => FocusZone::Search,
                    (FocusZone::Detail, true) => FocusZone::Grid,
                };
                if self.focus_zone == FocusZone::Search {
                    ctx.memory_mut(|mem| mem.request_focus(egui::Id::new("search_input")));
                }
            }
            // Arrow keys: navigate grid
            if self.focus_zone == FocusZone::Grid && !displayed.is_empty() {
                let cur_idx = self
                    .selected_hash
                    .and_then(|h| displayed.iter().position(|e| e.hash == h))
                    .unwrap_or(0);
                let cur_row_start = (cur_idx / cols) * cols;
                let cur_row_end = ((cur_row_start + cols).min(displayed.len())).saturating_sub(1);
                let new_idx = if i.key_pressed(Key::ArrowRight) {
                    if cur_idx < cur_row_end {
                        Some(cur_idx + 1)
                    } else {
                        Some(cur_row_start)
                    }
                } else if i.key_pressed(Key::ArrowLeft) {
                    if cur_idx > cur_row_start {
                        Some(cur_idx - 1)
                    } else {
                        Some(cur_row_end)
                    }
                } else if i.key_pressed(Key::ArrowDown) {
                    let next = cur_idx + cols;
                    if next < displayed.len() {
                        Some(next)
                    } else {
                        None
                    }
                } else if i.key_pressed(Key::ArrowUp) {
                    Some(cur_idx.saturating_sub(cols))
                } else {
                    None
                };
                if let Some(idx) = new_idx {
                    self.selected_hash = Some(displayed[idx].hash);
                }
            }
            if i.key_pressed(Key::Space) && self.focus_zone == FocusZone::Grid {
                if self.selected_hash.is_some() {
                    self.selected_hash = None;
                } else if let Some(first) = displayed.first() {
                    self.selected_hash = Some(first.hash);
                }
            }
            // Enter: focus grid / confirm
            if i.key_pressed(Key::Enter)
                && self.focus_zone == FocusZone::Grid
                && self.selected_hash.is_some()
            {
                self.focus_zone = FocusZone::Detail;
            }
            // Escape: deselect, return focus to grid
            if i.key_pressed(Key::Escape) && !self.settings_open {
                self.selected_hash = None;
                self.focus_zone = FocusZone::Grid;
            }
        });

        // Main content: center-only toolbar, empty state, grid, masonry, or list
        CentralPanel::default().show(ctx, |ui| {
            egui::Frame::NONE
                .fill(egui::Color32::from_rgb(31, 32, 36))
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ToolbarWidget::show(
                            ui,
                            &mut self.search_query,
                            &mut self.view_mode,
                            &mut self.density,
                            &mut self.sort_by,
                            &mut self.sort_ascending,
                            &mut self.collapsed_sidebar,
                            &mut self.collapsed_detail,
                            &mut self.user_expanded_sidebar,
                            &mut self.user_expanded_detail,
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let add = egui::Button::new(
                                egui::RichText::new(format!("+ {}", strings::MSG_OPEN_FOLDER))
                                    .size(12.5)
                                    .color(egui::Color32::from_rgb(10, 22, 26)),
                            )
                            .fill(egui::Color32::from_rgb(61, 184, 226))
                            .corner_radius(5.0)
                            .min_size(egui::vec2(108.0, 28.0));
                            if ui.add(add).clicked() {
                                self.request_folder_picker();
                            }
                            if ui
                                .button("\u{27F3}")
                                .on_hover_text(strings::MSG_REFRESH)
                                .clicked()
                            {
                                if let Some(ref folder) = self.current_folder.clone() {
                                    self.scan_requested_folder = Some(folder.clone());
                                }
                            }
                        });
                    });

                    if !self.search_query.is_empty() || self.library_filter != LibraryFilter::All {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if let Some(label) = self.filter_label() {
                                let chip = egui::Button::new(
                                    egui::RichText::new(format!("{}  \u{2715}", label))
                                        .size(11.0)
                                        .color(egui::Color32::from_rgb(208, 240, 242)),
                                )
                                .fill(egui::Color32::from_rgb(35, 69, 76))
                                .corner_radius(999.0)
                                .min_size(egui::Vec2::new(0.0, 22.0));
                                if ui.add(chip).clicked() {
                                    self.library_filter = LibraryFilter::All;
                                }
                            }

                            if !self.search_query.is_empty() {
                                let chip_text = format!(
                                    "{}: {}",
                                    strings::MSG_FILTER_CHIP_SEARCH,
                                    self.search_query
                                );
                                let chip = egui::Button::new(
                                    egui::RichText::new(format!("{}  \u{2715}", chip_text))
                                        .size(11.0)
                                        .color(egui::Color32::from_rgb(230, 200, 200)),
                                )
                                .fill(egui::Color32::from_rgb(74, 40, 45))
                                .corner_radius(999.0)
                                .min_size(egui::Vec2::new(0.0, 22.0));

                                if ui.add(chip).clicked() {
                                    self.search_query.clear();
                                }
                            }
                        });
                    } else {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(
                                    self.filter_label()
                                        .unwrap_or_else(|| "All models".to_string()),
                                )
                                .size(11.5)
                                .color(egui::Color32::from_rgb(142, 146, 154)),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new(format!("{} items", displayed.len()))
                                            .monospace()
                                            .size(11.0)
                                            .color(egui::Color32::from_rgb(142, 146, 154)),
                                    );
                                },
                            );
                        });
                    }
                });

            // Show branded empty state when library has never been populated
            let is_empty =
                self.entries.is_empty() && !matches!(self.scan_status, ScanStatus::Scanning { .. });
            if is_empty {
                let mut open_folder = false;
                EmptyState::show(ui, &mut open_folder);
                if open_folder {
                    self.scan_requested_folder = None;
                    self.request_folder_picker();
                }
            } else if let Some(ref placeholder) = self.placeholder_tex.clone() {
                if displayed.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(72.0);
                        ui.label(
                            egui::RichText::new("No matching models")
                                .size(15.0)
                                .color(egui::Color32::from_rgb(220, 224, 228)),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(
                                "Clear search or filters to return to the library.",
                            )
                            .size(12.0)
                            .color(egui::Color32::from_rgb(145, 150, 158)),
                        );
                    });
                    return;
                }
                match self.view_mode {
                    ViewMode::Grid => {
                        GridWidget::show(
                            ui,
                            &displayed,
                            &self.textures,
                            &mut self.selected_hash,
                            placeholder,
                            self.focus_zone,
                            self.density.card_width(),
                        );
                    }
                    ViewMode::List => {
                        ListWidget::show(
                            ui,
                            &displayed,
                            &self.textures,
                            &mut self.selected_hash,
                            placeholder,
                            self.focus_zone,
                        );
                    }
                    ViewMode::Masonry => {
                        MasonryWidget::show(
                            ui,
                            &displayed,
                            &self.textures,
                            &mut self.selected_hash,
                            placeholder,
                            self.focus_zone,
                            self.density.card_width(),
                        );
                    }
                }
            }
        });

        SettingsDialog::show(
            ctx,
            &mut self.settings_open,
            &mut self.settings_tab,
            self.current_folder.as_deref(),
            &mut self.density,
            &mut self.theme,
            &mut self.language,
            &mut self.gpu_thumbnails_enabled,
            &mut self.thumbnail_workers,
            &mut self.slicer_path,
        );

        self.save_preferences_if_needed();
    }
}

fn slicer_path_opt(value: &str) -> Option<&std::path::Path> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(std::path::Path::new(trimmed))
    }
}

fn is_watch_relevant_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "stl" | "3mf" | "obj" | "step" | "stp"
            )
        })
        .unwrap_or(false)
        || path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".modelrack.json"))
}

fn show_titlebar(
    ui: &mut egui::Ui,
    ctx: &Context,
    current_folder: Option<&std::path::Path>,
    settings_open: &mut bool,
    maximized: &mut bool,
) {
    let full = ui.max_rect();
    let drag_response = ui.interact(full, egui::Id::new("titlebar_drag"), egui::Sense::drag());
    if drag_response.drag_started() {
        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }

    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), full.height()),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.set_height(full.height());
            ui.add_space(18.0);
            if traffic_light(
                ui,
                TrafficLightKind::Close,
                egui::Color32::from_rgb(255, 95, 86),
            )
            .clicked()
            {
                crate::macos::hide_application();
            }
            if traffic_light(
                ui,
                TrafficLightKind::Minimize,
                egui::Color32::from_rgb(255, 189, 46),
            )
            .clicked()
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            if traffic_light(
                ui,
                TrafficLightKind::Maximize,
                egui::Color32::from_rgb(39, 201, 63),
            )
            .clicked()
            {
                *maximized = !*maximized;
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(*maximized));
            }
            ui.add_space(14.0);
            draw_app_mark(ui);
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(strings::APP_TITLE)
                    .size(13.0)
                    .color(egui::Color32::from_rgb(205, 208, 214)),
            );
            ui.label(
                egui::RichText::new("—")
                    .size(12.0)
                    .color(egui::Color32::from_rgb(96, 100, 108)),
            );
            ui.label(
                egui::RichText::new(titlebar_path(current_folder))
                    .monospace()
                    .size(11.5)
                    .color(egui::Color32::from_rgb(136, 140, 148)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let settings = egui::Button::new(
                    egui::RichText::new("\u{2727}")
                        .size(14.0)
                        .color(egui::Color32::from_rgb(148, 152, 160)),
                )
                .frame(false)
                .min_size(egui::vec2(26.0, 26.0));
                if ui.add(settings).on_hover_text("Settings").clicked() {
                    *settings_open = true;
                }
            });
        },
    );
}

#[derive(Clone, Copy)]
enum TrafficLightKind {
    Close,
    Minimize,
    Maximize,
}

fn traffic_light(
    ui: &mut egui::Ui,
    kind: TrafficLightKind,
    color: egui::Color32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(14.0, 18.0), egui::Sense::click());
    let center = rect.center();
    let radius = 5.5;
    let fill = if response.hovered() {
        color
    } else {
        color.gamma_multiply(0.92)
    };
    ui.painter().circle_filled(center, radius, fill);
    ui.painter().circle_stroke(
        center,
        radius,
        egui::Stroke::new(0.5, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 80)),
    );

    if response.hovered() {
        let glyph = egui::Color32::from_rgba_unmultiplied(45, 45, 48, 170);
        match kind {
            TrafficLightKind::Close => {
                ui.painter().line_segment(
                    [
                        center + egui::vec2(-2.0, -2.0),
                        center + egui::vec2(2.0, 2.0),
                    ],
                    egui::Stroke::new(1.1, glyph),
                );
                ui.painter().line_segment(
                    [
                        center + egui::vec2(2.0, -2.0),
                        center + egui::vec2(-2.0, 2.0),
                    ],
                    egui::Stroke::new(1.1, glyph),
                );
            }
            TrafficLightKind::Minimize => {
                ui.painter().line_segment(
                    [
                        center + egui::vec2(-2.8, 0.0),
                        center + egui::vec2(2.8, 0.0),
                    ],
                    egui::Stroke::new(1.2, glyph),
                );
            }
            TrafficLightKind::Maximize => {
                let points = [
                    center + egui::vec2(-2.4, 1.9),
                    center + egui::vec2(1.9, 1.9),
                    center + egui::vec2(1.9, -2.4),
                ];
                ui.painter().add(egui::Shape::convex_polygon(
                    points.to_vec(),
                    glyph,
                    egui::Stroke::NONE,
                ));
            }
        }
    }

    response
}

fn draw_app_mark(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
    let center = rect.center() + egui::vec2(0.0, -0.8);
    let s = 4.05;
    let base_y = center.y + 4.2;
    let dx = s * 0.866;
    draw_iso_cube(ui, egui::pos2(center.x - dx, base_y), s, false);
    draw_iso_cube(ui, egui::pos2(center.x + dx, base_y), s, false);
    draw_iso_cube(ui, egui::pos2(center.x, base_y - s), s, true);
}

fn draw_iso_cube(ui: &mut egui::Ui, anchor: egui::Pos2, size: f32, accent: bool) {
    let top = if accent {
        egui::Color32::from_rgb(126, 212, 240)
    } else {
        egui::Color32::from_rgb(78, 182, 216)
    };
    let left = if accent {
        egui::Color32::from_rgb(78, 182, 216)
    } else {
        egui::Color32::from_rgb(59, 143, 176)
    };
    let right = if accent {
        egui::Color32::from_rgb(42, 110, 138)
    } else {
        egui::Color32::from_rgb(31, 86, 112)
    };
    let dx = size * 0.866;
    let top_poly = vec![
        anchor + egui::vec2(0.0, -size),
        anchor + egui::vec2(dx, -size * 0.5),
        anchor,
        anchor + egui::vec2(-dx, -size * 0.5),
    ];
    let left_poly = vec![
        anchor + egui::vec2(-dx, -size * 0.5),
        anchor,
        anchor + egui::vec2(0.0, size),
        anchor + egui::vec2(-dx, size * 0.5),
    ];
    let right_poly = vec![
        anchor + egui::vec2(dx, -size * 0.5),
        anchor,
        anchor + egui::vec2(0.0, size),
        anchor + egui::vec2(dx, size * 0.5),
    ];
    ui.painter().add(egui::Shape::convex_polygon(
        left_poly,
        left,
        egui::Stroke::NONE,
    ));
    ui.painter().add(egui::Shape::convex_polygon(
        right_poly,
        right,
        egui::Stroke::NONE,
    ));
    ui.painter().add(egui::Shape::convex_polygon(
        top_poly.clone(),
        top,
        egui::Stroke::NONE,
    ));
    ui.painter().add(egui::Shape::closed_line(
        top_poly,
        egui::Stroke::new(
            0.35,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 70),
        ),
    ));
}

fn titlebar_path(current_folder: Option<&std::path::Path>) -> String {
    current_folder
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "~/Library/3d".to_string())
}

fn install_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        crate::fonts::INTER_FAMILY.to_string(),
        FontData::from_static(crate::fonts::INTER_VARIABLE).into(),
    );
    fonts.font_data.insert(
        crate::fonts::PRETENDARD_FAMILY.to_string(),
        FontData::from_static(crate::fonts::PRETENDARD_VARIABLE).into(),
    );
    fonts.font_data.insert(
        crate::fonts::MONO_FAMILY.to_string(),
        FontData::from_static(crate::fonts::JETBRAINS_MONO_REGULAR).into(),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, crate::fonts::PRETENDARD_FAMILY.to_string());
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, crate::fonts::INTER_FAMILY.to_string());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, crate::fonts::MONO_FAMILY.to_string());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .push(crate::fonts::PRETENDARD_FAMILY.to_string());
    ctx.set_fonts(fonts);
}

fn configure_egui(ctx: &Context, theme: &str) {
    let mut visuals = if theme == "light" {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    if theme == "light" {
        visuals.window_fill = egui::Color32::from_rgb(244, 245, 247);
        visuals.panel_fill = egui::Color32::from_rgb(238, 240, 243);
        visuals.extreme_bg_color = egui::Color32::from_rgb(252, 252, 253);
        visuals.faint_bg_color = egui::Color32::from_rgb(226, 229, 234);
        visuals.selection.bg_fill = egui::Color32::from_rgb(202, 235, 238);
        visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(20, 124, 132));
    } else {
        visuals.window_fill = egui::Color32::from_rgb(27, 28, 32);
        visuals.panel_fill = egui::Color32::from_rgb(24, 25, 29);
        visuals.extreme_bg_color = egui::Color32::from_rgb(20, 21, 24);
        visuals.faint_bg_color = egui::Color32::from_rgb(36, 38, 43);
        visuals.selection.bg_fill = egui::Color32::from_rgb(40, 88, 94);
        visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(84, 202, 210));
    }
    ctx.set_visuals(visuals);
}

fn preferences_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("ModelRack")
                .join("settings.json")
        })
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(|appdata| {
            PathBuf::from(appdata)
                .join("ModelRack")
                .join("settings.json")
        })
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|base| base.join("ModelRack").join("settings.json"))
    }
}

fn load_preferences() -> AppPrefs {
    let Some(path) = preferences_path() else {
        return AppPrefs::default();
    };
    let Ok(data) = std::fs::read_to_string(path) else {
        return AppPrefs::default();
    };
    serde_json::from_str(&data).unwrap_or_default()
}

fn save_preferences(prefs: &AppPrefs) -> std::io::Result<()> {
    let Some(path) = preferences_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(prefs)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, data)?;
    std::fs::rename(tmp_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_json_round_trips() {
        let prefs = AppPrefs {
            density: "large".to_string(),
            view_mode: "masonry".to_string(),
            gpu_thumbnails_enabled: false,
            theme: "light".to_string(),
            language: "ko".to_string(),
            thumbnail_workers: 2,
            slicer_path: "/Applications/OrcaSlicer.app".to_string(),
            last_folder: Some(PathBuf::from("/tmp/modelrack-library")),
        };

        let json = serde_json::to_string(&prefs).unwrap();
        let loaded: AppPrefs = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded, prefs);
        assert_eq!(Density::from_str(&loaded.density), Density::Large);
        assert_eq!(ViewMode::from_str(&loaded.view_mode), ViewMode::Masonry);
    }

    #[test]
    fn preferences_json_uses_defaults_for_missing_fields() {
        let loaded: AppPrefs = serde_json::from_str("{}").unwrap();

        assert_eq!(Density::from_str(&loaded.density), Density::Medium);
        assert!(loaded.gpu_thumbnails_enabled);
        assert_eq!(loaded.theme, "dark");
        assert_eq!(loaded.language, "en");
        assert_eq!(loaded.thumbnail_workers, 4);
        assert_eq!(loaded.slicer_path, "");
        assert_eq!(loaded.last_folder, None);
    }

    #[test]
    fn folder_watcher_filters_model_and_sidecar_paths() {
        assert!(is_watch_relevant_path(std::path::Path::new("part.stl")));
        assert!(is_watch_relevant_path(std::path::Path::new("part.3mf")));
        assert!(is_watch_relevant_path(std::path::Path::new(
            "part.stl.modelrack.json"
        )));
        assert!(!is_watch_relevant_path(std::path::Path::new("notes.json")));
        assert!(!is_watch_relevant_path(std::path::Path::new("preview.png")));
    }
}
