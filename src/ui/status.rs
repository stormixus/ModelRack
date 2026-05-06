use egui::{Color32, RichText, Ui};

use crate::app::ScanStatus;
use crate::strings;

pub struct StatusBar;

impl StatusBar {
    pub fn show(ui: &mut Ui, status: &ScanStatus) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            match status {
                ScanStatus::Idle => {
                    ui.label(
                        RichText::new("Open a folder to begin")
                            .color(ui.ctx().style().visuals.weak_text_color()),
                    );
                }
                ScanStatus::Scanning { found, skipped } => {
                    let text = format!(
                        "{} {} ({})",
                        strings::MSG_SCANNING,
                        format_found(*found),
                        format_skipped(*skipped),
                    );
                    ui.label(RichText::new(text).color(Color32::from_rgb(200, 180, 80)));
                }
                ScanStatus::Done { found, skipped } => {
                    let mut parts = vec![format_found(*found)];
                    if *skipped > 0 {
                        parts.push(format_skipped(*skipped));
                    }
                    ui.label(
                        RichText::new(parts.join("  "))
                            .color(ui.ctx().style().visuals.weak_text_color()),
                    );
                }
                ScanStatus::Error(ref msg) => {
                    ui.label(RichText::new(msg).color(Color32::from_rgb(220, 80, 80)));
                }
            }
        });
    }
}

fn format_found(count: usize) -> String {
    match count {
        0 => "0 files".into(),
        1 => "1 file".into(),
        n => format!("{} files", n),
    }
}

fn format_skipped(count: usize) -> String {
    format!("{} {}", count, strings::MSG_SKIPPED)
}
