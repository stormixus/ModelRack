use std::collections::HashMap;

use egui::{Color32, ScrollArea, Sense, TextureHandle, Ui, Vec2};

use crate::scanner::StlFileInfo;
use crate::strings;
use crate::utils;
use crate::view_model::FocusZone;

const ROW_HEIGHT: f32 = 44.0;
const THUMB_SIZE: f32 = 40.0;

pub struct ListWidget;

impl ListWidget {
    pub fn show(
        ui: &mut Ui,
        entries: &[StlFileInfo],
        textures: &HashMap<[u8; 32], TextureHandle>,
        selected_hash: &mut Option<[u8; 32]>,
        placeholder_tex: &TextureHandle,
        focus_zone: FocusZone,
    ) {
        if entries.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.label(strings::MSG_EMPTY_FOLDER);
            });
            return;
        }

        let available = ui.available_width();

        ScrollArea::vertical().show(ui, |ui| {
            // Header row
            ui.horizontal(|ui| {
                ui.set_height(24.0);
                ui.label(
                    egui::RichText::new(strings::MSG_LIST_HEADER_NAME)
                        .size(11.0)
                        .color(Color32::from_rgb(160, 160, 165)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(strings::MSG_LIST_HEADER_FORMAT)
                            .size(11.0)
                            .color(Color32::from_rgb(160, 160, 165)),
                    );
                });
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(strings::MSG_LIST_HEADER_TRIS)
                        .size(11.0)
                        .color(Color32::from_rgb(160, 160, 165)),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(strings::MSG_LIST_HEADER_SIZE)
                        .size(11.0)
                        .color(Color32::from_rgb(160, 160, 165)),
                );
            });
            ui.separator();

            for entry in entries {
                let is_selected = *selected_hash == Some(entry.hash);

                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(available, ROW_HEIGHT), Sense::click());

                if response.clicked() {
                    *selected_hash = Some(entry.hash);
                }

                let bg = if is_selected {
                    Color32::from_rgb(55, 70, 90)
                } else if response.hovered() {
                    Color32::from_rgb(48, 48, 53)
                } else {
                    Color32::from_rgb(38, 38, 43)
                };
                ui.painter().rect_filled(rect, 2.0, bg);

                // Keyboard focus accent bar
                if is_selected && focus_zone == FocusZone::Grid {
                    let bar_rect = egui::Rect::from_min_size(
                        rect.min + egui::Vec2::new(6.0, 0.0),
                        egui::Vec2::new(2.0, ROW_HEIGHT),
                    );
                    ui.painter()
                        .rect_filled(bar_rect, 0.0, Color32::from_rgb(80, 200, 200));
                }

                // Thumbnail
                let thumb_rect = egui::Rect::from_min_size(
                    rect.min + Vec2::new(6.0, 4.0),
                    Vec2::new(THUMB_SIZE, THUMB_SIZE),
                );
                let texture = textures.get(&entry.hash).unwrap_or(placeholder_tex);
                ui.painter().image(
                    texture.id(),
                    thumb_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );

                // Filename
                let mut display_name = entry.filename.clone();
                if entry.meta.as_ref().is_some_and(|meta| meta.favorite) {
                    display_name = format!("★ {}", display_name);
                }
                let truncated = utils::truncate_filename(&display_name, 40);
                let text_pos = egui::pos2(thumb_rect.max.x + 8.0, rect.center().y - 7.0);
                ui.painter().text(
                    text_pos,
                    egui::Align2::LEFT_TOP,
                    truncated,
                    egui::FontId::proportional(12.5),
                    if is_selected {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(220, 220, 220)
                    },
                );
                if response.hovered() && display_name.len() > 40 {
                    response.on_hover_text(entry.filename.clone());
                }

                // Right-side metadata
                let right_x = rect.right() - 8.0;
                let mono_font = egui::FontId::monospace(11.0);
                let meta_color = Color32::from_rgb(180, 180, 185);

                // Format
                let format_text = match entry.stl_type {
                    crate::scanner::StlType::Binary => "Binary",
                    crate::scanner::StlType::Ascii => "ASCII",
                    crate::scanner::StlType::ThreeMf => "3MF",
                    crate::scanner::StlType::Obj => "OBJ",
                    crate::scanner::StlType::Step => "STEP",
                    crate::scanner::StlType::LargeStl => "STL >100MB",
                    crate::scanner::StlType::Unknown => "?",
                };
                ui.painter().text(
                    egui::pos2(right_x - 240.0, rect.center().y - 6.0),
                    egui::Align2::LEFT_CENTER,
                    format_text,
                    mono_font.clone(),
                    meta_color,
                );

                // Triangle count
                let tris_text = match entry.triangle_count {
                    Some(n) => format_number(n as u64),
                    None => "—".to_string(),
                };
                ui.painter().text(
                    egui::pos2(right_x - 160.0, rect.center().y - 6.0),
                    egui::Align2::LEFT_CENTER,
                    tris_text,
                    mono_font.clone(),
                    meta_color,
                );

                // File size
                let size_text = format_size(entry.size);
                ui.painter().text(
                    egui::pos2(right_x - 70.0, rect.center().y - 6.0),
                    egui::Align2::LEFT_CENTER,
                    size_text,
                    mono_font,
                    meta_color,
                );

                if let Some(meta) = &entry.meta {
                    if meta.printed > 0 {
                        ui.painter().text(
                            egui::pos2(right_x - 310.0, rect.center().y - 6.0),
                            egui::Align2::LEFT_CENTER,
                            format!("printed {}", meta.printed),
                            egui::FontId::monospace(10.5),
                            Color32::from_rgb(120, 198, 154),
                        );
                    }
                }
            }
        });
    }
}

fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1_024 {
        format!("{:.1} KB", bytes as f64 / 1_024.0)
    } else {
        format!("{} B", bytes)
    }
}
