use std::collections::{BTreeMap, HashMap, HashSet};
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
    #[serde(default = "default_accent_color")]
    pub accent_color: String,
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
    #[serde(default = "default_card_label_mode")]
    pub card_label_mode: String,
    #[serde(default = "default_date_format_mode")]
    pub date_format_mode: String,
    #[serde(default = "default_show_file_extensions")]
    pub show_file_extensions: bool,
    #[serde(default = "default_startup_view")]
    pub startup_view: String,
    #[serde(default)]
    pub last_folder: Option<PathBuf>,
    /// Top-level library folders (user-added roots). Persisted; merged with legacy `last_folder` on load when empty.
    #[serde(default)]
    pub library_folders: Vec<PathBuf>,
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
            accent_color: default_accent_color(),
            language: default_language(),
            slicer_path: String::new(),
            sort_by: default_sort_by(),
            sort_ascending: default_sort_ascending(),
            thumbnail_style: default_thumbnail_style(),
            thumbnail_lighting: default_thumbnail_lighting(),
            thumbnail_aa: default_thumbnail_aa(),
            active_printer_keys: default_active_printer_keys(),
            default_printer_key: default_printer_key(),
            card_label_mode: default_card_label_mode(),
            date_format_mode: default_date_format_mode(),
            show_file_extensions: default_show_file_extensions(),
            startup_view: default_startup_view(),
            last_folder: None,
            library_folders: Vec::new(),
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

fn default_accent_color() -> String {
    "teal".to_string()
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

fn default_card_label_mode() -> String {
    "filename".to_string()
}

fn default_date_format_mode() -> String {
    "auto".to_string()
}

fn default_show_file_extensions() -> bool {
    true
}

fn default_startup_view() -> String {
    "last".to_string()
}

/// Card label rendering mode controlling how the model card title is built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardLabelMode {
    /// Show the filename as-is, optionally including the file extension.
    Filename,
    /// Prefer the sidecar-provided title, falling back to a title-cased filename stem.
    Titled,
}

impl CardLabelMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Filename => "filename",
            Self::Titled => "titled",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "titled" | "title" | "title-cased" => Self::Titled,
            _ => Self::Filename,
        }
    }
}

/// Timestamp formatting choice used for list/detail rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateFormatMode {
    /// Localized relative phrasing ("3d ago", "오늘", "今日").
    Auto,
    /// ISO 8601 (`YYYY-MM-DD`).
    Iso,
    /// US-style (`Month D, YYYY`).
    Us,
}

impl DateFormatMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Iso => "iso",
            Self::Us => "us",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "iso" => Self::Iso,
            "us" => Self::Us,
            _ => Self::Auto,
        }
    }
}

