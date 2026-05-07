use std::collections::HashSet;
use std::path::{Path, PathBuf};

use egui::{Color32, CornerRadius, RichText, Ui};

use crate::scanner::{MeshData, SidecarMeta, StlFileInfo, StlType};
use crate::strings;

pub struct MetadataPanel;

impl MetadataPanel {
    /// Renders the detail panel and returns true if sidecar metadata was modified.
    pub fn show(
        ui: &mut Ui,
        selected: Option<&mut StlFileInfo>,
        preview_mesh: Option<&MeshData>,
        preview_yaw: &mut f32,
        slicer_path: Option<&Path>,
    ) -> bool {
        ui.heading("Details");

        match selected {
            None => {
                ui.add_space(12.0);
                ui.label(
                    RichText::new(strings::MSG_NO_MODEL_SELECTED)
                        .color(ui.ctx().style().visuals.weak_text_color()),
                );
                false
            }
            Some(entry) => {
                let mut changed = false;
                ui.add_space(10.0);

                preview(ui, entry.stl_type, preview_mesh, preview_yaw);
                ui.add_space(10.0);

                ui.label(RichText::new(&entry.filename).size(14.0).strong());
                ui.label(
                    RichText::new(entry.path.display().to_string())
                        .monospace()
                        .size(10.5)
                        .color(Color32::from_rgb(132, 136, 145)),
                );
                ui.add_space(10.0);

                // --- Sidecar metadata ---
                let meta = entry.meta.get_or_insert_with(SidecarMeta::default);

                ui.horizontal(|ui| {
                    open_slicer_menu(ui, &entry.path, slicer_path);

                    if ui.button("Mark Printed").clicked() {
                        meta.printed += 1;
                        changed = true;
                    }

                    let favorite = if meta.favorite { "★" } else { "☆" };
                    if ui
                        .selectable_label(meta.favorite, favorite)
                        .on_hover_text(strings::MSG_META_FAVORITE)
                        .clicked()
                    {
                        meta.favorite = !meta.favorite;
                        changed = true;
                    }
                });
                ui.add_space(12.0);

                section_title(ui, "Geometry");
                key_value(ui, "Format", stl_type_label(entry.stl_type));
                key_value(
                    ui,
                    "Triangles",
                    &entry
                        .triangle_count
                        .map(format_count)
                        .unwrap_or_else(|| "—".to_string()),
                );
                if let Some(dims) = entry.dimensions {
                    key_value(
                        ui,
                        "Dimensions",
                        &format!("{:.1} x {:.1} x {:.1} mm", dims[0], dims[1], dims[2]),
                    );
                }
                key_value(ui, "File size", &format_size(entry.size));
                ui.add_space(10.0);

                // Author
                section_title(ui, "Metadata");
                ui.label(RichText::new(strings::MSG_META_AUTHOR).size(11.0).strong());
                let mut author = meta.author.clone();
                if ui.text_edit_singleline(&mut author).changed() {
                    meta.author = author;
                    changed = true;
                }
                ui.add_space(4.0);

                // Printed count
                ui.label(RichText::new(strings::MSG_META_PRINTED).size(11.0).strong());
                ui.label(format!("{} times", meta.printed));
                ui.add_space(4.0);

                // Tags
                ui.label(RichText::new(strings::MSG_META_TAGS).size(11.0).strong());
                if meta.tags.is_empty() {
                    ui.label(
                        RichText::new(strings::MSG_META_NO_TAGS)
                            .color(ui.ctx().style().visuals.weak_text_color()),
                    );
                } else {
                    ui.horizontal_wrapped(|ui| {
                        let mut remove_idx: Option<usize> = None;
                        for (i, tag) in meta.tags.iter().enumerate() {
                            let tag_color = tag_color(tag);
                            let pill = egui::Button::new(
                                RichText::new(format!("{} \u{2715}", tag))
                                    .size(11.0)
                                    .color(Color32::WHITE),
                            )
                            .fill(tag_color)
                            .corner_radius(6.0)
                            .min_size(egui::Vec2::new(0.0, 20.0));
                            if ui.add(pill).clicked() {
                                remove_idx = Some(i);
                            }
                        }
                        if let Some(i) = remove_idx {
                            meta.tags.remove(i);
                            changed = true;
                        }
                    });
                }
                // Add tag input
                ui.horizontal(|ui| {
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut meta.tag_input)
                            .hint_text("New tag...")
                            .desired_width(80.0),
                    );
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let trimmed = meta.tag_input.trim().to_string();
                        if !trimmed.is_empty() && !meta.tags.contains(&trimmed) {
                            meta.tags.push(trimmed);
                            meta.tag_input.clear();
                            changed = true;
                        }
                    }
                });
                ui.add_space(4.0);

