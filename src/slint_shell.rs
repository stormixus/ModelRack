use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::scanner;
use crate::strings;
use crate::view_model::{
    smart_filter_from_key, AppPrefs, AppViewSnapshot, BrowserCard as BrowserCardVm, Density,
    DisplayQuery, LibraryFilter, ScanStatus, SortBy, ViewMode,
};

use slint::winit_030::winit::dpi::{PhysicalPosition, PhysicalSize};
use slint::winit_030::WinitWindowAccessor;

slint::include_modules!();

pub fn run() -> Result<(), slint::PlatformError> {
    crate::macos::install_app_menu();

    let ui = ModelRackWindow::new()?;
    crate::fonts::install_slint_fonts();
    let state = Rc::new(RefCell::new(ShellState::default()));
    let snapshot = state.borrow_mut().snapshot_idle();

    apply_snapshot(&ui, &snapshot);
    apply_detail(&ui, &state.borrow());
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
                state.selected_index = None;
                state.snapshot_done()
            };
            apply_snapshot(&ui, &snapshot);
            apply_detail(&ui, &search_state.borrow());
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
                state.snapshot_done()
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
                state.snapshot_done()
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
                state.selected_index = None;
                state.snapshot_done()
            };
            apply_snapshot(&ui, &snapshot);
            apply_detail(&ui, &sort_state.borrow());
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
                state.selected_index = None;
                state.snapshot_done()
            };
            apply_snapshot(&ui, &snapshot);
            apply_detail(&ui, &filter_state.borrow());
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
                state.snapshot_done()
            };
            apply_snapshot(&ui, &snapshot);
            apply_settings(&ui, &settings_state.borrow());
        }
    });

    let weak = ui.as_weak();
    let select_state = state.clone();
    ui.on_select_model(move |index| {
        if let Some(ui) = weak.upgrade() {
            let mut state = select_state.borrow_mut();
            state.selected_index = Some(index as usize);
            apply_detail(&ui, &state);
        }
    });

    let weak = ui.as_weak();
    let fav_state = state.clone();
    ui.on_toggle_favorite(move || {
        if let Some(ui) = weak.upgrade() {
            let mut state = fav_state.borrow_mut();
            if let Some(idx) = state.selected_index {
                if let Some(entry) = state.displayed.get(idx) {
                    let path = entry.path.clone();
                    if let Some(real) = state.entries.iter_mut().find(|e| e.path == path) {
                        let meta = real.meta.get_or_insert_with(Default::default);
                        meta.favorite = !meta.favorite;
                    }
                }
                let snapshot = state.snapshot_done();
                apply_snapshot(&ui, &snapshot);
                apply_detail(&ui, &state);
            }
        }
    });

    ui.on_open_in_slicer(move || {
        // placeholder — will launch slicer in future
    });

    ui.on_window_close(move || {
        crate::macos::hide_window();
    });

    ui.on_window_minimize(move || {
        crate::macos::minimize_window();
    });

    let weak = ui.as_weak();
    let zoom_restore = Rc::new(RefCell::new(None::<(PhysicalPosition<i32>, PhysicalSize<u32>)>));
    ui.on_window_zoom(move || {
        let mut handled = false;
        if let Some(ui) = weak.upgrade() {
            let restore = zoom_restore.clone();
            handled = ui
                .window()
                .with_winit_window(|window| {
                    if let Some((position, size)) = restore.borrow_mut().take() {
                        window.set_outer_position(position);
                        let _ = window.request_inner_size(size);
                        window.request_redraw();
                        return;
                    }

                    if let Some(monitor) = window.current_monitor() {
                        let position = window.outer_position().unwrap_or(PhysicalPosition::new(0, 0));
                        let size = window.outer_size();
                        *restore.borrow_mut() = Some((position, size));
                        window.set_outer_position(monitor.position());
                        let _ = window.request_inner_size(monitor.size());
                        window.request_redraw();
                    } else {
                        crate::macos::zoom_window();
                    }
                })
                .is_some();
        }
        if !handled {
            crate::macos::zoom_window();
        }
    });

    let weak = ui.as_weak();
    ui.on_titlebar_drag(move |_x, _y| {
        if let Some(ui) = weak.upgrade() {
            let _ = ui.window().with_winit_window(|window| window.drag_window());
        }
    });

    ui.show()?;

    // Delay NSWindow config so Slint has finished setting up the window
    slint::Timer::single_shot(std::time::Duration::from_millis(50), move || {
        crate::macos::configure_transparent_titlebar();
        crate::macos::show_windows();
    });

    slint::run_event_loop()?;
    Ok(())
}

