use egui::Ui;

use crate::strings;
use crate::view_model::{Density, SortBy, ViewMode};

pub struct ToolbarWidget;

impl ToolbarWidget {
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        ui: &mut Ui,
        search_query: &mut String,
        view_mode: &mut ViewMode,
        density: &mut Density,
        sort_by: &mut SortBy,
        sort_ascending: &mut bool,
        collapsed_sidebar: &mut bool,
        collapsed_detail: &mut bool,
        user_expanded_sidebar: &mut bool,
        user_expanded_detail: &mut bool,
    ) {
        ui.horizontal(|ui| {
            // Search input
            let search_id = egui::Id::new("search_input");
            let mut search_hovered = false;
            ui.horizontal(|ui| {
                let (rect, response) =
                    ui.allocate_exact_size(egui::Vec2::new(220.0, 22.0), egui::Sense::click());
                search_hovered = response.hovered();
                if response.clicked() {
                    ui.memory_mut(|mem| mem.request_focus(search_id));
                }

                let bg = if search_hovered || ui.memory(|mem| mem.focused() == Some(search_id)) {
                    egui::Color32::from_rgb(55, 55, 60)
                } else {
                    egui::Color32::from_rgb(42, 42, 47)
                };
                ui.painter().rect_filled(rect, 6.0, bg);
                ui.painter().rect_stroke(
                    rect,
                    6.0,
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 70, 75)),
                    egui::StrokeKind::Inside,
                );

                let icon_pos = rect.left_center() + egui::Vec2::new(6.0, -5.0);
                ui.painter().text(
                    icon_pos,
                    egui::Align2::LEFT_CENTER,
                    "\u{1F50D}",
                    egui::FontId::proportional(11.0),
                    egui::Color32::from_rgb(150, 150, 155),
                );

                let text_pos = rect.left_center() + egui::Vec2::new(22.0, -5.0);
                let text_widget = egui::TextEdit::singleline(search_query)
                    .id(search_id)
                    .font(egui::FontId::proportional(12.0))
                    .text_color(egui::Color32::from_rgb(220, 220, 220))
                    .hint_text(strings::MSG_SEARCH_PLACEHOLDER)
                    .desired_width(rect.width() - 46.0)
                    .frame(false);
                let text_response = ui.put(
                    egui::Rect::from_min_size(text_pos, egui::Vec2::new(rect.width() - 46.0, 16.0)),
                    text_widget,
                );

                if !search_query.is_empty() {
                    let clear_x = rect.right() - 16.0;
                    let clear_rect = egui::Rect::from_center_size(
                        egui::pos2(clear_x, rect.center().y),
                        egui::Vec2::new(14.0, 14.0),
                    );
                    let clear_resp = ui.put(clear_rect, egui::Button::new("x").frame(false));
                    if clear_resp.clicked() {
                        search_query.clear();
                        ui.memory_mut(|mem| mem.request_focus(search_id));
                    }
                }

                // Dismiss keyboard on enter
                if text_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    ui.memory_mut(|mem| mem.surrender_focus(search_id));
                }
            });

            ui.separator();

            // View mode toggle
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let grid_sel = ui
                    .selectable_label(*view_mode == ViewMode::Grid, "\u{25A6}")
                    .on_hover_text("Grid view");
                if grid_sel.clicked() {
                    *view_mode = ViewMode::Grid;
                }
                let list_sel = ui
                    .selectable_label(*view_mode == ViewMode::List, "\u{2630}")
                    .on_hover_text("List view");
                if list_sel.clicked() {
                    *view_mode = ViewMode::List;
                }
                let masonry_sel = ui
                    .selectable_label(*view_mode == ViewMode::Masonry, "\u{25A5}")
                    .on_hover_text("Masonry view");
                if masonry_sel.clicked() {
                    *view_mode = ViewMode::Masonry;
                }
            });

            ui.separator();

            // Density toggle
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (value, label) in [
                    (Density::Small, "S"),
                    (Density::Medium, "M"),
                    (Density::Large, "L"),
                ] {
                    if ui
                        .selectable_label(*density == value, label)
                        .on_hover_text("Thumbnail density")
                        .clicked()
                    {
                        *density = value;
                    }
                }
            });

            ui.separator();

            // Panel toggle buttons (visible when panels are collapsed)
            if *collapsed_sidebar {
                let sidebar_toggle = ui
                    .selectable_label(false, "\u{2630}")
                    .on_hover_text("Show sidebar (window < 800px)");
                if sidebar_toggle.clicked() {
                    *collapsed_sidebar = false;
                    *user_expanded_sidebar = true;
                }
            }
            if *collapsed_detail {
                let detail_toggle = ui
                    .selectable_label(false, "\u{25E8}")
                    .on_hover_text("Show detail panel (window < 1024px)");
                if detail_toggle.clicked() {
                    *collapsed_detail = false;
                    *user_expanded_detail = true;
                }
            }

            ui.separator();

            // Sort dropdown
            egui::ComboBox::from_id_salt("sort_combo")
                .selected_text(match sort_by {
                    SortBy::Name => strings::MSG_SORT_NAME,
                    SortBy::Date => strings::MSG_SORT_DATE,
                    SortBy::Size => strings::MSG_SORT_SIZE,
                    SortBy::Triangles => strings::MSG_SORT_TRIANGLES,
                })
                .width(120.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(sort_by, SortBy::Name, strings::MSG_SORT_NAME);
                    ui.selectable_value(sort_by, SortBy::Date, strings::MSG_SORT_DATE);
                    ui.selectable_value(sort_by, SortBy::Size, strings::MSG_SORT_SIZE);
                    ui.selectable_value(sort_by, SortBy::Triangles, strings::MSG_SORT_TRIANGLES);
                });

            // Sort direction toggle
            let dir_label = if *sort_ascending {
                "\u{2191}"
            } else {
                "\u{2193}"
            };
            if ui
                .selectable_label(false, dir_label)
                .on_hover_text(if *sort_ascending {
                    "Ascending"
                } else {
                    "Descending"
                })
                .clicked()
            {
                *sort_ascending = !*sort_ascending;
            }
        });
    }
}
