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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    Name,
    Modified,
    Added,
    Format,
    Size,
    Triangles,
    Dimensions,
    Volume,
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
    #[serde(default = "default_sort_by")]
    pub sort_by: String,
    #[serde(default = "default_sort_ascending")]
    pub sort_ascending: bool,
    #[serde(default = "default_thumbnail_style")]
    pub thumbnail_style: String,
    #[serde(default = "default_thumbnail_lighting")]
    pub thumbnail_lighting: String,
    #[serde(default = "default_thumbnail_aa")]
    pub thumbnail_aa: String,
    #[serde(default = "default_active_printer_keys")]
    pub active_printer_keys: Vec<String>,
    #[serde(default = "default_printer_key")]
    pub default_printer_key: String,
    #[serde(default)]
    pub last_folder: Option<PathBuf>,
    #[serde(default)]
    pub excluded_folders: Vec<PathBuf>,
    #[serde(default)]
    pub collapsed_folders: Vec<PathBuf>,
}

impl Default for AppPrefs {
    fn default() -> Self {
        Self {
            density: default_density(),
            view_mode: default_view_mode(),
            theme: default_theme(),
            language: default_language(),
            slicer_path: String::new(),
            sort_by: default_sort_by(),
            sort_ascending: default_sort_ascending(),
            thumbnail_style: default_thumbnail_style(),
            thumbnail_lighting: default_thumbnail_lighting(),
            thumbnail_aa: default_thumbnail_aa(),
            active_printer_keys: default_active_printer_keys(),
            default_printer_key: default_printer_key(),
            last_folder: None,
            excluded_folders: Vec::new(),
            collapsed_folders: Vec::new(),
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

fn default_sort_by() -> String {
    "name".to_string()
}

fn default_sort_ascending() -> bool {
    true
}

fn default_thumbnail_style() -> String {
    "iso".to_string()
}

fn default_thumbnail_lighting() -> String {
    "studio".to_string()
}

fn default_thumbnail_aa() -> String {
    "msaa4x".to_string()
}

fn default_printer_key() -> String {
    "bambu-p1s-0.4".to_string()
}

fn default_active_printer_keys() -> Vec<String> {
    vec![default_printer_key()]
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
    pub expandable: bool,
    pub expanded: bool,
    pub visible: bool,
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
    pub language: String,
}

impl AppViewSnapshot {
    pub fn from_parts(
        entries: &[scanner::StlFileInfo],
        current_folder: Option<&Path>,
        scan_status: &ScanStatus,
        prefs: &AppPrefs,
        query: DisplayQuery<'_>,
    ) -> Self {
        let language = prefs.language.as_str();
        let filter = filter_label_for_language(query.library_filter, language);
        let active_filter_key = filter_key(query.library_filter);
        let sort_by = query.sort_by;
        let sort_ascending = query.sort_ascending;
        let displayed = filtered_sorted_entries(entries, query);
        Self {
            library_label: titlebar_path(current_folder),
            sidebar: sidebar_summary(entries),
            folders: sidebar_folders(entries, current_folder, &prefs.collapsed_folders),
            tags: sidebar_tags(entries),
            cards: browser_cards_for_language(&displayed, language),
            browser: BrowserSummary {
                displayed: displayed.len(),
                total: entries.len(),
                filter_label: filter,
                empty_message: empty_message(entries, &displayed, language),
            },
            status_text: scan_status_text_for_language(scan_status, language),
            density_label: density_short_label(Density::from_str(&prefs.density)).to_string(),
            view_mode_label: view_mode_title(ViewMode::from_str(&prefs.view_mode)).to_string(),
            sort_label: sort_label_for_language(sort_by, sort_ascending, language),
            active_filter_key,
            language: language.to_string(),
        }
    }
}

pub fn browser_cards_for_language(
    entries: &[scanner::StlFileInfo],
    language: &str,
) -> Vec<BrowserCard> {
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
                subtitle: browser_card_subtitle(entry, language),
                author: entry
                    .meta
                    .as_ref()
                    .and_then(|meta| (!meta.author.is_empty()).then(|| meta.author.clone()))
                    .unwrap_or_else(|| localized("You", "나", "自分", language).to_string()),
                relative_modified: relative_modified_label_for_language(entry.modified, language),
                thumb_key: thumbnail_key(&entry.filename).to_string(),
                thumb_path: entry.thumbnail_path.clone(),
                badge: browser_card_badge(entry, language),
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

fn browser_card_subtitle(entry: &scanner::StlFileInfo, language: &str) -> String {
    let triangle_label = entry
        .triangle_count
        .map(|count| format_triangle_count_for_language(count, language))
        .unwrap_or_else(|| match language_key(language) {
            "ko" => "— 삼각형".to_string(),
            "ja" => "— 三角形".to_string(),
            _ => "— tris".to_string(),
        });
    if let Some(plate_count) = entry.three_mf_plate_count {
        let plate_label = match language_key(language) {
            "ko" => format!("{}개 플레이트", plate_count),
            "ja" => format!("{} プレート", plate_count),
            _ => format!("{} plates", plate_count),
        };
        format!(
            "{} · {} · {}",
            format_size(entry.size),
            plate_label,
            triangle_label
        )
    } else {
        format!("{} · {}", format_size(entry.size), triangle_label)
    }
}

fn browser_card_badge(entry: &scanner::StlFileInfo, language: &str) -> String {
    if let Some(plate_count) = entry.three_mf_plate_count {
        match language_key(language) {
            "ko" => format!("3MF · {}개 플레이트", plate_count),
            "ja" => format!("3MF · {} プレート", plate_count),
            _ => format!("3MF · {} plates", plate_count),
        }
    } else {
        stl_type_label(entry.stl_type).to_string()
    }
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
    collapsed_folders: &[PathBuf],
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

    let collapsed_folders = collapsed_folders
        .iter()
        .filter(|folder| counts.contains_key(*folder))
        .cloned()
        .collect::<Vec<_>>();

    let paths = counts
        .iter()
        .map(|(path, count)| (path.clone(), *count))
        .collect::<Vec<_>>();

    paths
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
            let expandable = counts
                .keys()
                .any(|candidate| candidate.parent() == Some(path.as_path()));
            let expanded = !collapsed_folders.iter().any(|collapsed| collapsed == &path);
            let visible = !collapsed_folders
                .iter()
                .any(|collapsed| path != *collapsed && path.starts_with(collapsed));
            SidebarFolder {
                path,
                label,
                count,
                depth,
                expandable,
                expanded,
                visible,
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

fn language_key(language: &str) -> &str {
    match language {
        "ko" => "ko",
        "ja" => "ja",
        _ => "en",
    }
}

fn localized<'a>(en: &'a str, ko: &'a str, ja: &'a str, language: &str) -> &'a str {
    match language_key(language) {
        "ko" => ko,
        "ja" => ja,
        _ => en,
    }
}

pub fn browser_count_label_for_language(displayed: usize, total: usize, language: &str) -> String {
    if displayed == total {
        match language_key(language) {
            "ko" => format!("{}개 항목", total),
            "ja" => format!("{} 件", total),
            _ => format!("{} items", total),
        }
    } else {
        match language_key(language) {
            "ko" => format!("{} / {}개 항목", displayed, total),
            "ja" => format!("{} / {} 件", displayed, total),
            _ => format!("{} of {} items", displayed, total),
        }
    }
}

pub fn scan_status_text_for_language(status: &ScanStatus, language: &str) -> String {
    match status {
        ScanStatus::Idle => localized("Ready", "준비됨", "準備完了", language).to_string(),
        ScanStatus::Scanning {
            found,
            scanned,
            skipped,
            current,
        } => match language_key(language) {
            "ko" => format!(
                "스캔 중 {} · 발견 {} · 스캔 {} · 건너뜀 {}",
                current, found, scanned, skipped
            ),
            "ja" => format!(
                "スキャン中 {} · 検出 {} · スキャン済み {} · スキップ {}",
                current, found, scanned, skipped
            ),
            _ => format!(
                "Scanning {} · found {} · scanned {} · skipped {}",
                current, found, scanned, skipped
            ),
        },
        ScanStatus::Done { found, skipped } => {
            if *skipped == 0 {
                match language_key(language) {
                    "ko" => format!("{}개 모델", found),
                    "ja" => format!("{} モデル", found),
                    _ => format!("{} models", found),
                }
            } else {
                match language_key(language) {
                    "ko" => format!("{}개 모델 · {}개 건너뜀", found, skipped),
                    "ja" => format!("{} モデル · {} 件スキップ", found, skipped),
                    _ => format!("{} models · {} skipped", found, skipped),
                }
            }
        }
    }
}

pub fn sort_label_for_language(sort_by: SortBy, ascending: bool, language: &str) -> String {
    let field = match sort_by {
        SortBy::Name => localized("Name", "이름", "名前", language),
        SortBy::Modified => localized("Modified", "수정일", "更新日", language),
        SortBy::Added => localized("Added", "추가일", "追加日", language),
        SortBy::Format => localized("Format", "형식", "形式", language),
        SortBy::Size => localized("Size", "크기", "サイズ", language),
        SortBy::Triangles => localized("Triangles", "삼각형", "三角形", language),
        SortBy::Dimensions => localized("Dimensions", "치수", "寸法", language),
        SortBy::Volume => localized("Volume", "부피", "体積", language),
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

fn relative_modified_label_for_language(
    modified: Option<std::time::SystemTime>,
    language: &str,
) -> String {
    let Some(modified) = modified else {
        return localized("unknown", "알 수 없음", "不明", language).to_string();
    };
    let Ok(elapsed) = std::time::SystemTime::now().duration_since(modified) else {
        return localized("today", "오늘", "今日", language).to_string();
    };
    let days = elapsed.as_secs() / 86400;
    if days < 1 {
        localized("today", "오늘", "今日", language).to_string()
    } else if days < 7 {
        match language_key(language) {
            "ko" => format!("{}일 전", days),
            "ja" => format!("{}日前", days),
            _ => format!("{}d ago", days),
        }
    } else if days < 30 {
        match language_key(language) {
            "ko" => format!("{}주 전", days / 7),
            "ja" => format!("{}週間前", days / 7),
            _ => format!("{}w ago", days / 7),
        }
    } else if days < 365 {
        match language_key(language) {
            "ko" => format!("{}개월 전", days / 30),
            "ja" => format!("{}か月前", days / 30),
            _ => format!("{}mo ago", days / 30),
        }
    } else {
        match language_key(language) {
            "ko" => format!("{}년 전", days / 365),
            "ja" => format!("{}年前", days / 365),
            _ => format!("{}y ago", days / 365),
        }
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

fn empty_message(
    entries: &[scanner::StlFileInfo],
    displayed: &[scanner::StlFileInfo],
    language: &str,
) -> String {
    if entries.is_empty() {
        localized(
            "No models yet",
            "아직 모델 없음",
            "モデルがまだありません",
            language,
        )
        .to_string()
    } else if displayed.is_empty() {
        localized(
            "No matching models",
            "일치하는 모델 없음",
            "一致するモデルがありません",
            language,
        )
        .to_string()
    } else {
        match language_key(language) {
            "ko" => format!("{}개 모델 표시 중", displayed.len()),
            "ja" => format!("{} モデル表示中", displayed.len()),
            _ => format!("{} visible models", displayed.len()),
        }
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
        scanner::StlType::Scad => "SCAD",
        scanner::StlType::LargeStl => "LARGE",
        scanner::StlType::Unknown => "ERR",
    }
}

fn format_triangle_count_for_language(count: usize, language: &str) -> String {
    let unit = localized("tris", "삼각형", "三角形", language);
    if count >= 1_000_000 {
        format!("{:.1}M {}", count as f64 / 1_000_000.0, unit)
    } else if count >= 1_000 {
        format!("{:.1}K {}", count as f64 / 1_000.0, unit)
    } else {
        format!("{} {}", count, unit)
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

pub fn filter_label_for_language(filter: &LibraryFilter, language: &str) -> Option<String> {
    match filter {
        LibraryFilter::All => None,
        LibraryFilter::Recent => Some(localized("Recent", "최근", "最近", language).to_string()),
        LibraryFilter::Favorites => {
            Some(localized("Favorites", "즐겨찾기", "お気に入り", language).to_string())
        }
        LibraryFilter::Printed => {
            Some(localized("Printed", "출력됨", "印刷済み", language).to_string())
        }
        LibraryFilter::Duplicates => {
            Some(localized("Duplicates", "중복", "重複", language).to_string())
        }
        LibraryFilter::Ready => {
            Some(localized("Ready", "출력 준비", "印刷準備完了", language).to_string())
        }
        LibraryFilter::Errors => {
            Some(localized("Unparseable", "파싱 오류", "解析エラー", language).to_string())
        }
        LibraryFilter::Folder(folder) => Some(format!(
            "{}: {}",
            localized("Folder", "폴더", "フォルダ", language),
            folder
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(localized("Library", "라이브러리", "ライブラリ", language))
        )),
        LibraryFilter::Tag(tag) => Some(format!(
            "{}: {}",
            localized("Tag", "태그", "タグ", language),
            tag
        )),
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
        sorted.sort_by(|a, b| {
            let ordering = match query.sort_by {
                SortBy::Name => cmp_values(
                    a.filename.to_lowercase(),
                    b.filename.to_lowercase(),
                    query.sort_ascending,
                ),
                SortBy::Modified => cmp_options(a.modified, b.modified, query.sort_ascending),
                SortBy::Added => cmp_options(
                    a.meta.as_ref().and_then(|meta| meta.added.as_deref()),
                    b.meta.as_ref().and_then(|meta| meta.added.as_deref()),
                    query.sort_ascending,
                ),
                SortBy::Format => cmp_values(
                    file_format_rank(a.stl_type),
                    file_format_rank(b.stl_type),
                    query.sort_ascending,
                ),
                SortBy::Size => cmp_values(a.size, b.size, query.sort_ascending),
                SortBy::Triangles => {
                    cmp_options(a.triangle_count, b.triangle_count, query.sort_ascending)
                }
                SortBy::Dimensions => cmp_options(
                    dimension_sort_value(a.dimensions),
                    dimension_sort_value(b.dimensions),
                    query.sort_ascending,
                ),
                SortBy::Volume => cmp_options(
                    volume_sort_value(a.dimensions),
                    volume_sort_value(b.dimensions),
                    query.sort_ascending,
                ),
            };

            ordering.then_with(|| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()))
        });
    }

    sorted.into_iter().cloned().collect()
}

fn cmp_values<T: Ord>(a: T, b: T, ascending: bool) -> std::cmp::Ordering {
    if ascending {
        a.cmp(&b)
    } else {
        b.cmp(&a)
    }
}

fn cmp_options<T: Ord>(a: Option<T>, b: Option<T>, ascending: bool) -> std::cmp::Ordering {
    match (a, b) {
        (Some(a), Some(b)) => cmp_values(a, b, ascending),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

fn file_format_rank(stl_type: scanner::StlType) -> u8 {
    match stl_type {
        scanner::StlType::Binary => 0,
        scanner::StlType::Ascii => 1,
        scanner::StlType::ThreeMf => 2,
        scanner::StlType::Obj => 3,
        scanner::StlType::Step => 4,
        scanner::StlType::Scad => 5,
        scanner::StlType::LargeStl => 6,
        scanner::StlType::Unknown => 7,
    }
}

fn dimension_sort_value(dimensions: Option<[f32; 3]>) -> Option<u32> {
    dimensions.map(|[x, y, z]| x.max(y).max(z).max(0.0).round() as u32)
}

fn volume_sort_value(dimensions: Option<[f32; 3]>) -> Option<u64> {
    dimensions.map(|[x, y, z]| (x.max(0.0) * y.max(0.0) * z.max(0.0)).round() as u64)
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
            three_mf_plate_count: None,
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
            sort_by: "triangles".to_string(),
            sort_ascending: false,
            thumbnail_style: "wire".to_string(),
            thumbnail_lighting: "rim".to_string(),
            thumbnail_aa: "msaa8x".to_string(),
            active_printer_keys: vec!["bambu-p1s-0.4".to_string(), "prusa-mk4-0.4".to_string()],
            default_printer_key: "prusa-mk4-0.4".to_string(),
            last_folder: Some(PathBuf::from("/tmp/models")),
            excluded_folders: vec![PathBuf::from("/tmp/models/archived")],
            collapsed_folders: vec![PathBuf::from("/tmp/models/nested")],
        };
        let json = serde_json::to_string(&prefs).unwrap();
        let loaded: AppPrefs = serde_json::from_str(&json).unwrap();

        assert_eq!(Density::from_str(&loaded.density), Density::Large);
        assert_eq!(ViewMode::from_str(&loaded.view_mode), ViewMode::Masonry);
        assert_eq!(loaded.sort_by, "triangles");
        assert!(!loaded.sort_ascending);
        assert_eq!(loaded.thumbnail_style, "wire");
        assert_eq!(loaded.thumbnail_lighting, "rim");
        assert_eq!(loaded.thumbnail_aa, "msaa8x");
        assert_eq!(loaded.last_folder, Some(PathBuf::from("/tmp/models")));
        assert_eq!(
            loaded.active_printer_keys,
            vec!["bambu-p1s-0.4".to_string(), "prusa-mk4-0.4".to_string()]
        );
        assert_eq!(loaded.default_printer_key, "prusa-mk4-0.4");
        assert_eq!(
            loaded.excluded_folders,
            vec![PathBuf::from("/tmp/models/archived")]
        );
        assert_eq!(
            loaded.collapsed_folders,
            vec![PathBuf::from("/tmp/models/nested")]
        );
    }

    #[test]
    fn preferences_defaults_survive_missing_json_fields() {
        let loaded: AppPrefs = serde_json::from_str("{}").unwrap();

        assert_eq!(Density::from_str(&loaded.density), Density::Medium);
        assert_eq!(ViewMode::from_str(&loaded.view_mode), ViewMode::Grid);
        assert_eq!(loaded.theme, "dark");
        assert_eq!(loaded.language, "en");
        assert_eq!(loaded.sort_by, "name");
        assert!(loaded.sort_ascending);
        assert_eq!(loaded.thumbnail_style, "iso");
        assert_eq!(loaded.thumbnail_lighting, "studio");
        assert_eq!(loaded.thumbnail_aa, "msaa4x");
        assert_eq!(
            loaded.active_printer_keys,
            vec!["bambu-p1s-0.4".to_string()]
        );
        assert_eq!(loaded.default_printer_key, "bambu-p1s-0.4");
        assert!(loaded.excluded_folders.is_empty());
        assert!(loaded.collapsed_folders.is_empty());
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

        let folders = sidebar_folders(&entries, Some(Path::new("/tmp/models")), &[]);
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0].label, "models");
        assert_eq!(folders[0].count, 3);
        assert_eq!(folders[0].depth, 0);
        assert!(folders[0].expandable);
        assert!(folders[0].expanded);
        assert!(folders[0].visible);
        assert_eq!(folders[1].label, "nested");
        assert_eq!(folders[1].count, 1);
        assert_eq!(folders[1].depth, 1);
        assert!(!folders[1].expandable);
        assert!(folders[1].visible);

        let collapsed = sidebar_folders(
            &entries,
            Some(Path::new("/tmp/models")),
            &[PathBuf::from("/tmp/models")],
        );
        assert_eq!(collapsed.len(), 2);
        assert_eq!(collapsed[0].label, "models");
        assert!(collapsed[0].expandable);
        assert!(!collapsed[0].expanded);
        assert!(collapsed[0].visible);
        assert_eq!(collapsed[1].label, "nested");
        assert!(!collapsed[1].visible);

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
        assert_eq!(snapshot.language, "en");
    }

    #[test]
    fn app_snapshot_localizes_korean_shell_labels() {
        let mut model = entry("/tmp/models/project.3mf", 4);
        model.stl_type = StlType::ThreeMf;
        model.size = 1024;
        model.triangle_count = Some(2_000);
        model.three_mf_plate_count = Some(3);
        model.modified =
            Some(std::time::SystemTime::now() - std::time::Duration::from_secs(3 * 365 * 86400));
        let prefs = AppPrefs {
            language: "ko".to_string(),
            ..AppPrefs::default()
        };

        let snapshot = AppViewSnapshot::from_parts(
            &[model],
            Some(Path::new("/tmp/models")),
            &ScanStatus::Done {
                found: 1,
                skipped: 0,
            },
            &prefs,
            DisplayQuery {
                search_query: "",
                library_filter: &LibraryFilter::All,
                sort_by: SortBy::Modified,
                sort_ascending: false,
                preserve_order: false,
            },
        );

        assert_eq!(snapshot.status_text, "1개 모델");
        assert_eq!(snapshot.sort_label, "수정일 ↓");
        assert_eq!(snapshot.browser.empty_message, "1개 모델 표시 중");
        assert_eq!(snapshot.cards[0].relative_modified, "3년 전");
        assert_eq!(
            snapshot.cards[0].subtitle,
            "1.0 KB · 3개 플레이트 · 2.0K 삼각형"
        );
        assert_eq!(snapshot.cards[0].badge, "3MF · 3개 플레이트");
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
    fn filtered_sort_supports_added_and_geometry_fields() {
        let mut small = entry("/tmp/models/small.stl", 1);
        small.meta = Some(SidecarMeta {
            added: Some("2026-05-03".to_string()),
            ..SidecarMeta::default()
        });
        small.dimensions = Some([10.0, 20.0, 30.0]);
        small.triangle_count = Some(120);

        let mut large = entry("/tmp/models/large.3mf", 2);
        large.stl_type = StlType::ThreeMf;
        large.meta = Some(SidecarMeta {
            added: Some("2026-05-01".to_string()),
            ..SidecarMeta::default()
        });
        large.dimensions = Some([80.0, 40.0, 20.0]);
        large.triangle_count = Some(4_000);

        let entries = vec![small.clone(), large.clone()];
        let added = filtered_sorted_entries(
            &entries,
            DisplayQuery {
                search_query: "",
                library_filter: &LibraryFilter::All,
                sort_by: SortBy::Added,
                sort_ascending: true,
                preserve_order: false,
            },
        );
        assert_eq!(added[0].filename, "large.3mf");

        let dimensions = filtered_sorted_entries(
            &entries,
            DisplayQuery {
                search_query: "",
                library_filter: &LibraryFilter::All,
                sort_by: SortBy::Dimensions,
                sort_ascending: false,
                preserve_order: false,
            },
        );
        assert_eq!(dimensions[0].filename, "large.3mf");

        let volume = filtered_sorted_entries(
            &entries,
            DisplayQuery {
                search_query: "",
                library_filter: &LibraryFilter::All,
                sort_by: SortBy::Volume,
                sort_ascending: true,
                preserve_order: false,
            },
        );
        assert_eq!(volume[0].filename, "small.stl");
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

        let cards = browser_cards_for_language(&[model], "en");

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
    fn browser_cards_badge_multi_plate_3mf_files() {
        let mut model = entry("/tmp/models/project.3mf", 4);
        model.stl_type = StlType::ThreeMf;
        model.size = 1024;
        model.triangle_count = Some(2_000);
        model.three_mf_plate_count = Some(3);

        let cards = browser_cards_for_language(&[model], "en");

        assert_eq!(cards[0].subtitle, "1.0 KB · 3 plates · 2.0K tris");
        assert_eq!(cards[0].badge, "3MF · 3 plates");
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
