use egui::{Color32, FontId, Pos2, Shape, Ui, Vec2};

pub struct EmptyState;

impl EmptyState {
    pub fn show(ui: &mut Ui, on_open_folder: &mut bool) {
        let center = ui.min_rect().center();
        let content_height = 280.0;
        let top = center.y - content_height / 2.0;

        // Draw isometric cube icon with 3 rhombuses
        let cube_center = Pos2::new(center.x, top + 60.0);
        let s: f32 = 48.0; // rhombus half-width
        let h: f32 = 27.0; // rhombus half-height

        // Top face (brightest)
        ui.painter().add(Shape::convex_polygon(
            vec![
                cube_center + Vec2::new(0.0, -h),
                cube_center + Vec2::new(s, 0.0),
                cube_center + Vec2::new(0.0, h),
                cube_center + Vec2::new(-s, 0.0),
            ],
            Color32::from_rgb(80, 170, 170),
            (2.0, Color32::from_rgb(60, 140, 140)),
        ));

        // Left face (medium)
        ui.painter().add(Shape::convex_polygon(
            vec![
                cube_center + Vec2::new(-s, 0.0),
                cube_center + Vec2::new(0.0, h),
                cube_center + Vec2::new(-s * 0.65, h * 1.7),
                cube_center + Vec2::new(-s * 1.65, h * 0.7),
            ],
            Color32::from_rgb(55, 130, 130),
            (2.0, Color32::from_rgb(40, 100, 100)),
        ));

        // Right face (darkest)
        ui.painter().add(Shape::convex_polygon(
            vec![
                cube_center + Vec2::new(0.0, h),
                cube_center + Vec2::new(s, 0.0),
                cube_center + Vec2::new(s * 0.35, h * 1.7),
                cube_center + Vec2::new(-s * 0.65, h * 1.7),
            ],
            Color32::from_rgb(40, 100, 100),
            (2.0, Color32::from_rgb(30, 75, 75)),
        ));

        ui.add_space(140.0);

        // Headline
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("No models yet")
                    .font(FontId::proportional(16.0))
                    .color(Color32::from_rgb(230, 230, 235)),
            );
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new("Add a folder to start browsing your STL collection")
                    .font(FontId::proportional(12.0))
                    .color(Color32::from_rgb(150, 150, 160)),
            );
            ui.add_space(14.0);

            // "Add Folder" teal button
            let btn = egui::Button::new(
                egui::RichText::new("  Add Folder  ")
                    .font(FontId::proportional(13.0))
                    .color(Color32::WHITE),
            )
            .fill(Color32::from_rgb(30, 140, 140))
            .corner_radius(6.0)
            .min_size(Vec2::new(120.0, 32.0));

            if ui.add(btn).clicked() {
                *on_open_folder = true;
            }

            ui.add_space(16.0);
            ui.label(
                egui::RichText::new(
                    "ModelRack reads STL files directly from your folders \u{2014} no uploads, no cloud.",
                )
                .font(FontId::proportional(11.0))
                .color(Color32::from_rgb(120, 120, 130)),
            );
        });
    }
}