fn apply_detail(ui: &ModelRackWindow, state: &ShellState) {
    ui.set_selected_card_index(state.selected_index.map(|i| i as i32).unwrap_or(-1));
    if let Some(idx) = state.selected_index {
        if let Some(entry) = state.displayed.get(idx) {
            ui.set_has_selection(true);
            ui.set_detail_name(entry.filename.clone().into());
            ui.set_detail_path(
                entry
                    .path
                    .parent()
                    .map(|p| format!("{}/", p.display()))
                    .unwrap_or_default()
                    .into(),
            );
            ui.set_detail_format(
                match entry.stl_type {
                    scanner::StlType::Binary => "Binary STL",
                    scanner::StlType::Ascii => "ASCII STL",
                    scanner::StlType::ThreeMf => "3MF",
                    scanner::StlType::Obj => "OBJ",
                    scanner::StlType::Step => "STEP",
                    scanner::StlType::LargeStl => "Large STL",
                    scanner::StlType::Unknown => "Unknown",
                }
                .into(),
            );
            ui.set_detail_tris(
                entry
                    .triangle_count
                    .map(|t| {
                        if t >= 1_000_000 {
                            format!("{:.2}M", t as f64 / 1_000_000.0)
                        } else if t >= 1_000 {
                            format!("{:.1}K", t as f64 / 1_000.0)
                        } else {
                            format!("{}", t)
                        }
                    })
                    .unwrap_or_else(|| "—".to_string())
                    .into(),
            );
            ui.set_detail_dims(
                entry
                    .dimensions
                    .map(|[x, y, z]| format!("{:.1} × {:.1} × {:.1} mm", x, y, z))
                    .unwrap_or_else(|| "—".to_string())
                    .into(),
            );
            ui.set_detail_volume(
                entry
                    .dimensions
                    .map(|[x, y, z]| format!("{:.1} cm³", x * y * z / 1000.0 * 0.30))
                    .unwrap_or_else(|| "—".to_string())
                    .into(),
            );
            ui.set_detail_filesize(format_size(entry.size).into());
            ui.set_detail_hash(
                entry
                    .hash
                    .iter()
                    .take(8)
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>()
                    .into(),
            );
            ui.set_detail_modified(
                entry
                    .modified
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| {
                        let secs = d.as_secs();
                        let days = secs / 86400;
                        if days < 1 {
                            "today".to_string()
                        } else if days < 30 {
                            format!("{}d ago", days)
                        } else {
                            format!("{}mo ago", days / 30)
                        }
                    })
                    .unwrap_or_else(|| "—".to_string())
                    .into(),
            );
            ui.set_detail_added("—".into());
            ui.set_detail_author(
                entry
                    .meta
                    .as_ref()
                    .and_then(|m| {
                        if m.author.is_empty() {
                            None
                        } else {
                            Some(m.author.clone())
                        }
                    })
                    .unwrap_or_else(|| "You".to_string())
                    .into(),
            );
            ui.set_detail_tags_label(
                entry
                    .meta
                    .as_ref()
                    .map(|m| {
                        if m.tags.is_empty() {
                            "No tags".to_string()
                        } else {
                            m.tags.join(" · ")
                        }
                    })
                    .unwrap_or_else(|| "No tags".to_string())
                    .into(),
            );
            ui.set_detail_notes(
                entry
                    .meta
                    .as_ref()
                    .and_then(|m| (!m.notes.is_empty()).then(|| m.notes.clone()))
                    .unwrap_or_else(|| "Add notes...".to_string())
                    .into(),
            );
            ui.set_detail_printed_count(entry.meta.as_ref().map_or(0, |m| m.printed as i32));
            ui.set_detail_fav(
                entry
                    .meta
                    .as_ref()
                    .map(|m| m.favorite)
                    .unwrap_or(false),
            );

            // Mesh health (deterministic from triangle count)
            let manifold = entry.stl_type != scanner::StlType::Unknown;
            let watertight = manifold && entry.triangle_count.unwrap_or(0) > 100;
            let normals = manifold;
            ui.set_detail_manifold(manifold);
            ui.set_detail_watertight(watertight);
            ui.set_detail_normals(normals);
            ui.set_detail_health_score(
                if manifold { 50 } else { 20 }
                    + if watertight { 25 } else { 0 }
                    + if normals { 15 } else { 0 },
            );

            // Print estimate
            if let Some([x, y, z]) = entry.dimensions {
                let bbox_cm3 = x * y * z / 1000.0;
                let part_vol = bbox_cm3 * 0.30;
                let grams = (part_vol * 0.45 + part_vol * 0.55 * 0.15) * 1.24;
                let minutes = (grams / 0.6).max(6.0) as u32;
                let hours = minutes / 60;
                let mins = minutes % 60;
                ui.set_detail_estimate_time(
                    if hours > 0 {
                        format!("{}h {}m", hours, mins)
                    } else {
                        format!("{}m", mins)
                    }
                    .into(),
                );
                ui.set_detail_estimate_grams(format!("{}g", grams as u32).into());
                ui.set_detail_estimate_layers(format!("{}", (z / 0.20) as u32).into());
                ui.set_detail_bed_fit(x <= 256.0 && y <= 256.0 && z <= 256.0);
            } else {
                ui.set_detail_estimate_time("".into());
                ui.set_detail_estimate_grams("".into());
                ui.set_detail_estimate_layers("".into());
                ui.set_detail_bed_fit(false);
            }
        } else {
            ui.set_has_selection(false);
        }
    } else {
        ui.set_has_selection(false);
    }
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1_024 {
        format!("{:.1} KB", bytes as f64 / 1_024.0)
    } else {
        format!("{} B", bytes)
    }
}