#[derive(Clone, Copy)]
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
        library_roots: &[PathBuf],
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
            library_label: titlebar_for_library_roots(library_roots, language),
            sidebar: sidebar_summary(entries),
            folders: sidebar_folders(entries, library_roots, &prefs.collapsed_folders),
            tags: sidebar_tags(entries),
            cards: browser_cards_for_prefs(&displayed, prefs),
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

    /// Like [`AppViewSnapshot::from_parts`], but uses a precomputed `displayed` slice so callers
    /// can avoid repeating an expensive `filtered_sorted_entries` pass (for example during
    /// streaming library scans).
    pub fn from_parts_with_displayed_slice(
        entries: &[scanner::StlFileInfo],
        displayed: &[scanner::StlFileInfo],
        library_roots: &[PathBuf],
        scan_status: &ScanStatus,
        prefs: &AppPrefs,
        query: DisplayQuery<'_>,
    ) -> Self {
        let language = prefs.language.as_str();
        let filter = filter_label_for_language(query.library_filter, language);
        let active_filter_key = filter_key(query.library_filter);
        let sort_by = query.sort_by;
        let sort_ascending = query.sort_ascending;
        Self {
            library_label: titlebar_for_library_roots(library_roots, language),
            sidebar: sidebar_summary(entries),
            folders: sidebar_folders(entries, library_roots, &prefs.collapsed_folders),
            tags: sidebar_tags(entries),
            cards: browser_cards_for_prefs(displayed, prefs),
            browser: BrowserSummary {
                displayed: displayed.len(),
                total: entries.len(),
                filter_label: filter,
                empty_message: empty_message(entries, displayed, language),
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

/// Convenience wrapper used by tests and external callers that only have a
/// language string at hand. Production code prefers
/// [`browser_cards_for_prefs`].
#[allow(dead_code)]
pub fn browser_cards_for_language(
    entries: &[scanner::StlFileInfo],
    language: &str,
) -> Vec<BrowserCard> {
    let prefs = AppPrefs {
        language: language.to_string(),
        ..AppPrefs::default()
    };
    browser_cards_for_prefs(entries, &prefs)
}

pub fn browser_cards_for_prefs(
    entries: &[scanner::StlFileInfo],
    prefs: &AppPrefs,
) -> Vec<BrowserCard> {
    let language = prefs.language.as_str();
    let label_mode = CardLabelMode::from_str(&prefs.card_label_mode);
    let date_mode = DateFormatMode::from_str(&prefs.date_format_mode);
    let show_extension = prefs.show_file_extensions;
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
                title: card_label_for_entry(entry, label_mode, show_extension),
                subtitle: browser_card_subtitle(entry, language),
                author: entry
                    .meta
                    .as_ref()
                    .and_then(|meta| (!meta.author.is_empty()).then(|| meta.author.clone()))
                    .unwrap_or_else(|| localized("You", "나", "自分", language).to_string()),
                relative_modified: format_modified_label(entry.modified, date_mode, language),
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

/// Render the user-facing card title from a scan entry, honoring the configured
/// label mode and file-extension toggle. Falls back to the filename stem when a
/// sidecar title is not available.
pub fn card_label_for_entry(
    entry: &scanner::StlFileInfo,
    mode: CardLabelMode,
    show_extension: bool,
) -> String {
    let filename = entry.filename.as_str();
    let (stem, extension) = split_filename(filename);
    let sidecar_title = entry
        .meta
        .as_ref()
        .map(|meta| meta.title.trim())
        .filter(|title| !title.is_empty());

    match mode {
        CardLabelMode::Filename => {
            if show_extension {
                filename.to_string()
            } else {
                stem.to_string()
            }
        }
        CardLabelMode::Titled => {
            if let Some(title) = sidecar_title {
                if show_extension && !extension.is_empty() {
                    format!("{title}.{extension}")
                } else {
                    title.to_string()
                }
            } else {
                let titled = humanize_filename_stem(stem);
                if show_extension && !extension.is_empty() {
                    format!("{titled}.{extension}")
                } else {
                    titled
                }
            }
        }
    }
}

fn split_filename(filename: &str) -> (&str, &str) {
    match filename.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() && !extension.contains('/') => {
            (stem, extension)
        }
        _ => (filename, ""),
    }
}

/// Turn a filename stem into a friendlier, title-cased label by splitting on
/// common separators and capitalizing each fragment.
pub fn humanize_filename_stem(stem: &str) -> String {
    let cleaned: String = stem
        .chars()
        .map(|c| match c {
            '_' | '-' | '.' => ' ',
            _ => c,
        })
        .collect();
    let mut out = String::with_capacity(cleaned.len());
    for (idx, word) in cleaned.split_whitespace().enumerate() {
        if word.is_empty() {
            continue;
        }
        if idx > 0 {
            out.push(' ');
        }
        let lower = word.to_lowercase();
        let mut chars = lower.chars();
        if let Some(first) = chars.next() {
            for c in first.to_uppercase() {
                out.push(c);
            }
        }
        out.push_str(chars.as_str());
    }
    if out.is_empty() {
        stem.to_string()
    } else {
        out
    }
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

/// Deduplicate library roots by canonical path (when resolvable), preserving first occurrence
/// order, then sort lexically by path components for stable UI.
pub fn dedupe_library_roots(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for r in roots {
        let key = std::fs::canonicalize(r).unwrap_or_else(|_| r.clone());
        if seen.insert(key) {
            out.push(r.clone());
        }
    }
    out.sort_by(|a, b| a.components().cmp(b.components()));
    out
}

/// One sidebar row per configured library root: counts only entries under that root
/// (`strip_prefix`), never mixing in paths from sibling roots.
pub fn sidebar_folders(
    entries: &[scanner::StlFileInfo],
    roots: &[PathBuf],
    collapsed_folders: &[PathBuf],
) -> Vec<SidebarFolder> {
    let roots = dedupe_library_roots(roots);
    if roots.is_empty() {
        return Vec::new();
    }
    roots
        .into_iter()
        .map(|path| {
            let count = entries
                .iter()
                .filter(|e| e.path.strip_prefix(&path).is_ok())
                .count();
            let label = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Library")
                .to_string();
            let expanded = !collapsed_folders.iter().any(|collapsed| collapsed == &path);
            let visible = !collapsed_folders.iter().any(|collapsed| {
                path != *collapsed && path.starts_with(collapsed)
            });
            SidebarFolder {
                path,
                label,
                count,
                depth: 0,
                expandable: false,
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

/// Public helper for rendering a modification timestamp using the user's
/// chosen date-format preference. Falls back to the localized "auto" relative
/// label when an absolute date cannot be derived.
pub fn format_modified_label(
    modified: Option<std::time::SystemTime>,
    mode: DateFormatMode,
    language: &str,
) -> String {
    match mode {
        DateFormatMode::Auto => relative_modified_label_for_language(modified, language),
        DateFormatMode::Iso => absolute_modified_label(modified, language, DateFormatMode::Iso),
        DateFormatMode::Us => absolute_modified_label(modified, language, DateFormatMode::Us),
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

fn absolute_modified_label(
    modified: Option<std::time::SystemTime>,
    language: &str,
    mode: DateFormatMode,
) -> String {
    let Some(modified) = modified else {
        return localized("unknown", "알 수 없음", "不明", language).to_string();
    };
    let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) else {
        return relative_modified_label_for_language(Some(modified), language);
    };
    let (year, month, day) = civil_date_from_unix_seconds(duration.as_secs() as i64);
    match mode {
        DateFormatMode::Iso => format!("{year:04}-{month:02}-{day:02}"),
        DateFormatMode::Us => match language_key(language) {
            "ko" => format!("{year}년 {month}월 {day}일"),
            "ja" => format!("{year}年{month}月{day}日"),
            _ => format!("{} {}, {}", us_month_name(month), day, year),
        },
        DateFormatMode::Auto => relative_modified_label_for_language(Some(modified), language),
    }
}

fn us_month_name(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "—",
    }
}

/// Convert a Unix timestamp (seconds since epoch, may be negative) into a
/// civil (proleptic Gregorian) `(year, month, day)` tuple. Avoids any chrono
/// dependency so we stay within the existing minimal-stdlib budget.
/// Implements Howard Hinnant's `date::civil_from_days` (public domain).
fn civil_date_from_unix_seconds(seconds: i64) -> (i32, u32, u32) {
    let mut days = seconds.div_euclid(86_400);
    days += 719_468;
    let era = days.div_euclid(146_097);
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
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
        LibraryFilter::Folder(folder) => entry.path.strip_prefix(folder).is_ok(),
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

fn titlebar_for_library_roots(roots: &[PathBuf], language: &str) -> String {
    match roots.len() {
        0 => localized(
            "Sample library",
            "샘플 라이브러리",
            "サンプルライブラリ",
            language,
        )
        .to_string(),
        1 => display_path_label(&roots[0]),
        n => {
            let joined = roots
                .iter()
                .map(|p| display_path_label(p))
                .collect::<Vec<_>>()
                .join(", ");
            match language_key(language) {
                "ko" => format!("라이브러리 폴더 {n}개 · {joined}"),
                "ja" => format!("ライブラリフォルダ {n} 件 · {joined}"),
                _ => format!("{n} library folders · {joined}"),
            }
        }
    }
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

    let duplicate_members: Option<HashSet<[u8; 32]>> =
        if matches!(query.library_filter, LibraryFilter::Duplicates) {
            let mut counts: HashMap<[u8; 32], usize> = HashMap::new();
            for entry in entries {
                *counts.entry(entry.hash).or_insert(0) += 1;
            }
            Some(
                counts
                    .into_iter()
                    .filter_map(|(h, c)| (c > 1).then_some(h))
                    .collect(),
            )
        } else {
            None
        };

    let mut sorted: Vec<&scanner::StlFileInfo> = entries
        .iter()
        .filter(|entry| {
            let passes_filter = match query.library_filter {
                LibraryFilter::Duplicates => duplicate_members
                    .as_ref()
                    .is_some_and(|set| set.contains(&entry.hash)),
                _ => entry_matches_filter(entries, query.library_filter, entry),
            };
            passes_filter
                && (filter_lower.is_empty()
                    || entry.filename.to_lowercase().contains(&filter_lower)
                    || entry.meta.as_ref().is_some_and(|meta| {
                        meta.tags.iter().any(|tag| {
                            tag.to_lowercase().contains(&filter_lower)
                        }) || meta.notes.to_lowercase().contains(&filter_lower)
                    }))
        })
        .collect();

    if !query.preserve_order {
        match query.sort_by {
            SortBy::Name => {
                let mut keyed: Vec<(String, &scanner::StlFileInfo)> = sorted
                    .into_iter()
                    .map(|e| (e.filename.to_lowercase(), e))
                    .collect();
                keyed.sort_by(|(la, _), (lb, _)| {
                    if query.sort_ascending {
                        la.cmp(lb)
                    } else {
                        lb.cmp(la)
                    }
                });
                sorted = keyed.into_iter().map(|(_, e)| e).collect();
            }
            _ => {
                let sort_by = query.sort_by;
                sorted.sort_by(|a, b| {
                    let ordering = match sort_by {
                        SortBy::Modified => {
                            cmp_options(a.modified, b.modified, query.sort_ascending)
                        }
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
                        SortBy::Name => std::cmp::Ordering::Equal,
                    };

                    ordering.then_with(|| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()))
                });
            }
        }
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
            accent_color: "purple".to_string(),
            language: "ko".to_string(),
            slicer_path: "/Applications/PrusaSlicer.app".to_string(),
            sort_by: "triangles".to_string(),
            sort_ascending: false,
            thumbnail_style: "wire".to_string(),
            thumbnail_lighting: "rim".to_string(),
            thumbnail_aa: "msaa8x".to_string(),
            active_printer_keys: vec!["bambu-p1s-0.4".to_string(), "prusa-mk4-0.4".to_string()],
            default_printer_key: "prusa-mk4-0.4".to_string(),
            card_label_mode: "titled".to_string(),
            date_format_mode: "iso".to_string(),
            show_file_extensions: false,
            startup_view: "empty".to_string(),
            last_folder: Some(PathBuf::from("/tmp/models")),
            library_folders: vec![PathBuf::from("/tmp/other-lib")],
            excluded_folders: vec![PathBuf::from("/tmp/models/archived")],
            collapsed_folders: vec![PathBuf::from("/tmp/models/nested")],
        };
        let json = serde_json::to_string(&prefs).unwrap();
        let loaded: AppPrefs = serde_json::from_str(&json).unwrap();

        assert_eq!(Density::from_str(&loaded.density), Density::Large);
        assert_eq!(ViewMode::from_str(&loaded.view_mode), ViewMode::Masonry);
        assert_eq!(loaded.accent_color, "purple");
        assert_eq!(loaded.sort_by, "triangles");
        assert!(!loaded.sort_ascending);
        assert_eq!(loaded.thumbnail_style, "wire");
        assert_eq!(loaded.thumbnail_lighting, "rim");
        assert_eq!(loaded.thumbnail_aa, "msaa8x");
        assert_eq!(loaded.last_folder, Some(PathBuf::from("/tmp/models")));
        assert_eq!(
            loaded.library_folders,
            vec![PathBuf::from("/tmp/other-lib")]
        );
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
        assert_eq!(loaded.accent_color, "teal");
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
        assert!(loaded.library_folders.is_empty());
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

        let folders = sidebar_folders(&entries, &[PathBuf::from("/tmp/models")], &[]);
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].label, "models");
        assert_eq!(folders[0].count, 3);
        assert_eq!(folders[0].depth, 0);
        assert!(!folders[0].expandable);
        assert!(folders[0].expanded);
        assert!(folders[0].visible);

        let collapsed = sidebar_folders(
            &entries,
            &[PathBuf::from("/tmp/models")],
            &[PathBuf::from("/tmp/models")],
        );
        assert_eq!(collapsed.len(), 1);
        assert_eq!(collapsed[0].label, "models");
        assert!(!collapsed[0].expandable);
        assert!(!collapsed[0].expanded);
        assert!(collapsed[0].visible);

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
    fn sidebar_folders_two_roots_counts_are_isolated() {
        let a = PathBuf::from("/tmp/mr-lib-a");
        let b = PathBuf::from("/tmp/mr-lib-b");
        let entries = vec![
            entry(a.join("x.stl").to_str().unwrap(), 1),
            entry(a.join("sub/y.stl").to_str().unwrap(), 2),
            entry(b.join("only.stl").to_str().unwrap(), 3),
        ];
        let roots = vec![a, b];
        let folders = sidebar_folders(&entries, &roots, &[]);
        assert_eq!(folders.len(), 2, "{folders:?}");
        let mut counts: Vec<(String, usize)> = folders
            .iter()
            .map(|f| (f.label.clone(), f.count))
            .collect();
        counts.sort_by(|x, y| x.0.cmp(&y.0));
        assert_eq!(counts[0], ("mr-lib-a".to_string(), 2));
        assert_eq!(counts[1], ("mr-lib-b".to_string(), 1));
        assert!(folders.iter().all(|f| f.depth == 0 && !f.expandable));
    }

    #[test]
    fn sidebar_folders_dedupes_identical_root_paths() {
        let root = PathBuf::from("/tmp/mr-dedupe-root");
        let entries = vec![entry(root.join("a.stl").to_str().unwrap(), 1)];
        let folders = sidebar_folders(&entries, &[root.clone(), root], &[]);
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].count, 1);
    }

    #[test]
    fn sidebar_folders_prefix_boundary_does_not_absorb_sibling_path() {
        let models = PathBuf::from("/tmp/mr-models");
        let models_extra = PathBuf::from("/tmp/mr-models-extra");
        let entries = vec![entry(models_extra.join("z.stl").to_str().unwrap(), 1)];
        let folders = sidebar_folders(&entries, &[models.clone(), models_extra.clone()], &[]);
        let short_root = folders.iter().find(|f| f.label == "mr-models").unwrap();
        let longer_root = folders
            .iter()
            .find(|f| f.label == "mr-models-extra")
            .unwrap();
        assert_eq!(short_root.count, 0);
        assert_eq!(longer_root.count, 1);
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
            &[PathBuf::from("/tmp/models")],
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
            &[PathBuf::from("/tmp/models")],
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
    fn app_snapshot_from_parts_matches_from_parts_with_displayed_slice() {
        let a = entry("/tmp/models/a.stl", 1);
        let b = entry("/tmp/models/b.stl", 2);
        let entries = vec![a, b];
        let roots = vec![PathBuf::from("/tmp/models")];
        let prefs = AppPrefs::default();
        let query = DisplayQuery {
            search_query: "",
            library_filter: &LibraryFilter::All,
            sort_by: SortBy::Name,
            sort_ascending: true,
            preserve_order: true,
        };
        let displayed = filtered_sorted_entries(&entries, query);
        let status = ScanStatus::Done {
            found: 2,
            skipped: 0,
        };
        let query_a = DisplayQuery {
            search_query: "",
            library_filter: &LibraryFilter::All,
            sort_by: SortBy::Name,
            sort_ascending: true,
            preserve_order: true,
        };
        let s1 = AppViewSnapshot::from_parts(&entries, &roots, &status, &prefs, query_a);
        let query_b = DisplayQuery {
            search_query: "",
            library_filter: &LibraryFilter::All,
            sort_by: SortBy::Name,
            sort_ascending: true,
            preserve_order: true,
        };
        let s2 = AppViewSnapshot::from_parts_with_displayed_slice(
            &entries,
            &displayed,
            &roots,
            &status,
            &prefs,
            query_b,
        );
        assert_eq!(s1.cards.len(), s2.cards.len());
        assert_eq!(s1.browser.displayed, s2.browser.displayed);
        assert_eq!(s1.browser.total, s2.browser.total);
        assert_eq!(s1.status_text, s2.status_text);
    }

    #[test]
    fn large_library_snapshot_matches_with_precomputed_displayed() {
        let mut entries = Vec::new();
        for i in 0..600 {
            let path = format!("/tmp/models/m_{i:04}.stl");
            entries.push(entry(&path, (i % 255) as u8));
        }
        let roots = vec![PathBuf::from("/tmp/models")];
        let prefs = AppPrefs::default();
        let query = DisplayQuery {
            search_query: "",
            library_filter: &LibraryFilter::All,
            sort_by: SortBy::Name,
            sort_ascending: true,
            preserve_order: false,
        };
        let displayed = filtered_sorted_entries(&entries, query);
        let status = ScanStatus::Done {
            found: entries.len(),
            skipped: 0,
        };
        let full = AppViewSnapshot::from_parts(&entries, &roots, &status, &prefs, query);
        let cheap = AppViewSnapshot::from_parts_with_displayed_slice(
            &entries,
            &displayed,
            &roots,
            &status,
            &prefs,
            query,
        );
        assert_eq!(full.cards.len(), cheap.cards.len());
        assert_eq!(full.browser.displayed, cheap.browser.displayed);
        assert_eq!(full.browser.total, cheap.browser.total);
        assert_eq!(full.active_filter_key, cheap.active_filter_key);
    }

    #[test]
    fn duplicates_filter_counts_hashes_once() {
        let mut entries = vec![
            entry("/lib/a.stl", 7),
            entry("/lib/b.stl", 7),
            entry("/lib/c.stl", 8),
        ];
        for i in 0..120 {
            entries.push(entry(
                &format!("/lib/u{i}.stl"),
                (i as u8).wrapping_add(50),
            ));
        }
        let query = DisplayQuery {
            search_query: "",
            library_filter: &LibraryFilter::Duplicates,
            sort_by: SortBy::Name,
            sort_ascending: true,
            preserve_order: false,
        };
        let out = filtered_sorted_entries(&entries, query);
        assert_eq!(out.len(), 2);
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
            &[],
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

    #[test]
    fn card_label_respects_filename_mode_and_extension_toggle() {
        let model = entry("/tmp/models/Hex_grip-v2.stl", 7);
        // Filename mode + extension visible → exact filename.
        assert_eq!(
            card_label_for_entry(&model, CardLabelMode::Filename, true),
            "Hex_grip-v2.stl"
        );
        // Filename mode + extension hidden → stem only.
        assert_eq!(
            card_label_for_entry(&model, CardLabelMode::Filename, false),
            "Hex_grip-v2"
        );
    }

    #[test]
    fn card_label_prefers_sidecar_title_in_titled_mode_else_humanizes_stem() {
        let mut titled = entry("/tmp/models/spool_holder.stl", 8);
        titled.meta = Some(SidecarMeta {
            title: "Smooth Spool Holder".to_string(),
            ..SidecarMeta::default()
        });
        assert_eq!(
            card_label_for_entry(&titled, CardLabelMode::Titled, false),
            "Smooth Spool Holder"
        );
        // Extension toggle should append the original extension.
        assert_eq!(
            card_label_for_entry(&titled, CardLabelMode::Titled, true),
            "Smooth Spool Holder.stl"
        );

        // No sidecar title → humanized stem.
        let plain = entry("/tmp/models/hex_grip-v2.stl", 9);
        assert_eq!(
            card_label_for_entry(&plain, CardLabelMode::Titled, false),
            "Hex Grip V2"
        );
    }

    #[test]
    fn browser_cards_for_prefs_apply_card_label_and_extension_settings() {
        let mut model = entry("/tmp/models/cool_thing.stl", 3);
        model.size = 1024;
        model.triangle_count = Some(2_000);
        model.meta = Some(SidecarMeta {
            title: "Cool Thing v2".to_string(),
            ..SidecarMeta::default()
        });

        let mut prefs = AppPrefs::default();
        prefs.card_label_mode = "titled".to_string();
        prefs.show_file_extensions = false;
        let cards = browser_cards_for_prefs(&[model.clone()], &prefs);
        assert_eq!(cards[0].title, "Cool Thing v2");

        prefs.card_label_mode = "filename".to_string();
        prefs.show_file_extensions = true;
        let cards = browser_cards_for_prefs(&[model.clone()], &prefs);
        assert_eq!(cards[0].title, "cool_thing.stl");

        prefs.card_label_mode = "filename".to_string();
        prefs.show_file_extensions = false;
        let cards = browser_cards_for_prefs(&[model], &prefs);
        assert_eq!(cards[0].title, "cool_thing");
    }

    #[test]
    fn format_modified_label_iso_and_us_modes_render_civil_date() {
        // 2024-03-15 00:00:00 UTC.
        let epoch = std::time::UNIX_EPOCH
            + std::time::Duration::from_secs(1710460800);

        assert_eq!(
            format_modified_label(Some(epoch), DateFormatMode::Iso, "en"),
            "2024-03-15"
        );
        assert_eq!(
            format_modified_label(Some(epoch), DateFormatMode::Us, "en"),
            "Mar 15, 2024"
        );
        assert_eq!(
            format_modified_label(Some(epoch), DateFormatMode::Us, "ko"),
            "2024년 3월 15일"
        );

        // Auto mode (today) should not panic and yields a localized string.
        let now = std::time::SystemTime::now();
        let label = format_modified_label(Some(now), DateFormatMode::Auto, "ko");
        assert!(!label.is_empty());
    }
}
