use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use egui::{Color32, RichText, Sense, Ui, Vec2};

use crate::scanner::{StlFileInfo, StlType};
use crate::view_model::LibraryFilter;

const SIDEBAR_WIDTH: f32 = 220.0;

pub struct SidebarWidget;

impl SidebarWidget {
    pub fn show(
        ui: &mut Ui,
        entries: &[StlFileInfo],
        current_folder: Option<&Path>,
        active_filter: &mut LibraryFilter,
        open_folder: &mut bool,
    ) {
        ui.set_width(SIDEBAR_WIDTH);
        ui.add_space(8.0);

        sidebar_title(ui, "Library", None);
        smart_item(
            ui,
            active_filter,
            LibraryFilter::All,
            "▦",
            "All Models",
            entries.len(),
        );
        smart_item(
            ui,
            active_filter,
            LibraryFilter::Recent,
            "◷",
            "Recent",
            entries.iter().filter(|e| is_recent(e.modified)).count(),
        );
        smart_item(
            ui,
            active_filter,
            LibraryFilter::Favorites,
            "★",
            "Favorites",
            entries
                .iter()
                .filter(|e| e.meta.as_ref().is_some_and(|m| m.favorite))
                .count(),
        );
        smart_item(
            ui,
            active_filter,
            LibraryFilter::Printed,
            "⎙",
            "Printed",
            entries
                .iter()
                .filter(|e| e.meta.as_ref().is_some_and(|m| m.printed > 0))
                .count(),
        );
        smart_item(
            ui,
            active_filter,
            LibraryFilter::Duplicates,
            "⧉",
            "Duplicates",
            duplicate_count(entries),
        );
        smart_item(
            ui,
            active_filter,
            LibraryFilter::Ready,
            "◈",
            "Ready",
            entries
                .iter()
                .filter(|e| e.stl_type != StlType::Unknown)
                .count(),
        );
        smart_item(
            ui,
            active_filter,
            LibraryFilter::Errors,
            "!",
            "Unparseable",
            entries
                .iter()
                .filter(|e| e.stl_type == StlType::Unknown)
                .count(),
        );

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(8.0);

        sidebar_title(ui, "Folders", Some("+"));
        if let Some(folder) = current_folder {
            let name = folder
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("Selected Folder");
            folder_item(ui, active_filter, "▾", name, folder, entries.len(), 0);
            ui.label(
                RichText::new(folder.display().to_string())
                    .monospace()
                    .size(10.5)
                    .color(Color32::from_rgb(125, 128, 136)),
            );
            for (path, label, count, depth) in folder_counts(entries, folder).into_iter().take(16) {
                folder_item(ui, active_filter, "·", &label, &path, count, depth);
            }
        } else {
            let button = egui::Button::new(
                RichText::new("+ Add Folder")
                    .size(12.5)
                    .color(Color32::from_rgb(225, 245, 245)),
            )
            .fill(Color32::from_rgb(28, 118, 122))
            .corner_radius(6.0)
            .min_size(Vec2::new(128.0, 28.0));
            if ui.add(button).clicked() {
                *open_folder = true;
            }
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(8.0);

        sidebar_title(ui, "Tags", Some("+"));
        let tags = tag_counts(entries);
        if tags.is_empty() {
            ui.label(
                RichText::new("No tags yet")
                    .size(12.0)
                    .color(Color32::from_rgb(130, 132, 140)),
            );
        } else {
            for (tag, count) in tags.into_iter().take(12) {
                tag_item(ui, active_filter, &tag, count);
            }
        }
    }
}

fn is_recent(modified: Option<SystemTime>) -> bool {
    modified.is_some_and(|modified| {
        SystemTime::now()
            .duration_since(modified)
            .is_ok_and(|age| age.as_secs() <= 30 * 24 * 60 * 60)
    })
}

fn duplicate_count(entries: &[StlFileInfo]) -> usize {
    let mut counts: BTreeMap<[u8; 32], usize> = BTreeMap::new();
    for entry in entries {
        *counts.entry(entry.hash).or_insert(0) += 1;
    }
    entries
        .iter()
        .filter(|entry| counts.get(&entry.hash).copied().unwrap_or(0) > 1)
        .count()
}

fn sidebar_title(ui: &mut Ui, title: &str, action: Option<&str>) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(title.to_uppercase())
                .size(10.5)
                .color(Color32::from_rgb(140, 144, 152)),
        );
        if let Some(action) = action {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(action)
                        .size(13.0)
                        .color(Color32::from_rgb(130, 200, 205)),
                );
            });
        }
    });
}

fn smart_item(
    ui: &mut Ui,
    active_filter: &mut LibraryFilter,
    filter: LibraryFilter,
    icon: &str,
    label: &str,
    count: usize,
) {
    let selected = *active_filter == filter;
    if sidebar_row(ui, selected, icon, label, count, None, 0).clicked() {
        *active_filter = filter;
    }
}

fn folder_item(
    ui: &mut Ui,
    active_filter: &mut LibraryFilter,
    icon: &str,
    label: &str,
    path: &Path,
    count: usize,
    depth: usize,
) {
    let selected = matches!(active_filter, LibraryFilter::Folder(active) if active == path);
    if sidebar_row(ui, selected, icon, label, count, None, depth).clicked() {
        *active_filter = LibraryFilter::Folder(path.to_path_buf());
    }
}