#[derive(Clone)]
struct ShellState {
    entries: Vec<scanner::StlFileInfo>,
    displayed: Vec<scanner::StlFileInfo>,
    current_folder: Option<PathBuf>,
    prefs: AppPrefs,
    search_query: String,
    filter: LibraryFilter,
    sort_by: SortBy,
    sort_ascending: bool,
    skipped: usize,
    settings_open: bool,
    settings_tab: String,
    selected_index: Option<usize>,
}

impl Default for ShellState {
    fn default() -> Self {
        let entries = demo_entries();
        Self {
            entries,
            displayed: Vec::new(),
            current_folder: Some(PathBuf::from("/Users/hwankishin/Library/3d")),
            prefs: AppPrefs::default(),
            search_query: String::new(),
            filter: LibraryFilter::All,
            sort_by: SortBy::Name,
            sort_ascending: true,
            skipped: 0,
            settings_open: false,
            settings_tab: "general".to_string(),
            selected_index: Some(0),
        }
    }
}

struct DemoModel {
    name: &'static str,
    folder: &'static str,
    size: u64,
    tris: Option<usize>,
    dims: Option<[f32; 3]>,
    stl_type: scanner::StlType,
    tags: &'static [&'static str],
    printed: u32,
    favorite: bool,
    author: &'static str,
}

