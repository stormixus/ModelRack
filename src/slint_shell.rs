use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::scanner;
use crate::strings;
use crate::view_model::{
    smart_filter_from_key, AppPrefs, AppViewSnapshot, BrowserCard as BrowserCardVm, Density,
    DisplayQuery, LibraryFilter, ScanStatus, SortBy, ViewMode,
};

slint::slint! {
    import { Button, LineEdit } from "std-widgets.slint";

    export struct BrowserCard {
        title: string,
        subtitle: string,
        badge: string,
        favorite: bool,
        printed: bool,
        error: bool,
    }

    export struct SidebarItem {
        key: string,
        label: string,
        count: int,
        depth: int,
    }

    component SidebarLine inherits Rectangle {
        in property <string> label;
        in property <int> count;
        in property <int> depth: 0;
        in property <bool> selected: false;
        callback activated();

        height: 28px;
        background: selected ? #2a464c : #00000000;
        border-radius: 6px;

        Text {
            x: 10px + depth * 12px;
            y: 6px;
            text: label;
            color: selected ? #ecf2f4 : #cdd0d6;
            font-size: 13px;
        }

        Text {
            x: parent.width - self.width - 10px;
            y: 7px;
            text: count;
            color: #8a8e96;
            font-family: "JetBrains Mono";
            font-size: 12px;
        }

        TouchArea {
            width: parent.width;
            height: parent.height;
            clicked => { root.activated(); }
        }
    }

    component SettingsTabButton inherits Rectangle {
        in property <string> label;
        in property <string> key;
        in property <bool> selected: false;
        callback activated(string);

        height: 30px;
        background: selected ? #2a464c : #00000000;
        border-radius: 6px;

        Text {
            x: 10px;
            y: 7px;
            text: label;
            color: selected ? #f1f6f8 : #b9bec7;
            font-size: 13px;
        }

        TouchArea {
            width: parent.width;
            height: parent.height;
            clicked => { root.activated(key); }
        }
    }

    component SettingsRow inherits Rectangle {
        in property <string> label;
        in property <string> value;

        height: 34px;
        background: #00000000;

        Text {
            x: 0;
            y: 8px;
            text: label;
            color: #e0e5eb;
            font-size: 13px;
            font-weight: 600;
        }

        Text {
            x: 172px;
            y: 9px;
            width: parent.width - 172px;
            text: value;
            horizontal-alignment: right;
            color: #9aa1ac;
            font-size: 12px;
            overflow: elide;
        }
    }

    export component ModelRackWindow inherits Window {
        in property <string> app-title;
        in property <string> library-label;
        in property <string> status-text;
        in property <string> density-label;
        in property <string> view-mode-label;
        in-out property <string> search-text;
        in property <string> browser-message;
        in property <string> browser-count-label;
        in property <string> sort-label;
        in property <int> all-count;
        in property <int> recent-count;
        in property <int> favorites-count;
        in property <int> printed-count;
        in property <int> duplicates-count;
        in property <int> ready-count;
        in property <int> errors-count;
        in property <[BrowserCard]> model-cards;
        in property <[SidebarItem]> folder-items;
        in property <[SidebarItem]> tag-items;
        in property <string> active-filter-key;
        in property <bool> settings-open;
        in property <string> settings-tab;
        in property <string> settings-language-label;
        in property <string> settings-theme-label;
        in property <string> settings-folder-label;
        in property <string> settings-density-label;
        in property <string> settings-gpu-label;
        in property <string> settings-workers-label;
        in property <string> settings-slicer-label;

        callback open-folder();
        callback open-settings();
        callback close-settings();
        callback choose-settings-tab(string);
        callback cycle-settings-language();
        callback toggle-settings-theme();
        callback cycle-settings-density();
        callback toggle-settings-gpu();
        callback cycle-settings-workers();
        callback apply-search(string);
        callback cycle-view-mode();
        callback cycle-density();
        callback toggle-sort();
        callback choose-filter(string);

        title: app-title;
        width: 1024px;
        height: 720px;
        default-font-family: "Inter";
        background: #1f2024;

        Rectangle {
            width: parent.width;
            height: parent.height;
            background: #1f2024;

            Rectangle {
                x: 0;
                y: 0;
                width: parent.width;
                height: 36px;
                background: #191a1d;

                HorizontalLayout {
                    x: 18px;
                    y: 0;
                    width: parent.width - 36px;
                    height: 36px;
                    spacing: 10px;
                    alignment: center;

                    Text {
                        text: "●  ●  ●";
                        color: #e1e3e8;
                        font-size: 14px;
                    }

                    Text {
                        text: app-title;
                        color: #cdd0d6;
                        font-size: 15px;
                        font-weight: 600;
                    }

                    Text {
                        text: "—";
                        color: #60646c;
                        font-size: 13px;
                    }

                    Text {
                        text: library-label;
                        color: #888c94;
                        font-size: 12px;
                    }

                    Rectangle { horizontal-stretch: 1; }

                    Button {
                        text: "Settings";
                        clicked => { open-settings(); }
                    }
                }
            }

            Rectangle {
                x: 0;
                y: 36px;
                width: 220px;
                height: parent.height - 60px;
                background: #18191d;

                VerticalLayout {
                    x: 18px;
                    y: 22px;
                    width: parent.width - 36px;
                    spacing: 14px;

                    Text {
                        text: "LIBRARY";
                        color: #8c919b;
                        font-size: 13px;
                    }

                    SidebarLine { label: "All Models"; count: all-count; selected: active-filter-key == "all"; activated => { choose-filter("all"); } }
                    SidebarLine { label: "Recent"; count: recent-count; selected: active-filter-key == "recent"; activated => { choose-filter("recent"); } }
                    SidebarLine { label: "Favorites"; count: favorites-count; selected: active-filter-key == "favorites"; activated => { choose-filter("favorites"); } }
                    SidebarLine { label: "Printed"; count: printed-count; selected: active-filter-key == "printed"; activated => { choose-filter("printed"); } }
                    SidebarLine { label: "Duplicates"; count: duplicates-count; selected: active-filter-key == "duplicates"; activated => { choose-filter("duplicates"); } }
                    SidebarLine { label: "Ready"; count: ready-count; selected: active-filter-key == "ready"; activated => { choose-filter("ready"); } }
                    SidebarLine { label: "Unparseable"; count: errors-count; selected: active-filter-key == "errors"; activated => { choose-filter("errors"); } }

                    if folder-items.length > 0: Text {
                        text: "FOLDERS";
                        color: #8c919b;
                        font-size: 13px;
                    }

                    for folder in folder-items: SidebarLine {
                        label: folder.label;
                        count: folder.count;
                        depth: folder.depth;
                        selected: active-filter-key == folder.key;
                        activated => { choose-filter(folder.key); }
                    }

                    if tag-items.length > 0: Text {
                        text: "TAGS";
                        color: #8c919b;
                        font-size: 13px;
                    }

                    for tag in tag-items: SidebarLine {
                        label: tag.label;
                        count: tag.count;
                        selected: active-filter-key == tag.key;
                        activated => { choose-filter(tag.key); }
                    }
                }
            }

            Rectangle {
                x: 220px;
                y: 36px;
                width: parent.width - 540px;
                height: parent.height - 60px;
                background: #1f2024;

                HorizontalLayout {
                    x: 12px;
                    y: 12px;
                    width: parent.width - 24px;
                    height: 34px;
                    spacing: 8px;
                    alignment: center;

                    search_field := LineEdit {
                        placeholder-text: "Search models, tags, notes";
                        text <=> search-text;
                        width: 240px;
                    }

                    Button {
                        text: "Search";
                        clicked => { apply-search(search_field.text); }
                    }

                    Text {
                        text: view-mode-label;
                        color: #aeb4be;
                        font-size: 12px;
                    }

                    Text {
                        text: density-label;
                        color: #aeb4be;
                        font-size: 12px;
                    }

                    Text {
                        text: sort-label;
                        color: #aeb4be;
                        font-size: 12px;
                    }

                    Button {
                        text: "View";
                        clicked => { cycle-view-mode(); }
                    }

                    Button {
                        text: "Density";
                        clicked => { cycle-density(); }
                    }

                    Button {
                        text: "Sort";
                        clicked => { toggle-sort(); }
                    }

                    Rectangle { horizontal-stretch: 1; }

                    Button {
                        text: "Open Folder";
                        clicked => { open-folder(); }
                    }
                }

                Rectangle {
                    x: 12px;
                    y: 62px;
                    width: parent.width - 24px;
                    height: parent.height - 86px;
                    background: #24262b;
                    border-color: #3a3d44;
                    border-width: 1px;

                    if model-cards.length == 0: VerticalLayout {
                        alignment: center;
                        spacing: 10px;

                        Text {
                            text: browser-message;
                            color: #dce2ea;
                            font-size: 20px;
                            font-weight: 600;
                        }

                        Text {
                            text: browser-count-label;
                            color: #9ca3af;
                            font-size: 13px;
                        }
                    }

                    if model-cards.length > 0: GridLayout {
                        x: 12px;
                        y: 12px;
                        width: parent.width - 24px;
                        spacing: 10px;

                        for card in model-cards: Rectangle {
                            width: 156px;
                            height: 140px;
                            background: #25282f;
                            border-color: card.error ? #893230 : #3d4149;
                            border-width: 1px;
                            border-radius: 6px;

                            Rectangle {
                                x: 8px;
                                y: 8px;
                                width: parent.width - 16px;
                                height: 78px;
                                background: #15171b;
                                border-radius: 4px;

                                Text {
                                    x: 8px;
                                    y: 7px;
                                    text: card.badge;
                                    color: card.error ? #ffb4b0 : #c6dfe5;
                                    font-family: "JetBrains Mono";
                                    font-size: 11px;
                                    font-weight: 700;
                                }

                                Text {
                                    x: parent.width - self.width - 8px;
                                    y: 7px;
                                    text: (card.favorite ? "★ " : "") + (card.printed ? "⎙" : "");
                                    color: #86d4df;
                                    font-size: 11px;
                                }
                            }

                            Text {
                                x: 8px;
                                y: 94px;
                                width: parent.width - 16px;
                                text: card.title;
                                color: #dde1e8;
                                font-size: 12px;
                                overflow: elide;
                            }

                            Text {
                                x: 8px;
                                y: 114px;
                                width: parent.width - 16px;
                                text: card.subtitle;
                                color: #8f949d;
                                font-family: "JetBrains Mono";
                                font-size: 11px;
                                overflow: elide;
                            }
                        }
                    }
                }
            }

            Rectangle {
                x: parent.width - 320px;
                y: 36px;
                width: 320px;
                height: parent.height - 60px;
                background: #191a1f;

                VerticalLayout {
                    x: 18px;
                    y: 22px;
                    width: parent.width - 36px;
                    spacing: 14px;

                    Text {
                        text: "DETAILS";
                        color: #8c919b;
                        font-size: 13px;
                    }

                    Text {
                        text: "No model selected";
                        color: #d9dde4;
                        font-size: 14px;
                    }
                }
            }

            Rectangle {
                x: 0;
                y: parent.height - 24px;
                width: parent.width;
                height: 24px;
                background: #17181b;

                Text {
                    x: 12px;
                    y: 4px;
                    text: status-text;
                    color: #9aa0aa;
                    font-family: "JetBrains Mono";
                    font-size: 11px;
                }
            }

            if settings-open: Rectangle {
                x: 0;
                y: 0;
                width: parent.width;
                height: parent.height;
                background: #00000088;

                Rectangle {
                    x: (parent.width - 760px) / 2;
                    y: (parent.height - 540px) / 2;
                    width: 760px;
                    height: 540px;
                    background: #202226;
                    border-color: #3b4048;
                    border-width: 1px;
                    border-radius: 8px;

                    Rectangle {
                        x: 0;
                        y: 0;
                        width: 188px;
                        height: parent.height;
                        background: #191b20;
                        border-radius: 8px;

                        VerticalLayout {
                            x: 16px;
                            y: 18px;
                            width: parent.width - 32px;
                            spacing: 8px;

                            Text {
                                text: "ModelRack";
                                color: #f0f4f7;
                                font-size: 15px;
                                font-weight: 700;
                            }

                            Text {
                                text: "v0.0.3";
                                color: #8f96a3;
                                font-family: "JetBrains Mono";
                                font-size: 11px;
                            }

                            Rectangle { height: 8px; }

                            SettingsTabButton { label: "General"; key: "general"; selected: settings-tab == "general"; activated(tab) => { choose-settings-tab(tab); } }
                            SettingsTabButton { label: "Appearance"; key: "appearance"; selected: settings-tab == "appearance"; activated(tab) => { choose-settings-tab(tab); } }
                            SettingsTabButton { label: "Library"; key: "library"; selected: settings-tab == "library"; activated(tab) => { choose-settings-tab(tab); } }
                            SettingsTabButton { label: "Thumbnails"; key: "thumbnails"; selected: settings-tab == "thumbnails"; activated(tab) => { choose-settings-tab(tab); } }
                            SettingsTabButton { label: "Slicer"; key: "slicer"; selected: settings-tab == "slicer"; activated(tab) => { choose-settings-tab(tab); } }
                            SettingsTabButton { label: "Advanced"; key: "advanced"; selected: settings-tab == "advanced"; activated(tab) => { choose-settings-tab(tab); } }
                            SettingsTabButton { label: "About"; key: "about"; selected: settings-tab == "about"; activated(tab) => { choose-settings-tab(tab); } }
                        }
                    }

                    Rectangle {
                        x: 188px;
                        y: 0;
                        width: parent.width - 188px;
                        height: parent.height;
                        background: #202226;

                        HorizontalLayout {
                            x: 24px;
                            y: 18px;
                            width: parent.width - 48px;
                            height: 32px;
                            alignment: center;

                            Text {
                                text: settings-tab == "general" ? "General" :
                                      settings-tab == "appearance" ? "Appearance" :
                                      settings-tab == "library" ? "Library" :
                                      settings-tab == "thumbnails" ? "Thumbnails" :
                                      settings-tab == "slicer" ? "Slicer" :
                                      settings-tab == "advanced" ? "Advanced" : "About";
                                color: #f0f4f7;
                                font-size: 20px;
                                font-weight: 700;
                            }

                            Rectangle { horizontal-stretch: 1; }

                            Button {
                                text: "Close";
                                clicked => { close-settings(); }
                            }
                        }

                        Rectangle {
                            x: 24px;
                            y: 62px;
                            width: parent.width - 48px;
                            height: 1px;
                            background: #3b4048;
                        }

                        VerticalLayout {
                            x: 24px;
                            y: 82px;
                            width: parent.width - 48px;
                            spacing: 10px;

                            if settings-tab == "general": SettingsRow { label: "Language"; value: settings-language-label; }
                            if settings-tab == "general": SettingsRow { label: "Startup"; value: "Reopen last library folder"; }
                            if settings-tab == "general": SettingsRow { label: "Shortcuts"; value: "Cmd-F search, Cmd-, settings"; }
                            if settings-tab == "general": Button { text: "Cycle Language"; clicked => { cycle-settings-language(); } }

                            if settings-tab == "appearance": SettingsRow { label: "Theme"; value: settings-theme-label; }
                            if settings-tab == "appearance": SettingsRow { label: "Typography"; value: "Inter + Pretendard + JetBrains Mono"; }
                            if settings-tab == "appearance": SettingsRow { label: "Accent"; value: "Teal"; }
                            if settings-tab == "appearance": Button { text: "Toggle Theme"; clicked => { toggle-settings-theme(); } }

                            if settings-tab == "library": SettingsRow { label: "Current folder"; value: settings-folder-label; }
                            if settings-tab == "library": SettingsRow { label: "Recursive scan"; value: "Enabled"; }
                            if settings-tab == "library": SettingsRow { label: "Metadata"; value: ".modelrack.json sidecars"; }

                            if settings-tab == "thumbnails": SettingsRow { label: "Density"; value: settings-density-label; }
                            if settings-tab == "thumbnails": SettingsRow { label: "Style"; value: "wgpu thumbnail bridge pending"; }
                            if settings-tab == "thumbnails": SettingsRow { label: "Failures"; value: "ERR badge"; }
                            if settings-tab == "thumbnails": Button { text: "Cycle Density"; clicked => { cycle-settings-density(); } }

                            if settings-tab == "slicer": SettingsRow { label: "Default slicer"; value: settings-slicer-label; }
                            if settings-tab == "slicer": SettingsRow { label: "Open behavior"; value: "Chooser/dropdown pending"; }

                            if settings-tab == "advanced": SettingsRow { label: "GPU thumbnails"; value: settings-gpu-label; }
                            if settings-tab == "advanced": SettingsRow { label: "Workers"; value: settings-workers-label; }
                            if settings-tab == "advanced": SettingsRow { label: "Privacy"; value: "Local files stay local"; }
                            if settings-tab == "advanced": Button { text: "Toggle GPU"; clicked => { toggle-settings-gpu(); } }
                            if settings-tab == "advanced": Button { text: "Cycle Workers"; clicked => { cycle-settings-workers(); } }

                            if settings-tab == "about": SettingsRow { label: "Version"; value: "v0.0.3 alpha"; }
                            if settings-tab == "about": SettingsRow { label: "Stack"; value: "Rust + Slint bridge"; }
                            if settings-tab == "about": SettingsRow { label: "Renderer"; value: "wgpu for 3D surfaces only"; }
                            if settings-tab == "about": SettingsRow { label: "License"; value: "MIT"; }
                        }
                    }
                }
            }
        }
    }

}

