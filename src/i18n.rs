use std::collections::HashMap;
use std::sync::OnceLock;

use crate::slint_shell::ModelRackWindow;

static EN: OnceLock<HashMap<String, String>> = OnceLock::new();
static KO: OnceLock<HashMap<String, String>> = OnceLock::new();
static JA: OnceLock<HashMap<String, String>> = OnceLock::new();
static ES: OnceLock<HashMap<String, String>> = OnceLock::new();
static ZH_CN: OnceLock<HashMap<String, String>> = OnceLock::new();
static ZH_TW: OnceLock<HashMap<String, String>> = OnceLock::new();
static PT: OnceLock<HashMap<String, String>> = OnceLock::new();
static RU: OnceLock<HashMap<String, String>> = OnceLock::new();

fn en_map() -> &'static HashMap<String, String> {
    EN.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/i18n/en.json")).expect("invalid en.json")
    })
}

fn ko_map() -> &'static HashMap<String, String> {
    KO.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/i18n/ko.json")).expect("invalid ko.json")
    })
}

fn ja_map() -> &'static HashMap<String, String> {
    JA.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/i18n/ja.json")).expect("invalid ja.json")
    })
}

fn es_map() -> &'static HashMap<String, String> {
    ES.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/i18n/es.json")).expect("invalid es.json")
    })
}

fn zh_cn_map() -> &'static HashMap<String, String> {
    ZH_CN.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/i18n/zh-CN.json")).expect("invalid zh-CN.json")
    })
}

fn zh_tw_map() -> &'static HashMap<String, String> {
    ZH_TW.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/i18n/zh-TW.json")).expect("invalid zh-TW.json")
    })
}

fn pt_map() -> &'static HashMap<String, String> {
    PT.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/i18n/pt.json")).expect("invalid pt.json")
    })
}

fn ru_map() -> &'static HashMap<String, String> {
    RU.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/i18n/ru.json")).expect("invalid ru.json")
    })
}

/// Look up a translation key for the given language, falling back to English.
pub fn tr<'a>(key: &'a str, language: &str) -> &'a str {
    let map = match language {
        "ko" => ko_map(),
        "ja" => ja_map(),
        "es" => es_map(),
        "zh-CN" => zh_cn_map(),
        "zh-TW" => zh_tw_map(),
        "pt" => pt_map(),
        "ru" => ru_map(),
        _ => en_map(),
    };
    if let Some(value) = map.get(key) {
        // SAFETY: The maps are 'static (OnceLock), so the &str lives for 'static,
        // which satisfies any 'a the caller needs.
        unsafe { &*(value.as_str() as *const str) }
    } else if let Some(value) = en_map().get(key) {
        unsafe { &*(value.as_str() as *const str) }
    } else {
        key.rsplit('/').next().unwrap_or(key)
    }
}

