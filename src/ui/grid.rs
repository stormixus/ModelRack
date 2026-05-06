use egui::{Color32, ScrollArea, Sense, TextureHandle, Ui, Vec2};
use std::collections::HashMap;

use crate::scanner::StlFileInfo;
use crate::strings;
use crate::thumbnail;
use crate::utils;

const THUMB_SIZE: Vec2 = Vec2::new(160.0, 140.0);
const PADDING: f32 = 8.0;

pub struct GridWidget;

impl GridWidget {
    pub fn show(
        ui: &mut Ui,
        entries: &[StlFileInfo],
        textures: &HashMap<[u8; 32], TextureHandle>,
        selected_hash: &mut Option<[u8; 32]>,
        placeholder_tex: &TextureHandle,
    ) {
        if entries.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.label(strings::MSG_EMPTY_FOLDER);
            });
            return;
        }

        ScrollArea::vertical().show(ui, |ui| {
            let available = ui.available_width();
            let cols = ((available / (THUMB_SIZE.x + PADDING)).max(1.0)) as usize;

            egui::Grid::new("thumbnail_grid")
                .spacing(Vec2::new(PADDING, PADDING))
                .show(ui, |ui| {
                    for (i, entry) in entries.iter().enumerate() {
                        let is_selected = *selected_hash == Some(entry.hash);

                        let (rect, response) = ui.allocate_exact_size(
                            THUMB_SIZE,
                            Sense::click(),
                        );

                        if response.clicked() {
                            *selected_hash = Some(entry.hash);
                        }

                        let bg = if is_selected {
                            Color32::from_rgb(70, 90, 110)
                        } else {
                            Color32::from_rgb(45, 45, 50)
                        };
                        let stroke = if is_selected {
                            egui::Stroke::new(2.0, Color32::from_rgb(130, 160, 200))
                        } else {
                            egui::Stroke::new(1.0, Color32::from_rgb(70, 70, 75))
                        };
                        ui.painter()
                            .rect_filled(rect, 4.0, bg);
                        ui.painter()
                            .rect_stroke(rect, 4.0, stroke, egui::StrokeKind::Inside);

                        // Thumbnail image area
                        let img_rect = egui::Rect::from_min_size(
                            rect.min + Vec2::new(4.0, 4.0),
                            Vec2::new(THUMB_SIZE.x - 8.0, THUMB_SIZE.y - 28.0),
                        );

                        let texture = textures.get(&entry.hash).unwrap_or(placeholder_tex);
                        let uv = egui::Rect::from_min_max(
                            egui::pos2(0.0, 0.0),
                            egui::pos2(1.0, 1.0),
                        );
                        ui.painter()
                            .image(texture.id(), img_rect, uv, Color32::WHITE);

                        // Filename
                        let truncated = utils::truncate_filename(&entry.filename, 22);
                        let text_pos = egui::pos2(
                            rect.min.x + 4.0,
                            img_rect.max.y + 2.0,
                        );
                        ui.painter().text(
                            text_pos,
                            egui::Align2::LEFT_TOP,
                            truncated,
                            egui::FontId::proportional(12.0),
                            if is_selected {
                                Color32::WHITE
                            } else {
                                Color32::from_rgb(200, 200, 200)
                            },
                        );

                        // Full filename tooltip on hover
                        if response.hovered() && entry.filename.len() > 22 {
                            response
                                .on_hover_text(entry.filename.clone());
                        }

                        if (i + 1) % cols == 0 {
                            ui.end_row();
                        }
                    }
                });
        });
    }
}

/// Generate a placeholder texture to reuse for entries with no real thumbnail
pub fn make_placeholder_texture(
    ctx: &egui::Context,
) -> TextureHandle {
    let color_image = thumbnail::generate_placeholder(160, 112);
    ctx.load_texture(
        "placeholder",
        color_image,
        egui::TextureOptions::LINEAR,
    )
}