pub fn run() -> Result<(), slint::PlatformError> {
    let ui = ModelRackWindow::new()?;
    crate::fonts::install_slint_fonts();
    let state = Rc::new(RefCell::new(ShellState::default()));
    let snapshot = state.borrow().snapshot(ScanStatus::Idle);

    apply_snapshot(&ui, &snapshot);
    apply_settings(&ui, &state.borrow());

    let weak = ui.as_weak();
    let open_state = state.clone();
    ui.on_open_folder(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_status_text("Choose a library folder".into());
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                ui.set_library_label(folder.display().to_string().into());
                ui.set_status_text("Scanning selected folder".into());
                let snapshot = {
                    let mut state = open_state.borrow_mut();
                    state.scan_folder(&folder)
                };
                apply_snapshot(&ui, &snapshot);
                apply_settings(&ui, &open_state.borrow());
            }
        }
    });

    let weak = ui.as_weak();
    let search_state = state.clone();
    ui.on_apply_search(move |query| {
        if let Some(ui) = weak.upgrade() {
            let snapshot = {
                let mut state = search_state.borrow_mut();
                state.search_query = query.to_string();
                state.snapshot(ScanStatus::Done {
                    found: state.entries.len(),
                    skipped: state.skipped,
                })
            };
            apply_snapshot(&ui, &snapshot);
            apply_settings(&ui, &search_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let view_state = state.clone();
    ui.on_cycle_view_mode(move || {
        if let Some(ui) = weak.upgrade() {
            let snapshot = {
                let mut state = view_state.borrow_mut();
                state.cycle_view_mode();
                state.snapshot(ScanStatus::Done {
                    found: state.entries.len(),
                    skipped: state.skipped,
                })
            };
            apply_snapshot(&ui, &snapshot);
            apply_settings(&ui, &view_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let density_state = state.clone();
    ui.on_cycle_density(move || {
        if let Some(ui) = weak.upgrade() {
            let snapshot = {
                let mut state = density_state.borrow_mut();
                state.cycle_density();
                state.snapshot(ScanStatus::Done {
                    found: state.entries.len(),
                    skipped: state.skipped,
                })
            };
            apply_snapshot(&ui, &snapshot);
            apply_settings(&ui, &density_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let sort_state = state.clone();
    ui.on_toggle_sort(move || {
        if let Some(ui) = weak.upgrade() {
            let snapshot = {
                let mut state = sort_state.borrow_mut();
                state.sort_ascending = !state.sort_ascending;
                state.snapshot(ScanStatus::Done {
                    found: state.entries.len(),
                    skipped: state.skipped,
                })
            };
            apply_snapshot(&ui, &snapshot);
            apply_settings(&ui, &sort_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let filter_state = state.clone();
    ui.on_choose_filter(move |key| {
        if let Some(ui) = weak.upgrade() {
            let snapshot = {
                let mut state = filter_state.borrow_mut();
                if let Some(filter) = smart_filter_from_key(key.as_str()) {
                    state.filter = filter;
                }
                state.snapshot(ScanStatus::Done {
                    found: state.entries.len(),
                    skipped: state.skipped,
                })
            };
            apply_snapshot(&ui, &snapshot);
            apply_settings(&ui, &filter_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_open_settings(move || {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.settings_open = true;
            }
            apply_settings(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_close_settings(move || {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.settings_open = false;
            }
            apply_settings(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_choose_settings_tab(move |tab| {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.settings_tab = tab.to_string();
            }
            apply_settings(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_cycle_settings_language(move || {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.cycle_language();
            }
            apply_settings(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_toggle_settings_theme(move || {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.toggle_theme();
            }
            apply_settings(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_cycle_settings_density(move || {
        if let Some(ui) = weak.upgrade() {
            let snapshot = {
                let mut state = settings_state.borrow_mut();
                state.cycle_density();
                state.snapshot(ScanStatus::Done {
                    found: state.entries.len(),
                    skipped: state.skipped,
                })
            };
            apply_snapshot(&ui, &snapshot);
            apply_settings(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_toggle_settings_gpu(move || {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.prefs.gpu_thumbnails_enabled = !state.prefs.gpu_thumbnails_enabled;
            }
            apply_settings(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let settings_state = state.clone();
    ui.on_cycle_settings_workers(move || {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = settings_state.borrow_mut();
                state.prefs.thumbnail_workers = if state.prefs.thumbnail_workers >= 8 {
                    1
                } else {
                    state.prefs.thumbnail_workers + 1
                };
            }
            apply_settings(&ui, &settings_state.borrow());
        }
    });

    ui.run()
}

#[derive(Clone)]
struct ShellState {
    entries: Vec<scanner::StlFileInfo>,
    current_folder: Option<PathBuf>,
    prefs: AppPrefs,
    search_query: String,
    filter: LibraryFilter,
    sort_by: SortBy,
    sort_ascending: bool,
    skipped: usize,
    settings_open: bool,
    settings_tab: String,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            current_folder: None,
            prefs: AppPrefs::default(),
            search_query: String::new(),
            filter: LibraryFilter::All,
            sort_by: SortBy::Name,
            sort_ascending: true,
            skipped: 0,
            settings_open: false,
            settings_tab: "general".to_string(),
        }
    }
}

impl ShellState {
    fn snapshot(&self, status: ScanStatus) -> AppViewSnapshot {
        AppViewSnapshot::from_parts(
            &self.entries,
            self.current_folder.as_deref(),
            &status,
            &self.prefs,
            DisplayQuery {
                search_query: &self.search_query,
                library_filter: &self.filter,
                sort_by: self.sort_by,
                sort_ascending: self.sort_ascending,
            },
        )
    }

    fn scan_folder(&mut self, folder: &Path) -> AppViewSnapshot {
        let (entries, skipped) = scan_folder_entries(folder);
        self.entries = entries;
        self.current_folder = Some(folder.to_path_buf());
        self.skipped = skipped;
        self.snapshot(ScanStatus::Done {
            found: self.entries.len(),
            skipped,
        })
    }

    fn cycle_view_mode(&mut self) {
        self.prefs.view_mode = match ViewMode::from_str(&self.prefs.view_mode) {
            ViewMode::Grid => "list",
            ViewMode::List => "masonry",
            ViewMode::Masonry => "grid",
        }
        .to_string();
    }

    fn cycle_density(&mut self) {
        self.prefs.density = match Density::from_str(&self.prefs.density) {
            Density::Small => "medium",
            Density::Medium => "large",
            Density::Large => "small",
        }
        .to_string();
    }

    fn cycle_language(&mut self) {
        self.prefs.language = match self.prefs.language.as_str() {
            "en" => "ko",
            "ko" => "ja",
            _ => "en",
        }
        .to_string();
    }

    fn toggle_theme(&mut self) {
        self.prefs.theme = if self.prefs.theme == "dark" {
            "light".to_string()
        } else {
            "dark".to_string()
        };
    }
}

fn apply_snapshot(ui: &ModelRackWindow, snapshot: &AppViewSnapshot) {
    ui.set_app_title(strings::APP_TITLE.into());
    ui.set_library_label(snapshot.library_label.clone().into());
    ui.set_status_text(snapshot.status_text.clone().into());
    ui.set_density_label(snapshot.density_label.clone().into());
    ui.set_view_mode_label(snapshot.view_mode_label.clone().into());
    ui.set_browser_message(snapshot.browser.empty_message.clone().into());
    ui.set_sort_label(snapshot.sort_label.clone().into());
    ui.set_browser_count_label(
        format!(
            "{} visible · {} total",
            snapshot.browser.displayed, snapshot.browser.total
        )
        .into(),
    );
    ui.set_all_count(snapshot.sidebar.all as i32);
    ui.set_recent_count(snapshot.sidebar.recent as i32);
    ui.set_favorites_count(snapshot.sidebar.favorites as i32);
    ui.set_printed_count(snapshot.sidebar.printed as i32);
    ui.set_duplicates_count(snapshot.sidebar.duplicates as i32);
    ui.set_ready_count(snapshot.sidebar.ready as i32);
    ui.set_errors_count(snapshot.sidebar.errors as i32);
    ui.set_active_filter_key(snapshot.active_filter_key.clone().into());
    let cards = snapshot
        .cards
        .iter()
        .map(browser_card)
        .collect::<Vec<BrowserCard>>();
    ui.set_model_cards(slint::ModelRc::new(slint::VecModel::from(cards)));
    let folders = snapshot
        .folders
        .iter()
        .map(|folder| SidebarItem {
            key: format!("folder:{}", folder.path.display()).into(),
            label: folder.label.clone().into(),
            count: folder.count as i32,
            depth: folder.depth as i32,
        })
        .collect::<Vec<SidebarItem>>();
    ui.set_folder_items(slint::ModelRc::new(slint::VecModel::from(folders)));
    let tags = snapshot
        .tags
        .iter()
        .map(|tag| SidebarItem {
            key: format!("tag:{}", tag.label).into(),
            label: tag.label.clone().into(),
            count: tag.count as i32,
            depth: 0,
        })
        .collect::<Vec<SidebarItem>>();
    ui.set_tag_items(slint::ModelRc::new(slint::VecModel::from(tags)));
}

fn apply_settings(ui: &ModelRackWindow, state: &ShellState) {
    ui.set_settings_open(state.settings_open);
    ui.set_settings_tab(state.settings_tab.clone().into());
    ui.set_settings_language_label(language_label(&state.prefs.language).into());
    ui.set_settings_theme_label(theme_label(&state.prefs.theme).into());
    ui.set_settings_folder_label(
        state
            .current_folder
            .as_ref()
            .map(|folder| folder.display().to_string())
            .unwrap_or_else(|| "No folder selected".to_string())
            .into(),
    );
    ui.set_settings_density_label(Density::from_str(&state.prefs.density).as_str().into());
    ui.set_settings_gpu_label(
        if state.prefs.gpu_thumbnails_enabled {
            "Enabled"
        } else {
            "Disabled"
        }
        .into(),
    );
    ui.set_settings_workers_label(format!("{} workers", state.prefs.thumbnail_workers).into());
    ui.set_settings_slicer_label(
        if state.prefs.slicer_path.trim().is_empty() {
            "System default STL opener".to_string()
        } else {
            state.prefs.slicer_path.clone()
        }
        .into(),
    );
}

fn language_label(language: &str) -> &'static str {
    match language {
        "ko" => "Korean",
        "ja" => "Japanese",
        _ => "English",
    }
}

fn theme_label(theme: &str) -> &'static str {
    if theme == "light" {
        "Light"
    } else {
        "Dark"
    }
}

fn scan_folder_entries(folder: &Path) -> (Vec<scanner::StlFileInfo>, usize) {
    let (tx, rx) = crossbeam_channel::unbounded();
    scanner::scan_folder_stream(folder, tx);

    let mut entries = Vec::new();
    let mut skipped = 0usize;
    for event in rx {
        match event {
            scanner::ScanEvent::Progress { .. } => {}
            scanner::ScanEvent::Entry { info, .. } => entries.push(*info),
            scanner::ScanEvent::Done {
                skipped: done_skipped,
            } => {
                skipped = done_skipped;
                break;
            }
        }
    }

    entries.sort_by(|a, b| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()));
    (entries, skipped)
}

fn browser_card(card: &BrowserCardVm) -> BrowserCard {
    BrowserCard {
        title: card.title.clone().into(),
        subtitle: card.subtitle.clone().into(),
        badge: card.badge.clone().into(),
        favorite: card.favorite,
        printed: card.printed,
        error: card.error,
    }
}