fn tag_item(ui: &mut Ui, active_filter: &mut LibraryFilter, tag: &str, count: usize) {
    let selected = matches!(active_filter, LibraryFilter::Tag(active) if active == tag);
    if sidebar_row(ui, selected, "●", tag, count, Some(tag_color(tag)), 0).clicked() {
        *active_filter = LibraryFilter::Tag(tag.to_string());
    }
}

fn sidebar_row(
    ui: &mut Ui,
    selected: bool,
    icon: &str,
    label: &str,
    count: usize,
    icon_color: Option<Color32>,
    depth: usize,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
    let fill = if selected {
        Color32::from_rgb(42, 70, 76)
    } else if response.hovered() {
        Color32::from_rgb(38, 40, 46)
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 6.0, fill);
    if selected {
        let bar = egui::Rect::from_min_size(
            rect.min + Vec2::new(0.0, 6.0),
            Vec2::new(2.0, rect.height() - 12.0),
        );
        ui.painter()
            .rect_filled(bar, 1.0, Color32::from_rgb(85, 205, 210));
    }

    let icon_color = icon_color.unwrap_or(Color32::from_rgb(160, 166, 174));
    let indent = depth as f32 * 12.0;
    ui.painter().text(
        rect.left_center() + Vec2::new(12.0 + indent, -0.5),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(12.0),
        icon_color,
    );
    ui.painter().text(
        rect.left_center() + Vec2::new(26.0 + indent, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.5),
        if selected {
            Color32::from_rgb(236, 242, 244)
        } else {
            Color32::from_rgb(205, 208, 214)
        },
    );
    ui.painter().text(
        rect.right_center() - Vec2::new(8.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        count.to_string(),
        egui::FontId::monospace(11.0),
        Color32::from_rgb(138, 142, 150),
    );

    response
}

fn folder_counts(entries: &[StlFileInfo], root: &Path) -> Vec<(PathBuf, String, usize, usize)> {
    let mut counts: BTreeMap<PathBuf, usize> = BTreeMap::new();
    for entry in entries {
        let Some(parent) = entry.path.parent() else {
            continue;
        };
        if parent == root {
            continue;
        }
        *counts.entry(parent.to_path_buf()).or_insert(0) += 1;
    }

    counts
        .into_iter()
        .map(|(path, count)| {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            let depth = relative.components().count().saturating_sub(1).min(3);
            let label = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Folder")
                .to_string();
            (path, label, count, depth)
        })
        .collect()
}

fn tag_counts(entries: &[StlFileInfo]) -> Vec<(String, usize)> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        if let Some(meta) = &entry.meta {
            for tag in &meta.tags {
                *counts.entry(tag.clone()).or_insert(0) += 1;
            }
        }
    }
    counts.into_iter().collect()
}

fn tag_color(tag: &str) -> Color32 {
    let palette = [
        Color32::from_rgb(88, 174, 185),
        Color32::from_rgb(137, 116, 190),
        Color32::from_rgb(92, 172, 126),
        Color32::from_rgb(198, 139, 76),
        Color32::from_rgb(190, 96, 116),
        Color32::from_rgb(104, 150, 212),
    ];
    let idx = tag.bytes().fold(0usize, |acc, b| {
        acc.wrapping_mul(31).wrapping_add(b as usize)
    });
    palette[idx % palette.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{StlFileInfo, StlType};

    fn entry(path: &str) -> StlFileInfo {
        StlFileInfo {
            path: PathBuf::from(path),
            filename: Path::new(path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            size: 1,
            hash: [0; 32],
            stl_type: StlType::Binary,
            triangle_count: Some(1),
            dimensions: Some([1.0, 1.0, 1.0]),
            modified: None,
            meta: None,
        }
    }

    fn entry_with_hash(path: &str, hash_byte: u8) -> StlFileInfo {
        let mut item = entry(path);
        item.hash = [hash_byte; 32];
        item
    }

    #[test]
    fn folder_counts_skip_root_files_and_count_nested_parents() {
        let root = Path::new("/library");
        let entries = vec![
            entry("/library/root.stl"),
            entry("/library/fixtures/a.stl"),
            entry("/library/fixtures/b.stl"),
            entry("/library/fixtures/jigs/c.stl"),
        ];

        let counts = folder_counts(&entries, root);

        assert!(counts.iter().any(|(path, label, count, depth)| path
            == Path::new("/library/fixtures")
            && label == "fixtures"
            && *count == 2
            && *depth == 0));
        assert!(counts.iter().any(|(path, label, count, depth)| path
            == Path::new("/library/fixtures/jigs")
            && label == "jigs"
            && *count == 1
            && *depth == 1));
    }

    #[test]
    fn duplicate_count_counts_all_members_of_duplicate_hash_groups() {
        let entries = vec![
            entry_with_hash("/library/a.stl", 1),
            entry_with_hash("/library/copy/a.stl", 1),
            entry_with_hash("/library/unique.stl", 2),
        ];

        assert_eq!(duplicate_count(&entries), 2);
    }
}
