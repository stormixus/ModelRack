// Sample STL library data — mixed homelab/maker/CJK
window.MODELS = [
  // Homelab cluster
  { id: 1, name: "raspberry_pi_5_poe_rackmount_v2_final.stl", folder: "homelab/rackmount", size: 2_840_000, tris: 48230, dims: [120.4, 88.0, 25.5], type: "Binary", tags: ["rackmount","raspberry-pi","poe","homelab"], printed: 3, fav: true, status: "printed", thumb: "rack" },
  { id: 2, name: "pi5_heatsink_clip.stl", folder: "homelab/rackmount", size: 142_000, tris: 1820, dims: [42.0, 32.0, 12.0], type: "Binary", tags: ["raspberry-pi","cooling"], printed: 2, fav: false, status: "printed", thumb: "clip" },
  { id: 3, name: "1U_blank_panel_19in.stl", folder: "homelab/rackmount", size: 380_400, tris: 240, dims: [482.6, 44.4, 2.0], type: "Binary", tags: ["rackmount","19inch"], printed: 1, fav: false, status: "printed", thumb: "panel" },
  { id: 4, name: "gmktec_nucbox_mount.stl", folder: "homelab/mini-pc", size: 1_120_000, tris: 18920, dims: [128.0, 128.0, 18.0], type: "Binary", tags: ["mini-pc","gmktec","mount"], printed: 1, fav: false, status: "printed", thumb: "mount" },
  { id: 5, name: "switch_8port_bracket.stl", folder: "homelab/network", size: 920_000, tris: 14820, dims: [220.0, 70.0, 32.0], type: "Binary", tags: ["network","switch","bracket"], printed: 0, fav: false, status: "queued", thumb: "bracket" },
  { id: 6, name: "ssd_2_5in_caddy_x4.stl", folder: "homelab/storage", size: 1_840_000, tris: 28100, dims: [110.0, 105.0, 50.0], type: "Binary", tags: ["storage","ssd","cage"], printed: 2, fav: true, status: "printed", thumb: "caddy" },

  // Functional / Maker
  { id: 7, name: "spool_holder_universal.stl", folder: "printer/upgrades", size: 2_240_000, tris: 32400, dims: [180.0, 95.0, 110.0], type: "Binary", tags: ["printer","spool","functional"], printed: 5, fav: true, status: "printed", thumb: "spool" },
  { id: 8, name: "bambu_p1s_chamber_thermometer.stl", folder: "printer/upgrades", size: 480_000, tris: 6200, dims: [60.0, 40.0, 18.0], type: "Binary", tags: ["bambulab","printer","upgrade"], printed: 1, fav: false, status: "printed", thumb: "therm" },
  { id: 9, name: "cable_chain_15x10.stl", folder: "printer/upgrades", size: 320_000, tris: 4400, dims: [220.0, 15.0, 10.0], type: "Binary", tags: ["cable","functional"], printed: 4, fav: false, status: "printed", thumb: "chain" },
  { id: 10, name: "snapmaker_a350_drag_chain_link.stl", folder: "printer/upgrades", size: 220_000, tris: 1840, dims: [38.0, 22.0, 10.0], type: "Binary", tags: ["snapmaker","cable"], printed: 8, fav: false, status: "printed", thumb: "link" },

  // Korean filenames
  { id: 11, name: "라즈베리파이_5_케이스_v3.stl", folder: "한국어_프로젝트", size: 1_640_000, tris: 22300, dims: [95.0, 65.0, 28.0], type: "Binary", tags: ["raspberry-pi","case"], printed: 2, fav: true, status: "printed", thumb: "case" },
  { id: 12, name: "책상정리_케이블_홀더.stl", folder: "한국어_프로젝트", size: 280_000, tris: 3120, dims: [60.0, 40.0, 25.0], type: "Binary", tags: ["desk","cable"], printed: 6, fav: false, status: "printed", thumb: "holder" },
  { id: 13, name: "키캡_oem_r4_blank.stl", folder: "한국어_프로젝트/keycaps", size: 88_000, tris: 920, dims: [18.0, 18.0, 11.0], type: "Binary", tags: ["keycap","keyboard"], printed: 12, fav: true, status: "printed", thumb: "keycap" },

  // Decorative
  { id: 14, name: "low_poly_fox.stl", folder: "decorative", size: 4_200_000, tris: 78400, dims: [85.0, 110.0, 60.0], type: "Binary", tags: ["decorative","lowpoly","animal"], printed: 1, fav: false, status: "printed", thumb: "fox" },
  { id: 15, name: "voronoi_planter_120mm.stl", folder: "decorative", size: 6_800_000, tris: 124000, dims: [120.0, 120.0, 95.0], type: "Binary", tags: ["decorative","planter","voronoi"], printed: 0, fav: true, status: "ready", thumb: "voro" },
  { id: 16, name: "geometric_vase_twisted.stl", folder: "decorative", size: 3_400_000, tris: 56000, dims: [80.0, 80.0, 180.0], type: "Binary", tags: ["decorative","vase"], printed: 2, fav: false, status: "printed", thumb: "vase" },
  { id: 17, name: "articulated_dragon_v4.stl", folder: "decorative/articulated", size: 18_400_000, tris: 320000, dims: [240.0, 80.0, 65.0], type: "Binary", tags: ["decorative","articulated","dragon"], printed: 0, fav: false, status: "ready", thumb: "drag" },

  // Misc
  { id: 18, name: "benchy_3dbenchy.stl", folder: "test_prints", size: 1_540_000, tris: 22500, dims: [60.0, 31.0, 48.0], type: "Binary", tags: ["test","benchmark","favorite"], printed: 4, fav: true, status: "printed", thumb: "benchy" },
  { id: 19, name: "calibration_cube_20mm.stl", folder: "test_prints", size: 12_400, tris: 12, dims: [20.0, 20.0, 20.0], type: "Binary", tags: ["test","calibration"], printed: 14, fav: false, status: "printed", thumb: "cube" },
  { id: 20, name: "all_in_one_test_v2.stl", folder: "test_prints", size: 880_000, tris: 14200, dims: [60.0, 60.0, 30.0], type: "Binary", tags: ["test","calibration"], printed: 3, fav: false, status: "printed", thumb: "test" },

  // Failures / corrupt
  { id: 21, name: "broken_export_garbage.stl", folder: "downloads", size: 184_000, tris: null, dims: null, type: "Unknown", tags: [], printed: 0, fav: false, status: "error", thumb: "err" },
  { id: 22, name: "weird_ascii_export.stl", folder: "downloads", size: 4_200_000, tris: 8400, dims: [42.0, 42.0, 42.0], type: "ASCII", tags: [], printed: 0, fav: false, status: "ready", thumb: "ascii" },

  // More homelab variations
  { id: 23, name: "hdd_3_5in_vibration_dampener.stl", folder: "homelab/storage", size: 240_000, tris: 2800, dims: [102.0, 14.0, 26.0], type: "Binary", tags: ["storage","hdd","damper"], printed: 4, fav: false, status: "printed", thumb: "damp" },
  { id: 24, name: "ups_battery_holder_18650_x8.stl", folder: "homelab/power", size: 1_280_000, tris: 18400, dims: [180.0, 78.0, 22.0], type: "Binary", tags: ["power","battery","18650"], printed: 1, fav: false, status: "printed", thumb: "batt" },
  { id: 25, name: "fan_grill_120mm_honeycomb.stl", folder: "homelab/cooling", size: 480_000, tris: 12200, dims: [120.0, 120.0, 4.0], type: "Binary", tags: ["fan","grill","cooling"], printed: 6, fav: true, status: "printed", thumb: "fan" },
  { id: 26, name: "noctua_fan_shroud_140mm.stl", folder: "homelab/cooling", size: 620_000, tris: 9800, dims: [140.0, 140.0, 30.0], type: "Binary", tags: ["fan","shroud","cooling"], printed: 2, fav: false, status: "printed", thumb: "shroud" },

  // Connector / mount
  { id: 27, name: "vesa_75_to_100_adapter.stl", folder: "mounts", size: 320_000, tris: 4800, dims: [120.0, 120.0, 6.0], type: "Binary", tags: ["vesa","mount","adapter"], printed: 2, fav: false, status: "printed", thumb: "vesa" },
  { id: 28, name: "monitor_arm_cable_clip.stl", folder: "mounts", size: 88_000, tris: 1200, dims: [42.0, 28.0, 18.0], type: "Binary", tags: ["cable","clip","desk"], printed: 8, fav: false, status: "printed", thumb: "mclip" },
  { id: 29, name: "wall_anchor_drywall_kit.stl", folder: "mounts", size: 64_000, tris: 600, dims: [25.0, 12.0, 12.0], type: "Binary", tags: ["wall","anchor"], printed: 16, fav: false, status: "printed", thumb: "anc" },

  // More test
  { id: 30, name: "stringing_test_tower.stl", folder: "test_prints", size: 180_000, tris: 1800, dims: [60.0, 30.0, 50.0], type: "Binary", tags: ["test","calibration","stringing"], printed: 2, fav: false, status: "printed", thumb: "string" },
  { id: 31, name: "overhang_test_45_60_75.stl", folder: "test_prints", size: 220_000, tris: 2400, dims: [80.0, 30.0, 40.0], type: "Binary", tags: ["test","calibration","overhang"], printed: 1, fav: false, status: "printed", thumb: "over" },
  { id: 32, name: "temp_tower_pla_180_220.stl", folder: "test_prints", size: 280_000, tris: 3600, dims: [50.0, 30.0, 100.0], type: "Binary", tags: ["test","calibration","temp"], printed: 2, fav: true, status: "printed", thumb: "temp" },

  // Decorative more
  { id: 33, name: "celtic_knot_coaster_set.stl", folder: "decorative", size: 920_000, tris: 14800, dims: [95.0, 95.0, 6.0], type: "Binary", tags: ["decorative","coaster"], printed: 4, fav: false, status: "printed", thumb: "celt" },
  { id: 34, name: "hex_organizer_drawer_module.stl", folder: "organization", size: 720_000, tris: 8400, dims: [120.0, 120.0, 25.0], type: "Binary", tags: ["organizer","modular","gridfinity"], printed: 9, fav: true, status: "printed", thumb: "hex" },
  { id: 35, name: "gridfinity_baseplate_4x4.stl", folder: "organization/gridfinity", size: 1_800_000, tris: 24000, dims: [168.0, 168.0, 5.0], type: "Binary", tags: ["organizer","gridfinity","modular"], printed: 12, fav: true, status: "printed", thumb: "grid" },
  { id: 36, name: "gridfinity_bin_2x2x4_solid.stl", folder: "organization/gridfinity", size: 480_000, tris: 8200, dims: [84.0, 84.0, 32.0], type: "Binary", tags: ["organizer","gridfinity"], printed: 24, fav: false, status: "printed", thumb: "bin" },
];