                // Notes
                ui.add_space(6.0);
                section_title(ui, strings::MSG_META_NOTES);
                let mut notes = meta.notes.clone();
                if ui
                    .add(
                        egui::TextEdit::multiline(&mut notes)
                            .hint_text(strings::MSG_META_NOTES_PLACEHOLDER)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY),
                    )
                    .changed()
                {
                    meta.notes = notes;
                    changed = true;
                }
                ui.add_space(6.0);

                // Print history (read-only)
                if !meta.print_history.is_empty() {
                    section_title(ui, strings::MSG_META_PRINT_HISTORY);
                    for record in &meta.print_history {
                        ui.label(
                            RichText::new(format!(
                                "{} — {} ({})",
                                record.date,
                                record.material,
                                if record.success { "OK" } else { "Failed" }
                            ))
                            .size(11.0)
                            .color(ui.ctx().style().visuals.weak_text_color()),
                        );
                    }
                    ui.add_space(4.0);
                }

                ui.separator();
                ui.add_space(4.0);
                section_title(ui, "File");
                key_value(ui, "Path", &entry.path.display().to_string());
                ui.label(
                    RichText::new(format!("b3 {}", hex_prefix(&entry.hash, 12)))
                        .monospace()
                        .size(10.5)
                        .color(Color32::from_rgb(132, 136, 145)),
                );

                changed
            }
        }
    }
}

fn open_slicer_menu(ui: &mut Ui, model_path: &Path, configured: Option<&Path>) {
    ui.scope(|ui| {
        ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::from_rgb(28, 130, 134);
        ui.style_mut().visuals.widgets.hovered.bg_fill = Color32::from_rgb(39, 148, 154);
        ui.style_mut().spacing.button_padding = egui::vec2(10.0, 7.0);
        ui.menu_button(
            RichText::new("Open in Slicer ▾")
                .size(12.5)
                .color(Color32::WHITE),
            |ui| {
                if let Some(path) = configured {
                    if ui
                        .button(format!("Configured: {}", path.display()))
                        .clicked()
                    {
                        let _ = launch_configured_slicer(path, model_path);
                        ui.close_menu();
                    }
                    ui.separator();
                }

                let installed_apps = available_slicer_apps();
                if !installed_apps.is_empty() {
                    for app in installed_apps {
                        if ui.button(app.name).clicked() {
                            let _ = open_with_app_path(model_path, &app.path);
                            ui.close_menu();
                        }
                    }
                } else {
                    ui.label(
                        RichText::new("No installed slicers found")
                            .size(12.0)
                            .color(Color32::from_rgb(132, 136, 145)),
                    );
                    ui.separator();
                    for app in common_slicer_names() {
                        if ui.button(format!("Try {}", app)).clicked() {
                            let _ = open_with_app_name(model_path, app);
                            ui.close_menu();
                        }
                    }
                }

                #[cfg(not(target_os = "macos"))]
                {
                    for app in common_slicer_names() {
                        if ui.button(app).clicked() {
                            let _ = open_with_app_name(model_path, app);
                            ui.close_menu();
                        }
                    }
                }

                #[cfg(target_os = "macos")]
                {
                    if ui.button("Choose Other App...").clicked() {
                        let _ = choose_application_and_open(model_path);
                        ui.close_menu();
                    }
                }
                ui.separator();
                if ui.button("System Default").clicked() {
                    let _ = open_model_path(model_path, None);
                    ui.close_menu();
                }
            },
        );
    });
}

struct SlicerApp {
    name: String,
    path: PathBuf,
}

fn common_slicer_names() -> &'static [&'static str] {
    &[
        "OrcaSlicer",
        "Bambu Studio",
        "PrusaSlicer",
        "Ultimaker Cura",
        "Creality Print",
        "Snapmaker Orca",
    ]
}

fn available_slicer_apps() -> Vec<SlicerApp> {
    #[cfg(target_os = "macos")]
    {
        let mut seen = HashSet::new();
        let mut apps = Vec::new();
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let roots = [
            Some(PathBuf::from("/Applications")),
            Some(PathBuf::from("/System/Applications")),
            home.map(|home| home.join("Applications")),
        ];

        for root in roots.into_iter().flatten().filter(|root| root.exists()) {
            if let Ok(entries) = std::fs::read_dir(root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|ext| ext.to_str()) != Some("app") {
                        continue;
                    }
                    let Some(name) = path.file_stem().and_then(|name| name.to_str()) else {
                        continue;
                    };
                    if !is_slicer_name(name) || !seen.insert(name.to_lowercase()) {
                        continue;
                    }
                    apps.push(SlicerApp {
                        name: name.to_string(),
                        path,
                    });
                }
            }
        }
        apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        apps
    }
    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

