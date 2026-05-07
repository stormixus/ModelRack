use egui::{Color32, CornerRadius, ScrollArea, Sense, TextureHandle, Ui, Vec2};
use std::collections::HashMap;

use crate::scanner::StlFileInfo;
use crate::strings;
use crate::thumbnail;
use crate::utils;
use crate::view_model::FocusZone;

const PADDING: f32 = 8.0;

pub struct GridWidget;

impl GridWidget {
    pub fn show(
        ui: &mut Ui,
        entries: &[StlFileInfo],
        textures: &HashMap<[u8; 32], TextureHandle>,
        selected_hash: &mut Option<[u8; 32]>,
        placeholder_tex: &TextureHandle,
        focus_zone: FocusZone,
        card_width: f32,
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
            let card_size = Vec2::new(card_width, card_width);
            let thumb_height = (card_width * 0.67).clamp(86.0, 148.0);
            let cols = ((available / (card_width + PADDING)).max(1.0)) as usize;

            egui::Grid::new("thumbnail_grid")
                .spacing(Vec2::new(PADDING, PADDING))
                .show(ui, |ui| {
                    for (i, entry) in entries.iter().enumerate() {
                        let is_selected = *selected_hash == Some(entry.hash);

                        let (rect, response) = ui.allocate_exact_size(card_size, Sense::click());

                        if response.clicked() {
                            *selected_hash = Some(entry.hash);
                        }

                        let bg = if is_selected {
                            Color32::from_rgb(47, 69, 78)
                        } else if response.hovered() {
                            Color32::from_rgb(48, 50, 56)
                        } else {
                            Color32::from_rgb(37, 38, 43)
                        };
                        let stroke = if is_selected {
                            egui::Stroke::new(1.0, Color32::from_rgb(88, 198, 204))
                        } else {
                            egui::Stroke::new(1.0, Color32::from_rgb(61, 63, 70))
                        };
                        ui.painter().rect_filled(rect, CornerRadius::same(6), bg);
                        ui.painter().rect_stroke(
                            rect,
                            CornerRadius::same(6),
                            stroke,
                            egui::StrokeKind::Inside,
                        );

                        // Keyboard focus accent ring
                        if is_selected && focus_zone == FocusZone::Grid {
                            let accent = egui::Stroke::new(2.0, Color32::from_rgb(80, 200, 200));
                            ui.painter().rect_stroke(
                                rect,
                                CornerRadius::same(6),
                                accent,
                                egui::StrokeKind::Outside,
                            );
                        }

                        // Thumbnail image area
                        let img_rect = egui::Rect::from_min_size(
                            rect.min + Vec2::new(4.0, 4.0),
                            Vec2::new(card_width - 8.0, thumb_height),
                        );

                        let texture = textures.get(&entry.hash).unwrap_or(placeholder_tex);
                        let uv =
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                        ui.painter()
                            .image(texture.id(), img_rect, uv, Color32::WHITE);

                        if entry.stl_type == crate::scanner::StlType::Unknown {
                            let badge = egui::Rect::from_min_size(
                                img_rect.min + Vec2::new(6.0, 6.0),
                                Vec2::new(32.0, 18.0),
                            );
                            ui.painter().rect_filled(
                                badge,
                                CornerRadius::same(4),
                                Color32::from_rgb(138, 50, 48),
                            );
                            ui.painter().text(
                                badge.center(),
                                egui::Align2::CENTER_CENTER,
                                "ERR",
                                egui::FontId::monospace(10.0),
                                Color32::WHITE,
                            );
                        } else if let Some(label) = type_badge_label(entry.stl_type) {
                            let badge = egui::Rect::from_min_size(
                                img_rect.min + Vec2::new(6.0, 6.0),
                                Vec2::new(44.0, 18.0),
                            );
                            ui.painter().rect_filled(
                                badge,
                                CornerRadius::same(4),
                                Color32::from_rgba_unmultiplied(18, 20, 24, 218),
                            );
                            ui.painter().text(
                                badge.center(),
                                egui::Align2::CENTER_CENTER,
                                label,
                                egui::FontId::monospace(10.0),
                                Color32::from_rgb(218, 226, 230),
                            );
                        }

                        if let Some(meta) = &entry.meta {
                            let mut badge_x = img_rect.right() - 6.0;
                            if meta.favorite {
                                badge_x -= 22.0;
                                draw_badge(ui, egui::pos2(badge_x, img_rect.min.y + 6.0), "★");
                            }
                            if meta.printed > 0 {
                                let label = if meta.printed > 9 {
                                    "9+".to_string()
                                } else {
                                    meta.printed.to_string()
                                };
                                badge_x -= 26.0;
                                draw_badge(ui, egui::pos2(badge_x, img_rect.min.y + 6.0), &label);
                            }
                            if !meta.tags.is_empty() {
                                badge_x -= 22.0;
                                draw_badge(ui, egui::pos2(badge_x, img_rect.min.y + 6.0), "●");
                            }
                        }

                        // Filename
                        let truncated = utils::truncate_filename(&entry.filename, 22);
                        let text_pos = egui::pos2(rect.min.x + 4.0, img_rect.max.y + 2.0);
                        ui.painter().text(
                            text_pos,
                            egui::Align2::LEFT_TOP,
                            truncated,
                            egui::FontId::proportional(12.0),
                            if is_selected {
                                Color32::WHITE
                            } else {
                                Color32::from_rgb(218, 220, 224)
                            },
                        );

                        let meta_text = format!(
                            "{} · {}",
                            format_size(entry.size),
                            entry
                                .triangle_count
                                .map(format_count)
                                .unwrap_or_else(|| "— tris".to_string())
                        );
                        ui.painter().text(
                            egui::pos2(rect.min.x + 4.0, img_rect.max.y + 20.0),
                            egui::Align2::LEFT_TOP,
                            meta_text,
                            egui::FontId::monospace(10.5),
                            Color32::from_rgb(144, 148, 156),
                        );

                        // Full filename tooltip on hover
                        if response.hovered() && entry.filename.len() > 22 {
                            response.on_hover_text(entry.filename.clone());
                        }

                        if (i + 1) % cols == 0 {
                            ui.end_row();
                        }
                    }
                });
        });
    }
}

