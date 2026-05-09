use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::scanner;

pub enum ScanStatus {
    Idle,
    Scanning {
        found: usize,
        scanned: usize,
        skipped: usize,
        current: String,
    },
    Done {
        found: usize,
        skipped: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LibraryFilter {
    All,
    Recent,
    Favorites,
    Printed,
    Duplicates,
    Ready,
    Errors,
    Folder(PathBuf),
    Tag(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Grid,
    List,
    Masonry,
}

impl ViewMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Grid => "grid",
            Self::List => "list",
            Self::Masonry => "masonry",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "list" => Self::List,
            "masonry" => Self::Masonry,
            _ => Self::Grid,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Density {
    Small,
    Medium,
    Large,
}

impl Density {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "small" => Self::Small,
            "large" => Self::Large,
            _ => Self::Medium,
        }
    }
}

// Date/Size/Triangles are implemented in filtering and labels, but the current
// Slint toolbar only exposes Name direction toggling. Keep the variants scoped
// here until the sort field picker is wired.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    Name,
    Date,
    Size,
    Triangles,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppPrefs {
    #[serde(default = "default_density")]
    pub density: String,
    #[serde(default = "default_view_mode")]
    pub view_mode: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub slicer_path: String,
    #[serde(default)]
    pub last_folder: Option<PathBuf>,
    #[serde(default)]
    pub excluded_folders: Vec<PathBuf>,
}

impl Default for AppPrefs {
    fn default() -> Self {
        Self {
            density: default_density(),
            view_mode: default_view_mode(),
            theme: default_theme(),
            language: default_language(),
            slicer_path: String::new(),
            last_folder: None,
            excluded_folders: Vec::new(),
        }
    }
}

fn default_density() -> String {
    "medium".to_string()
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_language() -> String {
    "en".to_string()
}

fn default_view_mode() -> String {
    "grid".to_string()
}

pub struct DisplayQuery<'a> {
    pub search_query: &'a str,
    pub library_filter: &'a LibraryFilter,
    pub sort_by: SortBy,
    pub sort_ascending: bool,
    pub preserve_order: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SidebarSummary {
    pub all: usize,
    pub recent: usize,
    pub favorites: usize,
    pub printed: usize,
    pub duplicates: usize,
    pub ready: usize,
    pub errors: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidebarFolder {
    pub path: PathBuf,
    pub label: String,
    pub count: usize,
    pub depth: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidebarTag {
    pub label: String,
    pub count: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BrowserSummary {
    pub displayed: usize,
    pub total: usize,
    pub filter_label: Option<String>,
    pub empty_message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserCard {
    pub stable_key: String,
    pub slot_index: usize,
    pub title: String,
    pub subtitle: String,
    pub author: String,
    pub relative_modified: String,
    pub thumb_key: String,
    pub thumb_path: Option<PathBuf>,
    pub badge: String,
    pub printed_count: u32,
    pub favorite: bool,
    pub printed: bool,
    pub error: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppViewSnapshot {
    pub library_label: String,
    pub sidebar: SidebarSummary,
    pub folders: Vec<SidebarFolder>,
    pub tags: Vec<SidebarTag>,
    pub cards: Vec<BrowserCard>,
    pub browser: BrowserSummary,
    pub status_text: String,
    pub density_label: String,
    pub view_mode_label: String,
    pub sort_label: String,
    pub active_filter_key: String,
}

impl AppViewSnapshot {
    pub fn from_parts(
        entries: &[scanner::StlFileInfo],
        current_folder: Option<&Path>,
        scan_status: &ScanStatus,
        prefs: &AppPrefs,
        query: DisplayQuery<'_>,
    ) -> Self {
        let filter = filter_label(query.library_filter);
        let active_filter_key = filter_key(query.library_filter);
        let sort_by = query.sort_by;
        let sort_ascending = query.sort_ascending;
        let displayed = filtered_sorted_entries(entries, query);
        Self {
            library_label: titlebar_path(current_folder),
            sidebar: sidebar_summary(entries),
            folders: sidebar_folders(entries, current_folder),
            tags: sidebar_tags(entries),
            cards: browser_cards(&displayed),
            browser: BrowserSummary {
                displayed: displayed.len(),
                total: entries.len(),
                filter_label: filter,
                empty_message: empty_message(entries, &displayed),
            },
            status_text: scan_status_text(scan_status),
            density_label: density_short_label(Density::from_str(&prefs.density)).to_string(),
            view_mode_label: view_mode_title(ViewMode::from_str(&prefs.view_mode)).to_string(),
            sort_label: sort_label(sort_by, sort_ascending),
            active_filter_key,
        }
    }
}

pub fn browser_cards(entries: &[scanner::StlFileInfo]) -> Vec<BrowserCard> {
    let mut cards = entries
        .iter()
        .enumerate()
        .map(|(slot_index, entry)| {
            let favorite = entry.meta.as_ref().is_some_and(|meta| meta.favorite);
            let printed_count = entry.meta.as_ref().map_or(0, |meta| meta.printed);
            let printed = printed_count > 0;
            BrowserCard {
                stable_key: entry.path.display().to_string(),
                slot_index,
                title: entry.filename.clone(),
                subtitle: format!(
                    "{} · {}",
                    format_size(entry.size),
                    entry
                        .triangle_count
                        .map(format_triangle_count)
                        .unwrap_or_else(|| "— tris".to_string())
                ),
                author: entry
                    .meta
                    .as_ref()
                    .and_then(|meta| (!meta.author.is_empty()).then(|| meta.author.clone()))
                    .unwrap_or_else(|| "You".to_string()),
                relative_modified: relative_modified_label(entry.modified),
                thumb_key: thumbnail_key(&entry.filename).to_string(),
                thumb_path: entry.thumbnail_path.clone(),
                badge: stl_type_label(entry.stl_type).to_string(),
                printed_count,
                favorite,
                printed,
                error: entry.stl_type == scanner::StlType::Unknown,
            }
        })
        .collect::<Vec<_>>();
    cards.sort_by(|a, b| a.stable_key.cmp(&b.stable_key));
    cards
}

pub fn sidebar_summary(entries: &[scanner::StlFileInfo]) -> SidebarSummary {
    SidebarSummary {
        all: entries.len(),
        recent: entries
            .iter()
            .filter(|entry| is_recent(entry.modified))
            .count(),
        favorites: entries
            .iter()
            .filter(|entry| entry.meta.as_ref().is_some_and(|meta| meta.favorite))
            .count(),
        printed: entries
            .iter()
            .filter(|entry| entry.meta.as_ref().is_some_and(|meta| meta.printed > 0))
            .count(),
        duplicates: duplicate_count(entries),
        ready: entries
            .iter()
            .filter(|entry| entry_is_ready_to_print(entries, entry))
            .count(),
        errors: entries
            .iter()
            .filter(|entry| entry.stl_type == scanner::StlType::Unknown)
            .count(),
    }
}

pub fn sidebar_folders(
    entries: &[scanner::StlFileInfo],
    root: Option<&Path>,
) -> Vec<SidebarFolder> {
    let Some(root) = root else {
        return Vec::new();
    };

    let mut counts: BTreeMap<PathBuf, usize> = BTreeMap::new();
    counts.insert(root.to_path_buf(), 0);
    for entry in entries {
        let Some(parent) = entry.path.parent() else {
            continue;
        };
        *counts.entry(root.to_path_buf()).or_insert(0) += 1;
        if let Ok(relative_parent) = parent.strip_prefix(root) {
            let mut ancestor = root.to_path_buf();
            for component in relative_parent.components() {
                ancestor.push(component.as_os_str());
                *counts.entry(ancestor.clone()).or_insert(0) += 1;
            }
        } else {
            *counts.entry(parent.to_path_buf()).or_insert(0) += 1;
        }
    }

    counts
        .into_iter()
        .map(|(path, count)| {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            let depth = if path == root {
                0
            } else {
                relative.components().count().min(4)
            };
            let label = if path == root {
                root.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Library")
                    .to_string()
            } else {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Folder")
                    .to_string()
            };
            SidebarFolder {
                path,
                label,
                count,
                depth,
            }
        })
        .collect()
}

pub fn sidebar_tags(entries: &[scanner::StlFileInfo]) -> Vec<SidebarTag> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        if let Some(meta) = &entry.meta {
            for tag in &meta.tags {
                *counts.entry(tag.clone()).or_insert(0) += 1;
            }
        }
    }
    counts
        .into_iter()
        .map(|(label, count)| SidebarTag { label, count })
        .collect()
}

pub fn scan_status_text(status: &ScanStatus) -> String {
    match status {
        ScanStatus::Idle => "Ready".to_string(),
        ScanStatus::Scanning {
            found,
            scanned,
            skipped,
            current,
        } => format!(
            "Scanning {} · found {} · scanned {} · skipped {}",
            current, found, scanned, skipped
        ),
        ScanStatus::Done { found, skipped } => {
            if *skipped == 0 {
                format!("{} models", found)
            } else {
                format!("{} models · {} skipped", found, skipped)
            }
        }
    }
}

pub fn sort_label(sort_by: SortBy, ascending: bool) -> String {
    let field = match sort_by {
        SortBy::Name => "Name",
        SortBy::Date => "Date",
        SortBy::Size => "Size",
        SortBy::Triangles => "Triangles",
    };
    let direction = if ascending { "↑" } else { "↓" };
    format!("{} {}", field, direction)
}

fn density_short_label(density: Density) -> &'static str {
    match density {
        Density::Small => "S",
        Density::Medium => "M",
        Density::Large => "L",
    }
}

fn view_mode_title(view_mode: ViewMode) -> &'static str {
    match view_mode {
        ViewMode::Grid => "Grid",
        ViewMode::List => "List",
        ViewMode::Masonry => "Masonry",
    }
}

fn relative_modified_label(modified: Option<std::time::SystemTime>) -> String {
    let Some(modified) = modified else {
        return "unknown".to_string();
    };
    let Ok(elapsed) = std::time::SystemTime::now().duration_since(modified) else {
        return "today".to_string();
    };
    let days = elapsed.as_secs() / 86400;
    if days < 1 {
        "today".to_string()
    } else if days < 7 {
        format!("{}d ago", days)
    } else if days < 30 {
        format!("{}w ago", days / 7)
    } else if days < 365 {
        format!("{}mo ago", days / 30)
    } else {
        format!("{}y ago", days / 365)
    }
}

pub fn filter_key(filter: &LibraryFilter) -> String {
    match filter {
        LibraryFilter::All => "all".to_string(),
        LibraryFilter::Recent => "recent".to_string(),
        LibraryFilter::Favorites => "favorites".to_string(),
        LibraryFilter::Printed => "printed".to_string(),
        LibraryFilter::Duplicates => "duplicates".to_string(),
        LibraryFilter::Ready => "ready".to_string(),
        LibraryFilter::Errors => "errors".to_string(),
        LibraryFilter::Folder(path) => format!("folder:{}", path.display()),
        LibraryFilter::Tag(tag) => format!("tag:{}", tag),
    }
}

pub fn smart_filter_from_key(key: &str) -> Option<LibraryFilter> {
    Some(match key {
        "all" => LibraryFilter::All,
        "recent" => LibraryFilter::Recent,
        "favorites" => LibraryFilter::Favorites,
        "printed" => LibraryFilter::Printed,
        "duplicates" => LibraryFilter::Duplicates,
        "ready" => LibraryFilter::Ready,
        "errors" => LibraryFilter::Errors,
        _ if key.starts_with("folder:") => {
            LibraryFilter::Folder(PathBuf::from(key.trim_start_matches("folder:")))
        }
        _ if key.starts_with("tag:") => {
            LibraryFilter::Tag(key.trim_start_matches("tag:").to_string())
        }
        _ => return None,
    })
}

pub fn entry_matches_filter(
    entries: &[scanner::StlFileInfo],
    filter: &LibraryFilter,
    entry: &scanner::StlFileInfo,
) -> bool {
    match filter {
        LibraryFilter::All => true,
        LibraryFilter::Recent => entry.modified.is_some_and(|modified| {
            std::time::SystemTime::now()
                .duration_since(modified)
                .is_ok_and(|age| age.as_secs() <= 30 * 24 * 60 * 60)
        }),
        LibraryFilter::Favorites => entry.meta.as_ref().is_some_and(|meta| meta.favorite),
        LibraryFilter::Printed => entry.meta.as_ref().is_some_and(|meta| meta.printed > 0),
        LibraryFilter::Duplicates => {
            entries
                .iter()
                .filter(|candidate| candidate.hash == entry.hash)
                .count()
                > 1
        }
        LibraryFilter::Ready => entry_is_ready_to_print(entries, entry),
        LibraryFilter::Errors => entry.stl_type == scanner::StlType::Unknown,
        LibraryFilter::Folder(folder) => entry.path.starts_with(folder),
        LibraryFilter::Tag(tag) => entry
            .meta
            .as_ref()
            .is_some_and(|meta| meta.tags.iter().any(|entry_tag| entry_tag == tag)),
    }
}

fn is_recent(modified: Option<std::time::SystemTime>) -> bool {
    modified.is_some_and(|modified| {
        std::time::SystemTime::now()
            .duration_since(modified)
            .is_ok_and(|age| age.as_secs() <= 30 * 24 * 60 * 60)
    })
}

fn duplicate_count(entries: &[scanner::StlFileInfo]) -> usize {
    let mut counts: BTreeMap<[u8; 32], usize> = BTreeMap::new();
    for entry in entries {
        *counts.entry(entry.hash).or_insert(0) += 1;
    }
    entries
        .iter()
        .filter(|entry| counts.get(&entry.hash).copied().unwrap_or(0) > 1)
        .count()
}

fn empty_message(entries: &[scanner::StlFileInfo], displayed: &[scanner::StlFileInfo]) -> String {
    if entries.is_empty() {
        "No models yet".to_string()
    } else if displayed.is_empty() {
        "No matching models".to_string()
    } else {
        format!("{} visible models", displayed.len())
    }
}

fn titlebar_path(current_folder: Option<&Path>) -> String {
    let Some(path) = current_folder else {
        return "Sample library".to_string();
    };

    display_path_label(path)
}

pub fn display_path_label(path: &Path) -> String {
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        if let Ok(rest) = path.strip_prefix(&home) {
            if rest.as_os_str().is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", rest.display());
        }
    }

    path.display().to_string()
}

fn stl_type_label(stl_type: scanner::StlType) -> &'static str {
    match stl_type {
        scanner::StlType::Binary | scanner::StlType::Ascii => "STL",
        scanner::StlType::ThreeMf => "3MF",
        scanner::StlType::Obj => "OBJ",
        scanner::StlType::Step => "STEP",
        scanner::StlType::LargeStl => "LARGE",
        scanner::StlType::Unknown => "ERR",
    }
}

fn format_triangle_count(count: usize) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M tris", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K tris", count as f64 / 1_000.0)
    } else {
        format!("{} tris", count)
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

pub fn filter_label(filter: &LibraryFilter) -> Option<String> {
    match filter {
        LibraryFilter::All => None,
        LibraryFilter::Recent => Some("Recent".to_string()),
        LibraryFilter::Favorites => Some("Favorites".to_string()),
        LibraryFilter::Printed => Some("Printed".to_string()),
        LibraryFilter::Duplicates => Some("Duplicates".to_string()),
        LibraryFilter::Ready => Some("Ready".to_string()),
        LibraryFilter::Errors => Some("Unparseable".to_string()),
        LibraryFilter::Folder(folder) => Some(format!(
            "Folder: {}",
            folder
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Library")
        )),
        LibraryFilter::Tag(tag) => Some(format!("Tag: {}", tag)),
    }
}

pub fn filtered_sorted_entries(
    entries: &[scanner::StlFileInfo],
    query: DisplayQuery<'_>,
) -> Vec<scanner::StlFileInfo> {
    let filter_lower = query.search_query.to_lowercase();
    let mut sorted: Vec<&scanner::StlFileInfo> = entries
        .iter()
        .filter(|entry| {
            entry_matches_filter(entries, query.library_filter, entry)
                && (filter_lower.is_empty()
                    || entry.filename.to_lowercase().contains(&filter_lower)
                    || entry.meta.as_ref().is_some_and(|meta| {
                        meta.tags
                            .iter()
                            .any(|tag| tag.to_lowercase().contains(&filter_lower))
                            || meta.notes.to_lowercase().contains(&filter_lower)
                    }))
        })
        .collect();

    if !query.preserve_order {
        match query.sort_by {
            SortBy::Name => {
                sorted.sort_by(|a, b| {
                    if query.sort_ascending {
                        a.filename.to_lowercase().cmp(&b.filename.to_lowercase())
                    } else {
                        b.filename.to_lowercase().cmp(&a.filename.to_lowercase())
                    }
                });
            }
            SortBy::Date => {
                sorted.sort_by(|a, b| {
                    if query.sort_ascending {
                        a.modified.cmp(&b.modified)
                    } else {
                        b.modified.cmp(&a.modified)
                    }
                });
            }
            SortBy::Size => {
                sorted.sort_by(|a, b| {
                    if query.sort_ascending {
                        a.size.cmp(&b.size)
                    } else {
                        b.size.cmp(&a.size)
                    }
                });
            }
            SortBy::Triangles => {
                sorted.sort_by(|a, b| {
                    if query.sort_ascending {
                        a.triangle_count.cmp(&b.triangle_count)
                    } else {
                        b.triangle_count.cmp(&a.triangle_count)
                    }
                });
            }
        }
    }

    sorted.into_iter().cloned().collect()
}

fn entry_is_ready_to_print(entries: &[scanner::StlFileInfo], entry: &scanner::StlFileInfo) -> bool {
    let uses_explicit_ready_status = entries.iter().any(has_ready_status);
    if uses_explicit_ready_status {
        return has_ready_status(entry);
    }

    entry.stl_type != scanner::StlType::Unknown
}

fn has_ready_status(entry: &scanner::StlFileInfo) -> bool {
    entry.meta.as_ref().is_some_and(|meta| {
        meta.tags
            .iter()
            .any(|tag| tag == "ready-to-print" || tag == "queued")
    })
}

pub(crate) fn thumbnail_key(filename: &str) -> &'static str {
    match filename {
        "raspberry_pi_5_poe_rackmount_v2_final.stl" => "rack",
        "pi5_heatsink_clip.stl" => "clip",
        "1U_blank_panel_19in.stl" => "panel",
        "gmktec_nucbox_mount.stl" => "mount",
        "switch_8port_bracket.stl" => "bracket",
        "ssd_2_5in_caddy_x4.stl" => "caddy",
        "spool_holder_universal.stl" => "spool",
        "bambu_p1s_chamber_thermometer.stl" => "therm",
        "cable_chain_15x10.stl" => "chain",
        "snapmaker_a350_drag_chain_link.stl" => "link",
        "라즈베리파이_5_케이스_v3.stl" => "case",
        "책상정리_케이블_홀더.stl" => "holder",
        "키캡_oem_r4_blank.stl" => "keycap",
        "low_poly_fox.stl" => "fox",
        "voronoi_planter_120mm.stl" => "voro",
        "geometric_vase_twisted.stl" => "vase",
        "articulated_dragon_v4.stl" => "drag",
        "benchy_3dbenchy.stl" => "benchy",
        "calibration_cube_20mm.stl" => "cube",
        "all_in_one_test_v2.stl" => "test",
        "broken_export_garbage.stl" => "err",
        "weird_ascii_export.stl" => "ascii",
        "hdd_3_5in_vibration_dampener.stl" => "damp",
        "ups_battery_holder_18650_x8.stl" => "batt",
        "fan_grill_120mm_honeycomb.stl" => "fan",
        "noctua_fan_shroud_140mm.stl" => "shroud",
        "vesa_75_to_100_adapter.stl" => "vesa",
        "monitor_arm_cable_clip.stl" => "mclip",
        "wall_anchor_drywall_kit.stl" => "anc",
        "stringing_test_tower.stl" => "string",
        "overhang_test_45_60_75.stl" => "over",
        "temp_tower_pla_180_220.stl" => "temp",
        "celtic_knot_coaster_set.stl" => "celt",
        "hex_organizer_drawer_module.stl" => "hex",
        "gridfinity_baseplate_4x4.stl" => "grid",
        "gridfinity_bin_2x2x4_solid.stl" => "bin",
        _ => "rack",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{SidecarMeta, StlFileInfo, StlType};

    fn entry(path: &str, hash_byte: u8) -> StlFileInfo {
        StlFileInfo {
            path: PathBuf::from(path),
            filename: Path::new(path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            size: 1,
            hash: [hash_byte; 32],
            stl_type: StlType::Binary,
            triangle_count: Some(1),
            dimensions: Some([1.0, 1.0, 1.0]),
            modified: None,
            thumbnail_path: None,
            meta: None,
        }
    }

    #[test]
    fn density_string_values_are_stable() {
        assert_eq!(Density::Small.as_str(), "small");
        assert_eq!(Density::Medium.as_str(), "medium");
        assert_eq!(Density::Large.as_str(), "large");
        assert_eq!(Density::from_str("small"), Density::Small);
        assert_eq!(Density::from_str("medium"), Density::Medium);
        assert_eq!(Density::from_str("large"), Density::Large);
        assert_eq!(Density::from_str("unexpected"), Density::Medium);
    }

    #[test]
    fn view_mode_string_values_are_stable() {
        assert_eq!(ViewMode::Grid.as_str(), "grid");
        assert_eq!(ViewMode::List.as_str(), "list");
        assert_eq!(ViewMode::Masonry.as_str(), "masonry");
        assert_eq!(ViewMode::from_str("grid"), ViewMode::Grid);
        assert_eq!(ViewMode::from_str("list"), ViewMode::List);
        assert_eq!(ViewMode::from_str("masonry"), ViewMode::Masonry);
        assert_eq!(ViewMode::from_str("unexpected"), ViewMode::Grid);
    }

    #[test]
    fn preferences_json_round_trips() {
        let prefs = AppPrefs {
            density: "large".to_string(),
            view_mode: "masonry".to_string(),
            theme: "light".to_string(),
            language: "ko".to_string(),
            slicer_path: "/Applications/PrusaSlicer.app".to_string(),
            last_folder: Some(PathBuf::from("/tmp/models")),
            excluded_folders: vec![PathBuf::from("/tmp/models/archived")],
        };
        let json = serde_json::to_string(&prefs).unwrap();
        let loaded: AppPrefs = serde_json::from_str(&json).unwrap();

        assert_eq!(Density::from_str(&loaded.density), Density::Large);
        assert_eq!(ViewMode::from_str(&loaded.view_mode), ViewMode::Masonry);
        assert_eq!(loaded.last_folder, Some(PathBuf::from("/tmp/models")));
        assert_eq!(
            loaded.excluded_folders,
            vec![PathBuf::from("/tmp/models/archived")]
        );
    }

    #[test]
    fn preferences_defaults_survive_missing_json_fields() {
        let loaded: AppPrefs = serde_json::from_str("{}").unwrap();

        assert_eq!(Density::from_str(&loaded.density), Density::Medium);
        assert_eq!(ViewMode::from_str(&loaded.view_mode), ViewMode::Grid);
        assert_eq!(loaded.theme, "dark");
        assert_eq!(loaded.language, "en");
        assert!(loaded.excluded_folders.is_empty());
    }

    #[test]
    fn sidebar_summary_counts_library_facets() {
        let mut favorite = entry("/tmp/models/a.stl", 1);
        favorite.meta = Some(SidecarMeta {
            favorite: true,
            tags: vec!["fixture".to_string()],
            ..SidecarMeta::default()
        });

        let mut printed = entry("/tmp/models/b.stl", 2);
        printed.meta = Some(SidecarMeta {
            printed: 1,
            tags: vec!["fixture".to_string(), "draft".to_string()],
            ..SidecarMeta::default()
        });

        let mut duplicate = entry("/tmp/models/nested/c.stl", 1);
        duplicate.stl_type = StlType::Unknown;

        let entries = vec![favorite, printed, duplicate];
        let summary = sidebar_summary(&entries);

        assert_eq!(summary.all, 3);
        assert_eq!(summary.favorites, 1);
        assert_eq!(summary.printed, 1);
        assert_eq!(summary.duplicates, 2);
        assert_eq!(summary.ready, 2);
        assert_eq!(summary.errors, 1);

        let folders = sidebar_folders(&entries, Some(Path::new("/tmp/models")));
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0].label, "models");
        assert_eq!(folders[0].count, 3);
        assert_eq!(folders[0].depth, 0);
        assert_eq!(folders[1].label, "nested");
        assert_eq!(folders[1].count, 1);
        assert_eq!(folders[1].depth, 1);

        let tags = sidebar_tags(&entries);
        assert_eq!(
            tags,
            vec![
                SidebarTag {
                    label: "draft".to_string(),
                    count: 1,
                },
                SidebarTag {
                    label: "fixture".to_string(),
                    count: 2,
                },
            ]
        );
    }

    #[test]
    fn app_snapshot_formats_shell_labels() {
        let entries = vec![entry("/tmp/models/a.stl", 1), entry("/tmp/models/b.stl", 2)];
        let prefs = AppPrefs {
            density: "large".to_string(),
            view_mode: "list".to_string(),
            ..AppPrefs::default()
        };

        let snapshot = AppViewSnapshot::from_parts(
            &entries,
            Some(Path::new("/tmp/models")),
            &ScanStatus::Done {
                found: 2,
                skipped: 1,
            },
            &prefs,
            DisplayQuery {
                search_query: "a",
                library_filter: &LibraryFilter::All,
                sort_by: SortBy::Name,
                sort_ascending: true,
                preserve_order: false,
            },
        );

        assert_eq!(snapshot.library_label, "/tmp/models");
        assert_eq!(snapshot.browser.displayed, 1);
        assert_eq!(snapshot.browser.total, 2);
        assert_eq!(snapshot.status_text, "2 models · 1 skipped");
        assert_eq!(snapshot.density_label, "L");
        assert_eq!(snapshot.view_mode_label, "List");
        assert_eq!(snapshot.sort_label, "Name ↑");
        assert_eq!(snapshot.active_filter_key, "all");
    }

    #[test]
    fn display_path_label_compacts_home_relative_paths() {
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
            return;
        };

        assert_eq!(display_path_label(&home), "~");
        assert_eq!(display_path_label(&home.join("Library/3d")), "~/Library/3d");
    }

    #[test]
    fn app_snapshot_labels_missing_folder_as_sample_library() {
        let entries = vec![entry("/tmp/demo/a.stl", 1)];
        let prefs = AppPrefs::default();

        let snapshot = AppViewSnapshot::from_parts(
            &entries,
            None,
            &ScanStatus::Idle,
            &prefs,
            DisplayQuery {
                search_query: "",
                library_filter: &LibraryFilter::All,
                sort_by: SortBy::Name,
                sort_ascending: true,
                preserve_order: false,
            },
        );

        assert_eq!(snapshot.library_label, "Sample library");
        assert!(snapshot.folders.is_empty());
    }

    #[test]
    fn browser_cards_format_model_rows_for_ui_shells() {
        let mut model = entry("/tmp/models/bracket.stl", 1);
        model.size = 2_097_152;
        model.triangle_count = Some(12_400);
        model.meta = Some(SidecarMeta {
            favorite: true,
            printed: 2,
            ..SidecarMeta::default()
        });

        let cards = browser_cards(&[model]);

        assert_eq!(
            cards,
            vec![BrowserCard {
                stable_key: "/tmp/models/bracket.stl".to_string(),
                slot_index: 0,
                title: "bracket.stl".to_string(),
                subtitle: "2.0 MB · 12.4K tris".to_string(),
                author: "You".to_string(),
                relative_modified: "unknown".to_string(),
                thumb_key: "rack".to_string(),
                thumb_path: None,
                badge: "STL".to_string(),
                printed_count: 2,
                favorite: true,
                printed: true,
                error: false,
            }]
        );
    }

    #[test]
    fn smart_filter_keys_round_trip_for_slint_sidebar() {
        for (key, filter) in [
            ("all", LibraryFilter::All),
            ("recent", LibraryFilter::Recent),
            ("favorites", LibraryFilter::Favorites),
            ("printed", LibraryFilter::Printed),
            ("duplicates", LibraryFilter::Duplicates),
            ("ready", LibraryFilter::Ready),
            ("errors", LibraryFilter::Errors),
        ] {
            let loaded = smart_filter_from_key(key).unwrap();
            assert_eq!(loaded, filter);
            assert_eq!(filter_key(&loaded), key);
        }

        assert_eq!(
            smart_filter_from_key("folder:/tmp/models/nested").unwrap(),
            LibraryFilter::Folder(PathBuf::from("/tmp/models/nested"))
        );
        assert_eq!(
            smart_filter_from_key("tag:fixture").unwrap(),
            LibraryFilter::Tag("fixture".to_string())
        );
        assert!(smart_filter_from_key("unknown").is_none());
    }
}
