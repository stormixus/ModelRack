use egui::{Color32, RichText};
use std::path::Path;

use crate::view_model::Density;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Appearance,
    Library,
    Thumbnails,
    Slicer,
    Advanced,
    About,
}

pub struct SettingsDialog;

impl SettingsDialog {
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        ctx: &egui::Context,
        open: &mut bool,
        tab: &mut SettingsTab,
        current_folder: Option<&Path>,
        density: &mut Density,
        theme: &mut String,
        language: &mut String,
        gpu_enabled: &mut bool,
        worker_count: &mut usize,
        slicer_path: &mut String,
    ) {
        if !*open {
            return;
        }

        let mut is_open = *open;
        egui::Window::new("Settings")
            .id(egui::Id::new("settings_dialog"))
            .open(&mut is_open)
            .resizable(false)
            .collapsible(false)
            .default_width(760.0)
            .default_height(540.0)
            .show(ctx, |ui| {
                ui.set_min_size(egui::vec2(740.0, 500.0));
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(180.0);
                        ui.add_space(4.0);
                        ui.label(RichText::new("ModelRack").size(13.0).strong());
                        ui.label(
                            RichText::new("v0.0.3")
                                .monospace()
                                .size(11.0)
                                .color(Color32::from_rgb(135, 138, 146)),
                        );
                        ui.add_space(12.0);

                        tab_button(ui, tab, SettingsTab::General, "General");
                        tab_button(ui, tab, SettingsTab::Appearance, "Appearance");
                        tab_button(ui, tab, SettingsTab::Library, "Library");
                        tab_button(ui, tab, SettingsTab::Thumbnails, "Thumbnails");
                        tab_button(ui, tab, SettingsTab::Slicer, "Slicer");
                        tab_button(ui, tab, SettingsTab::Advanced, "Advanced");
                        tab_button(ui, tab, SettingsTab::About, "About");
                    });

                    ui.separator();

                    ui.vertical(|ui| {
                        ui.set_width(520.0);
                        ui.add_space(4.0);
                        ui.heading(tab.title());
                        ui.separator();
                        ui.add_space(8.0);

                        match tab {
                            SettingsTab::General => general(ui, language),
                            SettingsTab::Appearance => appearance(ui, theme),
                            SettingsTab::Library => library(ui, current_folder),
                            SettingsTab::Thumbnails => thumbnails(ui, density),
                            SettingsTab::Slicer => slicer(ui, slicer_path),
                            SettingsTab::Advanced => advanced(ui, gpu_enabled, worker_count),
                            SettingsTab::About => about(ui),
                        }
                    });
                });
            });

        *open = is_open;
    }
}

impl SettingsTab {
    fn title(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Appearance => "Appearance",
            Self::Library => "Library",
            Self::Thumbnails => "Thumbnails",
            Self::Slicer => "Slicer",
            Self::Advanced => "Advanced",
            Self::About => "About",
        }
    }
}

fn tab_button(ui: &mut egui::Ui, active: &mut SettingsTab, tab: SettingsTab, label: &str) {
    let selected = *active == tab;
    let button = egui::Button::new(RichText::new(label).size(12.5).color(if selected {
        Color32::from_rgb(235, 242, 244)
    } else {
        Color32::from_rgb(180, 184, 192)
    }))
    .fill(if selected {
        Color32::from_rgb(42, 70, 76)
    } else {
        Color32::TRANSPARENT
    })
    .corner_radius(6.0)
    .min_size(egui::vec2(160.0, 28.0));

    if ui.add(button).clicked() {
        *active = tab;
    }
}

fn general(ui: &mut egui::Ui, language: &mut String) {
    ui.horizontal(|ui| {
        ui.set_height(34.0);
        ui.label(RichText::new("Language").size(12.5).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            language_button(ui, language, "ja", "日本語");
            language_button(ui, language, "ko", "한국어");
            language_button(ui, language, "en", "English");
        });
    });
    ui.separator();
    row(ui, "Startup", "Reopen the last library folder on launch.");
    row(
        ui,
        "Shortcuts",
        "Cmd-F focuses search. Cmd-, opens this settings window.",
    );
}

