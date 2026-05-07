use std::collections::HashMap;

use egui::{Color32, CornerRadius, ScrollArea, Sense, TextureHandle, Ui, Vec2};

use crate::scanner::StlFileInfo;
use crate::strings;
use crate::utils;
use crate::view_model::FocusZone;

const PADDING: f32 = 8.0;

pub struct MasonryWidget;

impl MasonryWidget {
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
            let cols = ((available / (card_width + PADDING)).max(1.0)) as usize;
            let mut col_heights = vec![0.0_f32; cols];
            let start = ui.cursor().min;
            let row_gap = PADDING;

            for entry in entries {
                let col = col_heights
                    .iter()
                    .enumerate()
                    .min_by(|a, b| a.1.total_cmp(b.1))
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                let height = masonry_height(entry, card_width);
                let min = egui::pos2(
                    start.x + col as f32 * (card_width + PADDING),
                    start.y + col_heights[col],
                );
                let rect = egui::Rect::from_min_size(min, Vec2::new(card_width, height));
                col_heights[col] += height + row_gap;
                let response = ui.allocate_rect(rect, Sense::click());

                if response.clicked() {
                    *selected_hash = Some(entry.hash);
                }

                draw_card(
                    ui,
                    rect,
                    entry,
                    textures.get(&entry.hash).unwrap_or(placeholder_tex),
                    *selected_hash == Some(entry.hash),
                    response.hovered(),
                    focus_zone,
                );
                if response.hovered() && entry.filename.len() > 24 {
                    response.on_hover_text(entry.filename.clone());
                }
            }

            let total_height = col_heights.into_iter().fold(0.0, f32::max);
            ui.allocate_space(Vec2::new(available, total_height.max(1.0)));
        });
    }
}

fn masonry_height(entry: &StlFileInfo, card_width: f32) -> f32 {
    let base = card_width * 0.82;
    let tri_factor = entry
        .triangle_count
        .map(|tris| ((tris as f32 + 1.0).log10() - 3.0).clamp(0.0, 2.0) * 16.0)
        .unwrap_or(0.0);
    let tag_factor = entry
        .meta
        .as_ref()
        .map(|meta| meta.tags.len().min(3) as f32 * 10.0)
        .unwrap_or(0.0);
    (base + tri_factor + tag_factor).clamp(card_width * 0.72, card_width * 1.12)
}

fn draw_card(
    ui: &mut Ui,
    rect: egui::Rect,
    entry: &StlFileInfo,
    texture: &TextureHandle,
    selected: bool,
    hovered: bool,
    focus_zone: FocusZone,
) {
    let bg = if selected {
        Color32::from_rgb(47, 69, 78)
    } else if hovered {
        Color32::from_rgb(48, 50, 56)
    } else {
        Color32::from_rgb(37, 38, 43)
    };
    let stroke = if selected {
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
    if selected && focus_zone == FocusZone::Grid {
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(6),
            egui::Stroke::new(2.0, Color32::from_rgb(80, 200, 200)),
            egui::StrokeKind::Outside,
        );
    }

    let img_rect = egui::Rect::from_min_max(
        rect.min + Vec2::new(4.0, 4.0),
        egui::pos2(rect.max.x - 4.0, rect.max.y - 48.0),
    );
    ui.painter().image(
        texture.id(),
        img_rect,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );

    let mut title = entry.filename.clone();
    if entry.meta.as_ref().is_some_and(|meta| meta.favorite) {
        title = format!("★ {}", title);
    }
    ui.painter().text(
        egui::pos2(rect.min.x + 6.0, rect.max.y - 40.0),
        egui::Align2::LEFT_TOP,
        utils::truncate_filename(&title, 24),
        egui::FontId::proportional(12.0),
        Color32::from_rgb(222, 224, 228),
    );
    let meta = format!(
        "{} · {}",
        format_size(entry.size),
        entry
            .triangle_count
            .map(format_count)
            .unwrap_or_else(|| "— tris".to_string())
    );
    ui.painter().text(
        egui::pos2(rect.min.x + 6.0, rect.max.y - 20.0),
        egui::Align2::LEFT_TOP,
        meta,
        egui::FontId::monospace(10.5),
        Color32::from_rgb(144, 148, 156),
    );
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