fn is_slicer_name(name: &str) -> bool {
    let normalized = name.to_lowercase();
    ["slicer", "bambu", "cura", "creality", "snapmaker"]
        .iter()
        .any(|needle| normalized.contains(needle))
}

fn preview(ui: &mut Ui, stl_type: StlType, mesh: Option<&MeshData>, yaw: &mut f32) {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 178.0), egui::Sense::drag());
    if response.dragged() {
        *yaw += response.drag_delta().x * 0.01;
        ui.ctx().request_repaint();
    }
    ui.painter()
        .rect_filled(rect, CornerRadius::same(6), Color32::from_rgb(31, 32, 37));
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(6),
        egui::Stroke::new(1.0, Color32::from_rgb(58, 60, 68)),
        egui::StrokeKind::Inside,
    );

    if let Some(mesh) = mesh {
        draw_mesh_preview(ui, mesh, rect, *yaw);
    } else {
        draw_fallback_preview(ui, rect, *yaw);
    }

    let hint = if stl_type == StlType::Unknown {
        "Unparseable"
    } else if mesh.is_some() {
        "Orbit: drag actual mesh"
    } else {
        "Orbit: drag to rotate"
    };
    ui.painter().text(
        rect.left_bottom() + egui::vec2(10.0, -10.0),
        egui::Align2::LEFT_BOTTOM,
        hint,
        egui::FontId::proportional(11.0),
        Color32::from_rgb(132, 136, 145),
    );
}

fn draw_mesh_preview(ui: &mut Ui, mesh: &MeshData, rect: egui::Rect, yaw: f32) {
    if mesh.vertices.is_empty() || mesh.faces.is_empty() {
        draw_fallback_preview(ui, rect, yaw);
        return;
    }

    let (center_3d, scale) = mesh_normalization(&mesh.vertices);
    let center_2d = rect.center() + egui::vec2(0.0, -4.0);
    let fit = rect.height().min(rect.width()) * 0.42;
    let projected: Vec<_> = mesh
        .vertices
        .iter()
        .map(|v| {
            project_preview_vertex(normalize_vertex(*v, center_3d, scale), yaw, center_2d, fit)
        })
        .collect();

    let max_segments = 1400usize;
    let total_segments = mesh.faces.len().saturating_mul(3).max(1);
    let stride = (total_segments / max_segments).max(1);
    let mut segment_idx = 0usize;

    for face in &mesh.faces {
        for (a, b) in [(face[0], face[1]), (face[1], face[2]), (face[2], face[0])] {
            segment_idx += 1;
            if !segment_idx.is_multiple_of(stride) {
                continue;
            }
            let a = a as usize;
            let b = b as usize;
            if a >= projected.len() || b >= projected.len() {
                continue;
            }
            ui.painter().line_segment(
                [projected[a], projected[b]],
                egui::Stroke::new(1.0, Color32::from_rgb(168, 204, 210)),
            );
        }
    }
}

fn draw_fallback_preview(ui: &mut Ui, rect: egui::Rect, yaw: f32) {
    let center = rect.center() + egui::vec2(0.0, -4.0);
    let line = egui::Stroke::new(1.2, Color32::from_rgb(170, 205, 210));
    let vertices = [
        [-1.0, -0.7, -1.0],
        [1.0, -0.7, -1.0],
        [1.0, -0.7, 1.0],
        [-1.0, -0.7, 1.0],
        [-0.8, 0.7, -0.8],
        [0.8, 0.7, -0.8],
        [0.8, 0.7, 0.8],
        [-0.8, 0.7, 0.8],
    ];
    let projected: Vec<_> = vertices
        .iter()
        .map(|v| project_preview_vertex(*v, yaw, center, 44.0))
        .collect();
    for (a, b) in [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
        (0, 6),
        (1, 7),
    ] {
        ui.painter()
            .line_segment([projected[a], projected[b]], line);
    }
}

fn mesh_normalization(vertices: &[[f32; 3]]) -> ([f32; 3], f32) {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for v in vertices {
        for axis in 0..3 {
            min[axis] = min[axis].min(v[axis]);
            max[axis] = max[axis].max(v[axis]);
        }
    }

    let center = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];
    let extent = [
        (max[0] - min[0]).abs(),
        (max[1] - min[1]).abs(),
        (max[2] - min[2]).abs(),
    ];
    let max_extent = extent[0].max(extent[1]).max(extent[2]).max(0.0001);
    (center, 2.0 / max_extent)
}