fn appearance(ui: &mut egui::Ui, theme: &mut String) {
    ui.horizontal(|ui| {
        ui.set_height(34.0);
        ui.label(RichText::new("Theme").size(12.5).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            theme_button(ui, theme, "light", "Light");
            theme_button(ui, theme, "dark", "Dark");
        });
    });
    ui.separator();
    row(
        ui,
        "Accent",
        "Teal is active. Violet, orange, and green variants are planned.",
    );
    row(
        ui,
        "Typography",
        "UI text follows the compact desktop sizing from the mockup.",
    );
}

fn library(ui: &mut egui::Ui, current_folder: Option<&Path>) {
    row(
        ui,
        "Current folder",
        &current_folder
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "No folder selected".to_string()),
    );
    row(ui, "Recursive scan", "Enabled for nested STL libraries.");
    row(
        ui,
        "Metadata",
        "Sidecar .stl.modelrack.json files are saved next to models.",
    );
}

fn thumbnails(ui: &mut egui::Ui, density: &mut Density) {
    row(
        ui,
        "Style",
        "Use Metal only for 3D model thumbnails, with wireframe fallback.",
    );
    ui.horizontal(|ui| {
        ui.set_height(34.0);
        ui.label(RichText::new("Density").size(12.5).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            density_button(ui, density, Density::Large, "L");
            density_button(ui, density, Density::Medium, "M");
            density_button(ui, density, Density::Small, "S");
        });
    });
    ui.separator();
    row(ui, "Failures", "Unparseable thumbnails show an ERR badge.");
}

fn slicer(ui: &mut egui::Ui, slicer_path: &mut String) {
    row(
        ui,
        "Default slicer",
        if slicer_path.trim().is_empty() {
            "System default STL opener"
        } else {
            "Configured executable path"
        },
    );
    ui.horizontal(|ui| {
        ui.set_height(34.0);
        ui.label(RichText::new("Path").size(12.5).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::TextEdit::singleline(slicer_path)
                    .hint_text("/Applications/OrcaSlicer.app or executable")
                    .desired_width(320.0),
            );
        });
    });
    ui.separator();
}

fn advanced(ui: &mut egui::Ui, gpu_enabled: &mut bool, worker_count: &mut usize) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("GPU thumbnails").size(12.5).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.checkbox(gpu_enabled, "");
        });
    });
    ui.separator();
    *worker_count = (*worker_count).clamp(1, 8);
    ui.horizontal(|ui| {
        ui.set_height(34.0);
        ui.label(RichText::new("Workers").size(12.5).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::Slider::new(worker_count, 1..=8)
                    .show_value(true)
                    .text("thumbnail threads"),
            );
        });
    });
    ui.separator();
    row(
        ui,
        "Privacy",
        "Local files stay local; no telemetry path is implemented.",
    );
}

fn about(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.label(RichText::new("ModelRack").size(22.0).strong());
        ui.label("A workshop tool for your 3D model library");
        ui.add_space(18.0);
    });
    row(ui, "Version", "v0.0.3 alpha");
    row(ui, "Stack", "Rust prototype; Slint UI planned");
    row(ui, "Renderer", "wgpu limited to 3D thumbnails/previews");
    row(ui, "Typography", "Bundled Inter + Pretendard planned");
    row(ui, "License", "MIT");
}

fn row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.set_height(34.0);
        ui.label(RichText::new(label).size(12.5).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(value)
                    .size(11.5)
                    .color(Color32::from_rgb(150, 154, 162)),
            );
        });
    });
    ui.separator();
}

fn density_button(ui: &mut egui::Ui, density: &mut Density, value: Density, label: &str) {
    if ui
        .selectable_label(*density == value, label)
        .on_hover_text(format!("{:.0}px grid cards", value.card_width()))
        .clicked()
    {
        *density = value;
    }
}

fn theme_button(ui: &mut egui::Ui, theme: &mut String, value: &str, label: &str) {
    if ui
        .selectable_label(theme == value, label)
        .on_hover_text(format!("Use {} theme", label.to_lowercase()))
        .clicked()
    {
        *theme = value.to_string();
    }
}

fn language_button(ui: &mut egui::Ui, language: &mut String, value: &str, label: &str) {
    if ui
        .selectable_label(language == value, label)
        .on_hover_text(format!("Use {}", label))
        .clicked()
    {
        *language = value.to_string();
    }
}