fn demo_entries() -> Vec<scanner::StlFileInfo> {
    let root = PathBuf::from("/Users/hwankishin/Library/3d");
    demo_models()
        .into_iter()
        .enumerate()
        .map(|(index, model)| demo_entry(&root, index, model))
        .collect()
}

fn demo_entry(root: &Path, index: usize, model: DemoModel) -> scanner::StlFileInfo {
    let path = root.join(model.folder).join(model.name);
    scanner::StlFileInfo {
        path,
        filename: model.name.to_string(),
        size: model.size,
        hash: demo_hash(index),
        stl_type: model.stl_type,
        triangle_count: model.tris,
        dimensions: model.dims,
        modified: demo_modified(index),
        meta: Some(scanner::SidecarMeta {
            tags: model.tags.iter().map(|tag| (*tag).to_string()).collect(),
            printed: model.printed,
            favorite: model.favorite,
            author: model.author.to_string(),
            notes: if index == 0 {
                "Rackmount bracket validated for the current homelab layout.".to_string()
            } else {
                String::new()
            },
            ..scanner::SidecarMeta::default()
        }),
    }
}

fn demo_hash(index: usize) -> [u8; 32] {
    let mut hash = [(index as u8).wrapping_add(11); 32];
    if index == 2 {
        hash = [7; 32];
    }
    if index == 3 {
        hash = [7; 32];
    }
    hash
}

fn demo_modified(index: usize) -> Option<std::time::SystemTime> {
    let days = if index < 12 {
        index as u64
    } else {
        240 + index as u64
    };
    Some(std::time::SystemTime::now() - std::time::Duration::from_secs(days * 86_400))
}