/// Push every `tr-*` property into the Slint UI for the given language.
pub fn apply_translations(ui: &ModelRackWindow, lang: &str) {
    ui.set_tr_about_subtitle(tr("about-subtitle", lang).into());
    ui.set_tr_accent_color(tr("accent-color", lang).into());
    ui.set_tr_active(tr("active", lang).into());
    ui.set_tr_add(tr("add", lang).into());
    ui.set_tr_add_favorite(tr("add-favorite", lang).into());
    ui.set_tr_add_folder(tr("add-folder", lang).into());
    ui.set_tr_add_notes(tr("add-notes", lang).into());
    ui.set_tr_add_print_count(tr("add-print-count", lang).into());
    ui.set_tr_add_printer(tr("add-printer", lang).into());
    ui.set_tr_add_subtag(tr("add-subtag", lang).into());
    ui.set_tr_add_subtag_title(tr("add-subtag-title", lang).into());
    ui.set_tr_add_tag(tr("add-tag", lang).into());
    ui.set_tr_add_tag_to_folder(tr("add-tag-to-folder", lang).into());
    ui.set_tr_add_tags_to_folder_title(tr("add-tags-to-folder-title", lang).into());
    ui.set_tr_added(tr("added", lang).into());
    ui.set_tr_after_export(tr("after-export", lang).into());
    ui.set_tr_all_models(tr("all-models", lang).into());
    ui.set_tr_already_added(tr("already-added", lang).into());
    ui.set_tr_anti_aliasing(tr("anti-aliasing", lang).into());
    ui.set_tr_apply(tr("apply", lang).into());
    ui.set_tr_author(tr("author", lang).into());
    ui.set_tr_author_heading(tr("author-heading", lang).into());
    ui.set_tr_author_or_source(tr("author-or-source", lang).into());
    ui.set_tr_auto(tr("auto", lang).into());
    ui.set_tr_browse(tr("browse", lang).into());
    ui.set_tr_build(tr("build", lang).into());
    ui.set_tr_bulk_edit_placeholder(tr("bulk-edit-placeholder", lang).into());
    ui.set_tr_cache_location(tr("cache-location", lang).into());
    ui.set_tr_cache_usage(tr("cache-usage", lang).into());
    ui.set_tr_cancel(tr("cancel", lang).into());
    ui.set_tr_card_label(tr("card-label", lang).into());
    ui.set_tr_card_label_hint(tr("card-label-hint", lang).into());
    ui.set_tr_check(tr("check", lang).into());
    ui.set_tr_choices_suffix(tr("choices-suffix", lang).into());
    ui.set_tr_choose_another_slicer(tr("choose-another-slicer", lang).into());
    ui.set_tr_clear_cache(tr("clear-cache", lang).into());
    ui.set_tr_clear_tags(tr("clear-tags", lang).into());
    ui.set_tr_copy_file_name(tr("copy-file-name", lang).into());
    ui.set_tr_copy_file_path(tr("copy-file-path", lang).into());
    ui.set_tr_copy_path(tr("copy-path", lang).into());
    ui.set_tr_copy_tag(tr("copy-tag", lang).into());
    ui.set_tr_create_new_tag(tr("create-new-tag", lang).into());
    ui.set_tr_custom_path(tr("custom-path", lang).into());
    ui.set_tr_dark(tr("dark", lang).into());
    ui.set_tr_date_format(tr("date-format", lang).into());
    ui.set_tr_default(tr("default", lang).into());
    ui.set_tr_default_estimate(tr("default-estimate", lang).into());
    ui.set_tr_default_slicer(tr("default-slicer", lang).into());
    ui.set_tr_default_sort(tr("default-sort", lang).into());
    ui.set_tr_default_view(tr("default-view", lang).into());
    ui.set_tr_delete_folder(tr("delete-folder", lang).into());
    ui.set_tr_delete_tag(tr("delete-tag", lang).into());
    ui.set_tr_density(tr("density", lang).into());
    ui.set_tr_detected_apps(tr("detected-apps", lang).into());
    ui.set_tr_dimensions(tr("dimensions", lang).into());
    ui.set_tr_duplicates(tr("duplicates", lang).into());
    ui.set_tr_empty_cta(tr("empty-cta", lang).into());
    ui.set_tr_empty_state(tr("empty-state", lang).into());
    ui.set_tr_enabled_profiles(tr("enabled-profiles", lang).into());
    ui.set_tr_english(tr("english", lang).into());
    ui.set_tr_enter_subtag_name(tr("enter-subtag-name", lang).into());
    ui.set_tr_enter_tag_name(tr("enter-tag-name", lang).into());
    ui.set_tr_even(tr("even", lang).into());
    ui.set_tr_exceeds_build_plate(tr("exceeds-build-plate", lang).into());
    ui.set_tr_favorites(tr("favorites", lang).into());
    ui.set_tr_filament(tr("filament", lang).into());
    ui.set_tr_file_heading(tr("file-heading", lang).into());
    ui.set_tr_file_size(tr("file-size", lang).into());
    ui.set_tr_file_types(tr("file-types", lang).into());
    ui.set_tr_filename(tr("filename", lang).into());
    ui.set_tr_filter_this_folder(tr("filter-this-folder", lang).into());
    ui.set_tr_filter_this_tag(tr("filter-this-tag", lang).into());
    ui.set_tr_filtering(tr("filtering", lang).into());
    ui.set_tr_fits_build_plate(tr("fits-build-plate", lang).into());
    ui.set_tr_focus_search(tr("focus-search", lang).into());
    ui.set_tr_folders_heading(tr("folders-heading", lang).into());
    ui.set_tr_format(tr("format", lang).into());
    ui.set_tr_fullscreen_orbit_hint(tr("fullscreen-orbit-hint", lang).into());
    ui.set_tr_geometry(tr("geometry", lang).into());
    ui.set_tr_global(tr("global", lang).into());
    ui.set_tr_gpu_rendering(tr("gpu-rendering", lang).into());
    ui.set_tr_grid(tr("grid", lang).into());
    ui.set_tr_grid_view(tr("grid-view", lang).into());
    ui.set_tr_history(tr("history", lang).into());
    ui.set_tr_import_folder(tr("import-folder", lang).into());
    ui.set_tr_isometric(tr("isometric", lang).into());
    ui.set_tr_language(tr("language", lang).into());
    ui.set_tr_large(tr("large", lang).into());
    ui.set_tr_last_library(tr("last-library", lang).into());
    ui.set_tr_layer_unknown(tr("layer-unknown", lang).into());
    ui.set_tr_layers(tr("layers", lang).into());
    ui.set_tr_layout_transitions(tr("layout-transitions", lang).into());
    ui.set_tr_library_folders(tr("library-folders", lang).into());
    ui.set_tr_library_heading(tr("library-heading", lang).into());
    ui.set_tr_light(tr("light", lang).into());
    ui.set_tr_lighting(tr("lighting", lang).into());
    ui.set_tr_list(tr("list", lang).into());
    ui.set_tr_list_view(tr("list-view", lang).into());
    ui.set_tr_load_more(tr("load-more", lang).into());
    ui.set_tr_log_level(tr("log-level", lang).into());
    ui.set_tr_maker(tr("maker", lang).into());
    ui.set_tr_maker_model_nozzle(tr("maker-model-nozzle", lang).into());
    ui.set_tr_manifold(tr("manifold", lang).into());
    ui.set_tr_masonry(tr("masonry", lang).into());
    ui.set_tr_masonry_view(tr("masonry-view", lang).into());
    ui.set_tr_material_settings_chips_heading(tr("material-settings-chips-heading", lang).into());
    ui.set_tr_material_settings_intro(tr("material-settings-intro", lang).into());
    ui.set_tr_material_unknown(tr("material-unknown", lang).into());
    ui.set_tr_medium(tr("medium", lang).into());
    ui.set_tr_mesh_health(tr("mesh-health", lang).into());
    ui.set_tr_metadata_storage(tr("metadata-storage", lang).into());
    ui.set_tr_metadata_storage_hint(tr("metadata-storage-hint", lang).into());
    ui.set_tr_model(tr("model", lang).into());
    ui.set_tr_modified(tr("modified", lang).into());
    ui.set_tr_multicolor_overhead(tr("multicolor-overhead", lang).into());
    ui.set_tr_my_printers(tr("my-printers", lang).into());
    ui.set_tr_name(tr("name", lang).into());
    ui.set_tr_new_tag_placeholder(tr("new-tag-placeholder", lang).into());
    ui.set_tr_no_notes(tr("no-notes", lang).into());
    ui.set_tr_no_print_records(tr("no-print-records", lang).into());
    ui.set_tr_no_printers(tr("no-printers", lang).into());
    ui.set_tr_no_tags(tr("no-tags", lang).into());
    ui.set_tr_normal_map(tr("normal-map", lang).into());
    ui.set_tr_normals(tr("normals", lang).into());
    ui.set_tr_notes_heading(tr("notes-heading", lang).into());
    ui.set_tr_nozzle(tr("nozzle", lang).into());
    ui.set_tr_nozzle_unknown(tr("nozzle-unknown", lang).into());
    ui.set_tr_off(tr("off", lang).into());
    ui.set_tr_on_startup(tr("on-startup", lang).into());
    ui.set_tr_open_in_slicer(tr("open-in-slicer", lang).into());
    ui.set_tr_open_logs(tr("open-logs", lang).into());
    ui.set_tr_open_settings(tr("open-settings", lang).into());
    ui.set_tr_open_with_menu(tr("open-with-menu", lang).into());
    ui.set_tr_open_with_prefix(tr("open-with-prefix", lang).into());
    ui.set_tr_open_with_suffix(tr("open-with-suffix", lang).into());
    ui.set_tr_orbit_hint(tr("orbit-hint", lang).into());
    ui.set_tr_parse_errors(tr("parse-errors", lang).into());
    ui.set_tr_parent_tag_prefix(tr("parent-tag-prefix", lang).into());
    ui.set_tr_pick_nozzle(tr("pick-nozzle", lang).into());
    ui.set_tr_print_estimate(tr("print-estimate", lang).into());
    ui.set_tr_printed(tr("printed", lang).into());
    ui.set_tr_printed_prefix(tr("printed-prefix", lang).into());
    ui.set_tr_printed_suffix(tr("printed-suffix", lang).into());
    ui.set_tr_printer_placeholder(tr("printer-placeholder", lang).into());
    ui.set_tr_printer_selector(tr("printer-selector", lang).into());
    ui.set_tr_printer_unknown(tr("printer-unknown", lang).into());
    ui.set_tr_prints_heading(tr("prints-heading", lang).into());
    ui.set_tr_profile_placeholder(tr("profile-placeholder", lang).into());
    ui.set_tr_profile_unknown(tr("profile-unknown", lang).into());
    ui.set_tr_promote_tag(tr("promote-tag", lang).into());
    ui.set_tr_ready_to_print(tr("ready-to-print", lang).into());
    ui.set_tr_recent(tr("recent", lang).into());
    ui.set_tr_records_suffix(tr("records-suffix", lang).into());
    ui.set_tr_regenerate_thumbnails(tr("regenerate-thumbnails", lang).into());
    ui.set_tr_remove(tr("remove", lang).into());
    ui.set_tr_remove_favorite(tr("remove-favorite", lang).into());
    ui.set_tr_remove_from_library(tr("remove-from-library", lang).into());
    ui.set_tr_remove_print_count(tr("remove-print-count", lang).into());
    ui.set_tr_rename_model(tr("rename-model", lang).into());
    ui.set_tr_render_density(tr("render-density", lang).into());
    ui.set_tr_rescan_folder(tr("rescan-folder", lang).into());
    ui.set_tr_rescan_on_change(tr("rescan-on-change", lang).into());
    ui.set_tr_reset_settings(tr("reset-settings", lang).into());
    ui.set_tr_result_notes(tr("result-notes", lang).into());
    ui.set_tr_reveal_in_finder(tr("reveal-in-finder", lang).into());
    ui.set_tr_rim(tr("rim", lang).into());
    ui.set_tr_save(tr("save", lang).into());
    ui.set_tr_search_placeholder(tr("search-placeholder", lang).into());
    ui.set_tr_select_model(tr("select-model", lang).into());
    ui.set_tr_select_model_hint(tr("select-model-hint", lang).into());
    ui.set_tr_set_default(tr("set-default", lang).into());
    ui.set_tr_settings_about(tr("settings-about", lang).into());
    ui.set_tr_settings_advanced(tr("settings-advanced", lang).into());
    ui.set_tr_settings_appearance(tr("settings-appearance", lang).into());
    ui.set_tr_settings_general(tr("settings-general", lang).into());
    ui.set_tr_settings_hotkeys(tr("settings-hotkeys", lang).into());
    ui.set_tr_settings_library(tr("settings-library", lang).into());
    ui.set_tr_settings_material(tr("settings-material", lang).into());
    ui.set_tr_settings_printers(tr("settings-printers", lang).into());
    ui.set_tr_settings_slicer(tr("settings-slicer", lang).into());
    ui.set_tr_settings_thumbnails(tr("settings-thumbnails", lang).into());
    ui.set_tr_show_details(tr("show-details", lang).into());
    ui.set_tr_show_file_extensions(tr("show-file-extensions", lang).into());
    ui.set_tr_small(tr("small", lang).into());
    ui.set_tr_sort_added(tr("sort-added", lang).into());
    ui.set_tr_sort_modified(tr("sort-modified", lang).into());
    ui.set_tr_stack(tr("stack", lang).into());
    ui.set_tr_startup_hint(tr("startup-hint", lang).into());
    ui.set_tr_studio(tr("studio", lang).into());
    ui.set_tr_tags_heading(tr("tags-heading", lang).into());
    ui.set_tr_theme(tr("theme", lang).into());
    ui.set_tr_thumbnail_cache_renderer(tr("thumbnail-cache-renderer", lang).into());
    ui.set_tr_thumbnail_style(tr("thumbnail-style", lang).into());
    ui.set_tr_time(tr("time", lang).into());
    ui.set_tr_title_cased(tr("title-cased", lang).into());
    ui.set_tr_triangles(tr("triangles", lang).into());
    ui.set_tr_updates(tr("updates", lang).into());
    ui.set_tr_us_date_example(tr("us-date-example", lang).into());
    ui.set_tr_use_embedded_3mf_preview(tr("use-embedded-3mf-preview", lang).into());
    ui.set_tr_use_embedded_3mf_preview_hint(tr("use-embedded-3mf-preview-hint", lang).into());
    ui.set_tr_use_gpu_thumbnails(tr("use-gpu-thumbnails", lang).into());
    ui.set_tr_version(tr("version", lang).into());
    ui.set_tr_view(tr("view", lang).into());
    ui.set_tr_volume_est(tr("volume-est", lang).into());
    ui.set_tr_watch_for_changes(tr("watch-for-changes", lang).into());
    ui.set_tr_watertight(tr("watertight", lang).into());
    ui.set_tr_wireframe(tr("wireframe", lang).into());
}