fn normalize_vertex(v: [f32; 3], center: [f32; 3], scale: f32) -> [f32; 3] {
    [
        (v[0] - center[0]) * scale,
        (v[1] - center[1]) * scale,
        (v[2] - center[2]) * scale,
    ]
}

fn project_preview_vertex(v: [f32; 3], yaw: f32, center: egui::Pos2, scale: f32) -> egui::Pos2 {
    let (sin_y, cos_y) = yaw.sin_cos();
    let x = v[0] * cos_y - v[2] * sin_y;
    let z = v[0] * sin_y + v[2] * cos_y;
    let y = v[1];

    let iso_x = (x - z) * 0.72;
    let iso_y = y - (x + z) * 0.28;
    center + egui::vec2(iso_x * scale, iso_y * scale)
}

fn section_title(ui: &mut Ui, title: &str) {
    ui.label(
        RichText::new(title.to_uppercase())
            .size(10.5)
            .color(Color32::from_rgb(142, 146, 154)),
    );
}

fn key_value(ui: &mut Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(key)
                .size(11.5)
                .color(Color32::from_rgb(156, 160, 168)),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(value)
                    .monospace()
                    .size(11.5)
                    .color(Color32::from_rgb(222, 224, 228)),
            );
        });
    });
}

fn stl_type_label(stl_type: StlType) -> &'static str {
    match stl_type {
        StlType::Binary => "Binary STL",
        StlType::Ascii => "ASCII STL",
        StlType::ThreeMf => "3MF",
        StlType::Obj => "OBJ",
        StlType::Step => "STEP",
        StlType::LargeStl => "Large STL",
        StlType::Unknown => "Unknown",
    }
}

fn hex_prefix(hash: &[u8; 32], bytes: usize) -> String {
    hash.iter()
        .take(bytes)
        .map(|b| format!("{:02x}", b))
        .collect()
}

fn open_model_path(path: &Path, slicer_path: Option<&Path>) -> std::io::Result<()> {
    if let Some(slicer) = slicer_path.filter(|path| !path.as_os_str().is_empty()) {
        if launch_configured_slicer(slicer, path).is_ok() {
            return Ok(());
        }
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(path).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &path.display().to_string()])
            .spawn()?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(path).spawn()?;
    }
    Ok(())
}

fn launch_configured_slicer(slicer: &Path, model: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        if slicer.extension().and_then(|ext| ext.to_str()) == Some("app") {
            std::process::Command::new("open")
                .arg("-a")
                .arg(slicer)
                .arg(model)
                .spawn()?;
            return Ok(());
        }
    }

    std::process::Command::new(slicer).arg(model).spawn()?;
    Ok(())
}

fn open_with_app_name(model: &Path, app_name: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-a")
            .arg(app_name)
            .arg(model)
            .spawn()?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app_name;
        open_model_path(model, None)?;
    }
    Ok(())
}

fn open_with_app_path(model: &Path, app_path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-a")
            .arg(app_path)
            .arg(model)
            .spawn()?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app_path;
        open_model_path(model, None)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn choose_application_and_open(model: &Path) -> std::io::Result<()> {
    let Some(app_path) = rfd::FileDialog::new()
        .add_filter("Application", &["app"])
        .set_directory("/Applications")
        .pick_file()
    else {
        return Ok(());
    };
    open_with_app_path(model, &app_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_normalization_centers_and_scales_bounds() {
        let vertices = [[0.0, 0.0, 0.0], [10.0, 5.0, 2.0]];
        let (center, scale) = mesh_normalization(&vertices);

        assert_eq!(center, [5.0, 2.5, 1.0]);
        assert_eq!(scale, 0.2);
        assert_eq!(
            normalize_vertex([10.0, 2.5, 1.0], center, scale),
            [1.0, 0.0, 0.0]
        );
    }

    #[test]
    fn slicer_name_detection_catches_common_apps() {
        assert!(is_slicer_name("PrusaSlicer"));
        assert!(is_slicer_name("Bambu Studio"));
        assert!(is_slicer_name("Ultimaker Cura"));
        assert!(!is_slicer_name("Preview"));
    }
}

fn tag_color(tag: &str) -> Color32 {
    let palette = [
        Color32::from_rgb(70, 130, 180),
        Color32::from_rgb(140, 100, 160),
        Color32::from_rgb(60, 150, 110),
        Color32::from_rgb(190, 130, 60),
        Color32::from_rgb(180, 80, 100),
        Color32::from_rgb(50, 140, 150),
    ];
    let idx = tag.bytes().fold(0usize, |acc, b| {
        acc.wrapping_mul(31).wrapping_add(b as usize)
    });
    palette[idx % palette.len()]
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