window.TAGS = [
  { name: "rackmount", count: 3 },
  { name: "raspberry-pi", count: 4 },
  { name: "homelab", count: 1 },
  { name: "printer", count: 4 },
  { name: "functional", count: 1 },
  { name: "decorative", count: 6 },
  { name: "test", count: 6 },
  { name: "calibration", count: 5 },
  { name: "gridfinity", count: 3 },
  { name: "cable", count: 4 },
  { name: "mount", count: 3 },
  { name: "fan", count: 2 },
  { name: "cooling", count: 3 },
  { name: "favorite", count: 1 },
];

window.FOLDERS = [
  { path: "homelab", count: 12, expanded: true, children: [
    { path: "homelab/rackmount", count: 3 },
    { path: "homelab/mini-pc", count: 1 },
    { path: "homelab/network", count: 1 },
    { path: "homelab/storage", count: 2 },
    { path: "homelab/power", count: 1 },
    { path: "homelab/cooling", count: 2 },
  ]},
  { path: "printer/upgrades", count: 4 },
  { path: "decorative", count: 5, children: [
    { path: "decorative/articulated", count: 1 },
  ]},
  { path: "test_prints", count: 6 },
  { path: "organization", count: 3, children: [
    { path: "organization/gridfinity", count: 2 },
  ]},
  { path: "mounts", count: 3 },
  { path: "한국어_프로젝트", count: 3, children: [
    { path: "한국어_프로젝트/keycaps", count: 1 },
  ]},
  { path: "downloads", count: 2 },
];