fn demo_models() -> Vec<DemoModel> {
    use scanner::StlType::{Ascii, Binary, Unknown};

    vec![
        DemoModel { name: "raspberry_pi_5_poe_rackmount_v2_final.stl", folder: "homelab/rackmount", size: 2_840_000, tris: Some(48_230), dims: Some([120.4, 88.0, 25.5]), stl_type: Binary, tags: &["rackmount", "raspberry-pi", "poe", "homelab"], printed: 3, favorite: true, author: "makerworld" },
        DemoModel { name: "pi5_heatsink_clip.stl", folder: "homelab/rackmount", size: 142_000, tris: Some(1_820), dims: Some([42.0, 32.0, 12.0]), stl_type: Binary, tags: &["raspberry-pi", "cooling"], printed: 2, favorite: false, author: "printables" },
        DemoModel { name: "1U_blank_panel_19in.stl", folder: "homelab/rackmount", size: 380_400, tris: Some(240), dims: Some([482.6, 44.4, 2.0]), stl_type: Binary, tags: &["rackmount", "19inch"], printed: 1, favorite: false, author: "thingiverse" },
        DemoModel { name: "gmktec_nucbox_mount.stl", folder: "homelab/mini-pc", size: 1_120_000, tris: Some(18_920), dims: Some([128.0, 128.0, 18.0]), stl_type: Binary, tags: &["mini-pc", "gmktec", "mount"], printed: 1, favorite: false, author: "鈴木一郎" },
        DemoModel { name: "switch_8port_bracket.stl", folder: "homelab/network", size: 920_000, tris: Some(14_820), dims: Some([220.0, 70.0, 32.0]), stl_type: Binary, tags: &["network", "switch", "bracket", "queued"], printed: 0, favorite: false, author: "김지훈" },
        DemoModel { name: "ssd_2_5in_caddy_x4.stl", folder: "homelab/storage", size: 1_840_000, tris: Some(28_100), dims: Some([110.0, 105.0, 50.0]), stl_type: Binary, tags: &["storage", "ssd", "cage"], printed: 2, favorite: true, author: "github/cnc" },
        DemoModel { name: "spool_holder_universal.stl", folder: "printer/upgrades", size: 2_240_000, tris: Some(32_400), dims: Some([180.0, 95.0, 110.0]), stl_type: Binary, tags: &["printer", "spool", "functional"], printed: 5, favorite: true, author: "You" },
        DemoModel { name: "bambu_p1s_chamber_thermometer.stl", folder: "printer/upgrades", size: 480_000, tris: Some(6_200), dims: Some([60.0, 40.0, 18.0]), stl_type: Binary, tags: &["bambulab", "printer", "upgrade"], printed: 1, favorite: false, author: "makerworld" },
        DemoModel { name: "cable_chain_15x10.stl", folder: "printer/upgrades", size: 320_000, tris: Some(4_400), dims: Some([220.0, 15.0, 10.0]), stl_type: Binary, tags: &["cable", "functional"], printed: 4, favorite: false, author: "You" },
        DemoModel { name: "snapmaker_a350_drag_chain_link.stl", folder: "printer/upgrades", size: 220_000, tris: Some(1_840), dims: Some([38.0, 22.0, 10.0]), stl_type: Binary, tags: &["snapmaker", "cable"], printed: 8, favorite: false, author: "printables" },
        DemoModel { name: "라즈베리파이_5_케이스_v3.stl", folder: "한국어_프로젝트", size: 1_640_000, tris: Some(22_300), dims: Some([95.0, 65.0, 28.0]), stl_type: Binary, tags: &["raspberry-pi", "case"], printed: 2, favorite: true, author: "makerworld" },
        DemoModel { name: "책상정리_케이블_홀더.stl", folder: "한국어_프로젝트", size: 280_000, tris: Some(3_120), dims: Some([60.0, 40.0, 25.0]), stl_type: Binary, tags: &["desk", "cable"], printed: 6, favorite: false, author: "You" },
        DemoModel { name: "키캡_oem_r4_blank.stl", folder: "한국어_프로젝트/keycaps", size: 88_000, tris: Some(920), dims: Some([18.0, 18.0, 11.0]), stl_type: Binary, tags: &["keycap", "keyboard"], printed: 12, favorite: true, author: "You" },
        DemoModel { name: "low_poly_fox.stl", folder: "decorative", size: 4_200_000, tris: Some(78_400), dims: Some([85.0, 110.0, 60.0]), stl_type: Binary, tags: &["decorative", "lowpoly"], printed: 1, favorite: false, author: "thingiverse" },
        DemoModel { name: "voronoi_planter_120mm.stl", folder: "decorative", size: 6_800_000, tris: Some(124_000), dims: Some([120.0, 120.0, 95.0]), stl_type: Binary, tags: &["decorative", "planter", "voronoi", "ready-to-print"], printed: 0, favorite: true, author: "makerworld" },
        DemoModel { name: "geometric_vase_twisted.stl", folder: "decorative", size: 3_400_000, tris: Some(56_000), dims: Some([80.0, 80.0, 180.0]), stl_type: Binary, tags: &["decorative", "vase"], printed: 2, favorite: false, author: "You" },
        DemoModel { name: "articulated_dragon_v4.stl", folder: "decorative/articulated", size: 18_400_000, tris: Some(320_000), dims: Some([240.0, 80.0, 65.0]), stl_type: Binary, tags: &["decorative", "articulated", "ready-to-print"], printed: 0, favorite: false, author: "printables" },
        DemoModel { name: "benchy_3dbenchy.stl", folder: "test_prints", size: 1_540_000, tris: Some(22_500), dims: Some([60.0, 31.0, 48.0]), stl_type: Binary, tags: &["test", "benchmark", "favorite"], printed: 4, favorite: true, author: "You" },
        DemoModel { name: "calibration_cube_20mm.stl", folder: "test_prints", size: 12_400, tris: Some(12), dims: Some([20.0, 20.0, 20.0]), stl_type: Binary, tags: &["test", "calibration"], printed: 14, favorite: false, author: "You" },
        DemoModel { name: "all_in_one_test_v2.stl", folder: "test_prints", size: 880_000, tris: Some(14_200), dims: Some([60.0, 60.0, 30.0]), stl_type: Binary, tags: &["test", "calibration"], printed: 3, favorite: false, author: "You" },
        DemoModel { name: "broken_export_garbage.stl", folder: "downloads", size: 184_000, tris: None, dims: None, stl_type: Unknown, tags: &[], printed: 0, favorite: false, author: "unknown" },
        DemoModel { name: "weird_ascii_export.stl", folder: "downloads", size: 4_200_000, tris: Some(8_400), dims: Some([42.0, 42.0, 42.0]), stl_type: Ascii, tags: &["ready-to-print"], printed: 0, favorite: false, author: "You" },
        DemoModel { name: "hdd_3_5in_vibration_dampener.stl", folder: "homelab/storage", size: 240_000, tris: Some(2_800), dims: Some([102.0, 14.0, 26.0]), stl_type: Binary, tags: &["storage", "hdd", "damper"], printed: 4, favorite: false, author: "You" },
        DemoModel { name: "ups_battery_holder_18650_x8.stl", folder: "homelab/power", size: 1_280_000, tris: Some(18_400), dims: Some([180.0, 78.0, 22.0]), stl_type: Binary, tags: &["power", "battery", "18650"], printed: 1, favorite: false, author: "makerworld" },
        DemoModel { name: "fan_grill_120mm_honeycomb.stl", folder: "homelab/cooling", size: 480_000, tris: Some(12_200), dims: Some([120.0, 120.0, 4.0]), stl_type: Binary, tags: &["fan", "grill", "cooling"], printed: 6, favorite: true, author: "You" },
        DemoModel { name: "noctua_fan_shroud_140mm.stl", folder: "homelab/cooling", size: 620_000, tris: Some(9_800), dims: Some([140.0, 140.0, 30.0]), stl_type: Binary, tags: &["fan", "shroud", "cooling"], printed: 0, favorite: false, author: "printables" },
        DemoModel { name: "vesa_75_to_100_adapter.stl", folder: "mounts", size: 320_000, tris: Some(4_800), dims: Some([120.0, 120.0, 6.0]), stl_type: Binary, tags: &["vesa", "mount", "adapter"], printed: 0, favorite: false, author: "You" },
        DemoModel { name: "monitor_arm_cable_clip.stl", folder: "mounts", size: 88_000, tris: Some(1_200), dims: Some([42.0, 28.0, 18.0]), stl_type: Binary, tags: &["cable", "clip", "desk"], printed: 0, favorite: false, author: "You" },
        DemoModel { name: "wall_anchor_drywall_kit.stl", folder: "mounts", size: 64_000, tris: Some(600), dims: Some([25.0, 12.0, 12.0]), stl_type: Binary, tags: &["wall", "anchor"], printed: 16, favorite: false, author: "printables" },
        DemoModel { name: "stringing_test_tower.stl", folder: "test_prints", size: 180_000, tris: Some(1_800), dims: Some([60.0, 30.0, 50.0]), stl_type: Binary, tags: &["test", "calibration", "stringing"], printed: 2, favorite: false, author: "You" },
        DemoModel { name: "overhang_test_45_60_75.stl", folder: "test_prints", size: 220_000, tris: Some(2_400), dims: Some([80.0, 30.0, 40.0]), stl_type: Binary, tags: &["test", "calibration", "overhang"], printed: 1, favorite: false, author: "You" },
        DemoModel { name: "temp_tower_pla_180_220.stl", folder: "test_prints", size: 280_000, tris: Some(3_600), dims: Some([50.0, 30.0, 100.0]), stl_type: Binary, tags: &["test", "calibration", "temp"], printed: 2, favorite: false, author: "You" },
        DemoModel { name: "celtic_knot_coaster_set.stl", folder: "decorative", size: 920_000, tris: Some(14_800), dims: Some([95.0, 95.0, 6.0]), stl_type: Binary, tags: &["decorative", "coaster"], printed: 4, favorite: false, author: "makerworld" },
        DemoModel { name: "hex_organizer_drawer_module.stl", folder: "organization", size: 720_000, tris: Some(8_400), dims: Some([120.0, 120.0, 25.0]), stl_type: Binary, tags: &["organizer", "modular", "gridfinity"], printed: 9, favorite: false, author: "You" },
        DemoModel { name: "gridfinity_baseplate_4x4.stl", folder: "organization/gridfinity", size: 1_800_000, tris: Some(24_000), dims: Some([168.0, 168.0, 5.0]), stl_type: Binary, tags: &["organizer", "gridfinity", "modular"], printed: 12, favorite: true, author: "printables" },
        DemoModel { name: "gridfinity_bin_2x2x4_solid.stl", folder: "organization/gridfinity", size: 480_000, tris: Some(8_200), dims: Some([84.0, 84.0, 32.0]), stl_type: Binary, tags: &["organizer", "gridfinity"], printed: 24, favorite: false, author: "You" },
    ]
}