fn format_count(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M tris", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K tris", n as f64 / 1_000.0)
    } else {
        format!("{} tris", n)
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

fn draw_badge(ui: &mut Ui, min: egui::Pos2, label: &str) {
    let rect = egui::Rect::from_min_size(min, Vec2::new(22.0, 18.0));
    ui.painter()
        .rect_filled(rect, CornerRadius::same(4), Color32::from_rgb(32, 78, 84));
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(4),
        egui::Stroke::new(1.0, Color32::from_rgb(78, 158, 164)),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(10.5),
        Color32::from_rgb(225, 244, 244),
    );
}

fn type_badge_label(stl_type: crate::scanner::StlType) -> Option<&'static str> {
    match stl_type {
        crate::scanner::StlType::Binary => Some("STL"),
        crate::scanner::StlType::Ascii => Some("ASCII"),
        crate::scanner::StlType::ThreeMf => Some("3MF"),
        crate::scanner::StlType::Obj => Some("OBJ"),
        crate::scanner::StlType::Step => Some("STEP"),
        crate::scanner::StlType::LargeStl => Some("LARGE"),
        crate::scanner::StlType::Unknown => None,
    }
}

/// Generate a placeholder texture to reuse for entries with no real thumbnail
pub fn make_placeholder_texture(ctx: &egui::Context) -> TextureHandle {
    let color_image = thumbnail::generate_placeholder(160, 112);
    ctx.load_texture("placeholder", color_image, egui::TextureOptions::LINEAR)
}
