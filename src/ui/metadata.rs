use egui::{RichText, Ui};

use crate::scanner::{StlFileInfo, StlType};
use crate::strings;

pub struct MetadataPanel;

impl MetadataPanel {
    pub fn show(ui: &mut Ui, selected: Option<&StlFileInfo>) {
        ui.heading("Details");

        match selected {
            None => {
                ui.add_space(12.0);
                ui.label(RichText::new(strings::MSG_NO_MODEL_SELECTED).color(
                    ui.ctx().style().visuals.weak_text_color(),
                ));
            }
            Some(entry) => {
                ui.add_space(8.0);

                // Filename
                ui.label(RichText::new("Name").strong());
                ui.label(&entry.filename);
                ui.add_space(6.0);

                // File size
                ui.label(RichText::new("Size").strong());
                ui.label(format_size(entry.size));
                ui.add_space(6.0);

                // Format
                ui.label(RichText::new("Format").strong());
                ui.label(match entry.stl_type {
                    StlType::Binary => "Binary STL",
                    StlType::Ascii => "ASCII STL",
                    StlType::Unknown => "Unknown / Unparseable",
                });
                ui.add_space(6.0);

                // Triangle count
                if let Some(count) = entry.triangle_count {
                    ui.label(RichText::new("Triangles").strong());
                    ui.label(format_count(count));
                    ui.add_space(6.0);
                }

                // Dimensions
                if let Some(dims) = entry.dimensions {
                    ui.label(RichText::new("Dimensions").strong());
                    ui.label(format!(
                        "{:.1} x {:.1} x {:.1} mm",
                        dims[0], dims[1], dims[2]
                    ));
                    ui.add_space(6.0);
                }

                // Path
                ui.label(RichText::new("Path").strong());
                ui.label(
                    RichText::new(entry.path.display().to_string())
                        .color(ui.ctx().style().visuals.weak_text_color()),
                );
            }
        }
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn format_count(n: usize) -> String {
    if n < 1000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{:.1}K", n as f64 / 1000.0)
    } else {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    }
}