impl ShellState {
    fn snapshot_done(&mut self) -> AppViewSnapshot {
        let status = ScanStatus::Done {
            found: self.entries.len(),
            skipped: self.skipped,
        };
        let query = DisplayQuery {
            search_query: &self.search_query,
            library_filter: &self.filter,
            sort_by: self.sort_by,
            sort_ascending: self.sort_ascending,
            preserve_order: self.is_reference_demo_state(),
        };
        self.displayed = crate::view_model::filtered_sorted_entries(&self.entries, query);
        let query = DisplayQuery {
            search_query: &self.search_query,
            library_filter: &self.filter,
            sort_by: self.sort_by,
            sort_ascending: self.sort_ascending,
            preserve_order: self.is_reference_demo_state(),
        };
        AppViewSnapshot::from_parts(
            &self.entries,
            self.current_folder.as_deref(),
            &status,
            &self.prefs,
            query,
        )
    }

    fn snapshot_idle(&mut self) -> AppViewSnapshot {
        let query = DisplayQuery {
            search_query: &self.search_query,
            library_filter: &self.filter,
            sort_by: self.sort_by,
            sort_ascending: self.sort_ascending,
            preserve_order: self.is_reference_demo_state(),
        };
        self.displayed = crate::view_model::filtered_sorted_entries(&self.entries, query);
        AppViewSnapshot::from_parts(
            &self.entries,
            self.current_folder.as_deref(),
            &ScanStatus::Idle,
            &self.prefs,
            DisplayQuery {
                search_query: &self.search_query,
                library_filter: &self.filter,
                sort_by: self.sort_by,
                sort_ascending: self.sort_ascending,
                preserve_order: self.is_reference_demo_state(),
            },
        )
    }

    fn is_reference_demo_state(&self) -> bool {
        self.current_folder
            .as_ref()
            .is_some_and(|folder| folder == Path::new("/Users/hwankishin/Library/3d"))
            && self
                .entries
                .first()
                .is_some_and(|entry| entry.filename == "raspberry_pi_5_poe_rackmount_v2_final.stl")
    }

    fn scan_folder(&mut self, folder: &Path) -> AppViewSnapshot {
        let (entries, skipped) = scan_folder_entries(folder);
        self.entries = entries;
        self.current_folder = Some(folder.to_path_buf());
        self.skipped = skipped;
        self.selected_index = if self.entries.is_empty() { None } else { Some(0) };
        self.snapshot_done()
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
    ui.set_app_version(format!("v{}", env!("CARGO_PKG_VERSION")).into());
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
        author: card.author.clone().into(),
        relative_modified: card.relative_modified.clone().into(),
        thumb_key: card.thumb_key.clone().into(),
        badge: card.badge.clone().into(),
        printed_count: card.printed_count as i32,
        favorite: card.favorite,
        printed: card.printed,
        error: card.error,
    }
}
