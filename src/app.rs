use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

use eframe::Frame;
use egui::{CentralPanel, Context, ScrollArea, SidePanel, TextureHandle, TopBottomPanel};

use crate::scanner;
use crate::strings;
use crate::ui::grid::{self, GridWidget};
use crate::ui::metadata::MetadataPanel;
use crate::ui::status::StatusBar;
use crate::worker::{self, ThumbnailJob, ThumbnailResult};

pub enum ScanStatus {
    Idle,
    Scanning { found: usize, skipped: usize },
    Done { found: usize, skipped: usize },
    Error(String),
}

pub struct AppState {
    pub entries: Vec<scanner::StlFileInfo>,
    pub textures: HashMap<[u8; 32], TextureHandle>,
    pub selected_hash: Option<[u8; 32]>,
    pub scan_status: ScanStatus,
    pub current_folder: Option<PathBuf>,
    placeholder_tex: Option<TextureHandle>,
    scan_requested_folder: Option<PathBuf>,
    folder_picker_rx: Option<mpsc::Receiver<Option<PathBuf>>>,
    thumbnail_result_rx: Option<crossbeam_channel::Receiver<ThumbnailResult>>,
    scan_result_rx: Option<crossbeam_channel::Receiver<scanner::ScanResult>>,
    cancel_token: Option<Arc<AtomicBool>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            textures: HashMap::new(),
            selected_hash: None,
            scan_status: ScanStatus::Idle,
            current_folder: None,
            placeholder_tex: None,
            scan_requested_folder: None,
            folder_picker_rx: None,
            thumbnail_result_rx: None,
            scan_result_rx: None,
            cancel_token: None,
        }
    }
}

impl AppState {
    fn start_scan(&mut self, ctx: &Context, folder: PathBuf) {
        self.current_folder = Some(folder.clone());

        // (#2) Check folder existence BEFORE spawning scan thread
        if !folder.exists() {
            self.scan_status = ScanStatus::Error(strings::MSG_FOLDER_NOT_FOUND.to_string());
            ctx.request_repaint();
            return;
        }

        // (#4) Clear old texture cache to prevent memory leaks
        self.textures.clear();

        // (#3) Cancel any running workers from previous scan
        if let Some(ref token) = self.cancel_token {
            token.store(true, Ordering::SeqCst);
        }
        self.thumbnail_result_rx = None;

        self.scan_status = ScanStatus::Scanning {
            found: 0,
            skipped: 0,
        };
        ctx.request_repaint();

        // (#1) Move scan to worker thread — eliminate UI freeze
        let (tx, rx) = crossbeam_channel::bounded(1);
        std::thread::spawn(move || {
            let result = scanner::scan_folder(&folder);
            let _ = tx.send(result);
        });

        self.scan_result_rx = Some(rx);
    }

    fn process_scan_result(&mut self, ctx: &Context, result: scanner::ScanResult) {
        // Generate placeholder textures for all entries
        for entry in &result.entries {
            let color_image = crate::thumbnail::generate_placeholder(160, 112);
            let handle = ctx.load_texture(
                format!("thumb_{:x?}", &entry.hash[..8]),
                color_image,
                egui::TextureOptions::LINEAR,
            );
            self.textures.insert(entry.hash, handle);
        }

        // Spawn thumbnail workers with fresh cancel token
        if !result.meshes.is_empty() {
            let token = Arc::new(AtomicBool::new(false));
            let (job_tx, job_rx) =
                crossbeam_channel::bounded::<ThumbnailJob>(result.meshes.len());
            let (result_tx, result_rx) =
                crossbeam_channel::bounded::<ThumbnailResult>(result.meshes.len());

            for mesh in result.meshes {
                let _ = job_tx.send(ThumbnailJob::from(mesh));
            }
            drop(job_tx);

            let num_workers = rayon::current_num_threads().min(4);
            worker::spawn_thumbnail_workers(job_rx, result_tx, num_workers, token.clone());

            self.cancel_token = Some(token);
            self.thumbnail_result_rx = Some(result_rx);
        } else {
            self.cancel_token = None;
        }

        let found = result.entries.len();
        let skipped = result.skipped;

        self.entries = result.entries;
        self.scan_status = ScanStatus::Done { found, skipped };

        if let Some(sel) = self.selected_hash {
            if !self.entries.iter().any(|e| e.hash == sel) {
                self.selected_hash = None;
            }
        }

        ctx.request_repaint();
    }

    fn selected_entry(&self) -> Option<&scanner::StlFileInfo> {
        self.selected_hash
            .and_then(|h| self.entries.iter().find(|e| e.hash == h))
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        if self.placeholder_tex.is_none() {
            self.placeholder_tex = Some(grid::make_placeholder_texture(ctx));
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

        // Process pending scan request
        if let Some(folder) = self.scan_requested_folder.take() {
            self.start_scan(ctx, folder);
        }

        // (#1) Poll for scan result from worker thread
        if let Some(ref rx) = self.scan_result_rx {
            if let Ok(result) = rx.try_recv() {
                self.scan_result_rx = None;
                self.process_scan_result(ctx, result);
            }
        }

        // Poll for completed thumbnails from workers
        if let Some(ref rx) = self.thumbnail_result_rx {
            let mut updated = false;
            for result in rx.try_iter() {
                match result {
                    ThumbnailResult::Success { hash, image } => {
                        let handle = ctx.load_texture(
                            format!("thumb_{:x?}", &hash[..8]),
                            image,
                            egui::TextureOptions::LINEAR,
                        );
                        self.textures.insert(hash, handle);
                        updated = true;
                    }
                    ThumbnailResult::Error { hash, message } => {
                        eprintln!("Thumbnail error for {:x?}: {}", &hash[..8], message);
                        let error_img =
                            crate::thumbnail::generate_error_placeholder(160, 112);
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

        // Top bar
        TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(strings::APP_TITLE);

                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        if ui.button(strings::MSG_REFRESH).clicked() {
                            if let Some(ref folder) = self.current_folder.clone() {
                                self.scan_requested_folder = Some(folder.clone());
                            }
                        }

                        if ui.button(strings::MSG_OPEN_FOLDER).clicked() {
                            let (tx, rx) = mpsc::channel();
                            self.folder_picker_rx = Some(rx);
                            std::thread::spawn(move || {
                                let folder = rfd::FileDialog::new().pick_folder();
                                let _ = tx.send(folder);
                            });
                        }
                    },
                );
            });
        });

        // Status bar
        TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            StatusBar::show(ui, &self.scan_status);
        });

        // Metadata panel
        SidePanel::right("metadata_panel")
            .resizable(false)
            .min_width(200.0)
            .default_width(220.0)
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    MetadataPanel::show(ui, self.selected_entry());
                });
            });

        // Thumbnail grid
        CentralPanel::default().show(ctx, |ui| {
            if let Some(ref placeholder) = self.placeholder_tex.clone() {
                GridWidget::show(
                    ui,
                    &self.entries,
                    &self.textures,
                    &mut self.selected_hash,
                    placeholder,
                );
            }
        });
    }
}
