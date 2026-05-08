// ModelRack — Hi-fi mockup of v0.1.0 (macOS, Linear/Arc dark)
// Loads: data.js (window.MODELS, TAGS, FOLDERS) + thumbnail.jsx (window.Thumbnail)

const { useState, useMemo, useEffect, useRef } = React;
const Thumb = window.Thumbnail;

// ─────────────────────────────────────────────────────────
// Tweak defaults
// ─────────────────────────────────────────────────────────
const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "theme": "dark",
  "lang": "en"
}/*EDITMODE-END*/;

// ─────────────────────────────────────────────────────────
// Settings (persisted to localStorage so the dialog survives reload)
// ─────────────────────────────────────────────────────────
const SETTINGS_DEFAULTS = {
  // General
  theme: "dark",            // dark | light | auto
  lang: "en",               // en | ko | ja
  startupFolder: "last",    // last | empty
  defaultView: "grid",      // grid | masonry | list
  defaultSort: "name-asc",  // name-asc | name-desc | modified | added | size | tris
  dateFormat: "auto",       // auto | iso | us
  confirmDelete: true,
  showHidden: false,
  showExtensions: true,

  // Appearance
  accent: "teal",           // teal | violet | orange | green
  density: "comfortable",   // comfortable | compact
  sidebarPos: "left",       // left | right
  cardLabel: "filename",    // filename | titled
  reduceMotion: false,
  thumbBg: "checker",       // solid | checker | dot

  // Library
  fileTypes: { stl: true, threeMF: true, step: false, obj: false },
  excludePatterns: ".DS_Store, Thumbs.db, *.tmp",
  sidecar: "json",          // json | db | none
  libRecursive: true,
  libWatch: true,
  libSizeCapMB: 200,

  // Thumbnails
  thumbStyle: "iso",        // iso | wire | normal
  thumbCacheGB: 2,
  thumbRenderSize: 512,
  thumbLighting: "studio",  // studio | even | rim
  thumbAA: "msaa4x",        // off | msaa2x | msaa4x | msaa8x

  // Slicer
  slicer: "orca",
  slicerPath: "",
  postExport: "open",       // open | reveal | none
  defaultProfile: "0.20mm Standard",

  // Advanced
  workers: 4,
  gpu: true,
  cacheLocation: "~/Library/Caches/modelrack",
  logLevel: "info",         // error | warn | info | debug | trace
  devMenu: false,
  telemetry: false,
};

function useSettings() {
  const [s, setS] = useState(() => {
    try {
      const stored = JSON.parse(localStorage.getItem("modelrack.settings") || "{}");
      return { ...SETTINGS_DEFAULTS, ...stored };
    } catch { return SETTINGS_DEFAULTS; }
  });
  const update = (patch) => {
    setS(prev => {
      const next = { ...prev, ...patch };
      try { localStorage.setItem("modelrack.settings", JSON.stringify(next)); } catch {}
      return next;
    });
  };
  return [s, update];
}

// ─────────────────────────────────────────────────────────
// Inline icons (single-stroke, lucide-ish, sized via wrapper)
// ─────────────────────────────────────────────────────────
const Icon = ({ name, size = 14, stroke = 1.5 }) => {
  const paths = {
    folder: <path d="M2 5.5A1.5 1.5 0 0 1 3.5 4h3l1.5 1.5h4.5A1.5 1.5 0 0 1 14 7v4.5A1.5 1.5 0 0 1 12.5 13h-9A1.5 1.5 0 0 1 2 11.5z" />,
    "folder-open": <path d="M2 5.5A1.5 1.5 0 0 1 3.5 4h3l1.5 1.5h4.5A1.5 1.5 0 0 1 14 7H2zm0 1.5h12l-1 5.5a1 1 0 0 1-1 .8H3a1 1 0 0 1-1-.8z" />,
    home: <path d="M2 8 8 3l6 5v5a1 1 0 0 1-1 1h-3v-4H7v4H4a1 1 0 0 1-1-1zm-1 0h2m12 0h-2" />,
    star: <path d="m8 2 1.8 4 4.2.4-3.2 2.9 1 4.3L8 11.4 4.2 13.6l1-4.3L2 6.4 6.2 6z" />,
    clock: <g><circle cx="8" cy="8" r="6.2" /><path d="M8 4.5V8l2.5 1.5" /></g>,
    print: <path d="M4 6V3h8v3m-9 0h10a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1h-1v2H4v-2H3a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1zm1 8h6v-3H5z" />,
    box: <g><path d="m8 2 6 3v6l-6 3-6-3V5z"/><path d="m2 5 6 3 6-3M8 8v7"/></g>,
    layers: <g><path d="m8 2 6 3-6 3-6-3z"/><path d="m2 8 6 3 6-3M2 11l6 3 6-3"/></g>,
    trash: <path d="M3 4h10M6 4V2.5h4V4m-5 0v9a1 1 0 0 0 1 1h4a1 1 0 0 0 1-1V4M7 7v5M9 7v5" />,
    search: <g><circle cx="7" cy="7" r="4.5" /><path d="m10.5 10.5 3 3" /></g>,
    plus: <path d="M8 3v10M3 8h10" />,
    minus: <path d="M3 8h10" />,
    chevron: <path d="m6 4 4 4-4 4" />,
    close: <path d="m4 4 8 8m0-8-8 8" />,
    grid: <g><rect x="2.5" y="2.5" width="4.5" height="4.5"/><rect x="9" y="2.5" width="4.5" height="4.5"/><rect x="2.5" y="9" width="4.5" height="4.5"/><rect x="9" y="9" width="4.5" height="4.5"/></g>,
    list: <g><path d="M5 4h9M5 8h9M5 12h9"/><circle cx="2.5" cy="4" r="0.7" fill="currentColor"/><circle cx="2.5" cy="8" r="0.7" fill="currentColor"/><circle cx="2.5" cy="12" r="0.7" fill="currentColor"/></g>,
    sort: <path d="M4 5h6m-6 3h4m-4 3h2m6-9v9m0 0L9 9.5M12 11l3-2.5" />,
    filter: <path d="M2.5 4h11l-4 5v3.5l-3 1V9z" />,
    refresh: <path d="M2 4v3h3m9 5v-3h-3M3 8a5 5 0 0 1 9-3.2L14 7M13 8a5 5 0 0 1-9 3.2L2 9" />,
    eye: <g><path d="M2 8s2.5-4 6-4 6 4 6 4-2.5 4-6 4-6-4-6-4z"/><circle cx="8" cy="8" r="2"/></g>,
    slicer: <g><path d="m2 12 6-9 6 9z"/><path d="M2 12h12M5 12l3-4.5 3 4.5"/></g>,
    history: <g><path d="M2.5 5.5a5 5 0 1 0 1.5-2.6L2 5"/><path d="M2 2v3h3M8 5v3l2 1.5"/></g>,
    moon: <path d="M13 9.5A5 5 0 0 1 6.5 3a5.5 5.5 0 1 0 6.5 6.5z" />,
    sun: <g><circle cx="8" cy="8" r="2.5"/><path d="M8 1.5v2M8 12.5v2M14.5 8h-2M3.5 8h-2M12.7 3.3l-1.4 1.4M4.7 11.3l-1.4 1.4M12.7 12.7l-1.4-1.4M4.7 4.7 3.3 3.3"/></g>,
    sparkle: <path d="M8 2v4M8 10v4M2 8h4M10 8h4M4.5 4.5l1.5 1.5M10 10l1.5 1.5M4.5 11.5 6 10M10 6l1.5-1.5" />,
    tag: <g><path d="M2 8.5V3a1 1 0 0 1 1-1h5.5L14 7.5 8.5 13 2 8.5z"/><circle cx="5" cy="5" r="0.8" fill="currentColor"/></g>,
    duplicate: <g><path d="M5 5h7v7H5z"/><path d="M3 9V3a.5.5 0 0 1 .5-.5H9"/></g>,
    error: <g><circle cx="8" cy="8" r="6"/><path d="M8 5v4M8 11v0.1"/></g>,
    cmd: <path d="M5.5 2.5a1.5 1.5 0 1 0 0 3h5a1.5 1.5 0 1 0 0-3v5m0 0a1.5 1.5 0 1 0 0 3h-5a1.5 1.5 0 1 0 0-3z" />,
    drag: <g><circle cx="6" cy="4" r="0.9" fill="currentColor"/><circle cx="10" cy="4" r="0.9" fill="currentColor"/><circle cx="6" cy="8" r="0.9" fill="currentColor"/><circle cx="10" cy="8" r="0.9" fill="currentColor"/><circle cx="6" cy="12" r="0.9" fill="currentColor"/><circle cx="10" cy="12" r="0.9" fill="currentColor"/></g>,
    rotate: <g><path d="M8 2v3M8 11v3M2 8h3M11 8h3"/><circle cx="8" cy="8" r="3"/></g>,
    fullscreen: <path d="M3 6V3h3M13 6V3h-3M3 10v3h3M13 10v3h-3" />,
    info: <g><circle cx="8" cy="8" r="6"/><path d="M8 7v4M8 5v0.1"/></g>,
    check: <path d="m3 8.5 3.5 3 6.5-7" />,
    link: <g><path d="M9 6.5h2.5a3 3 0 0 1 0 6H9M7 9.5H4.5a3 3 0 0 1 0-6H7"/><path d="M6 8h4"/></g>,
    download: <g><path d="M8 2v8m0 0L5 7m3 3 3-3"/><path d="M3 12v1.5h10V12"/></g>,
  };
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none"
         stroke="currentColor" strokeWidth={stroke}
         strokeLinecap="round" strokeLinejoin="round"
         style={{ display: "block", flexShrink: 0 }}>
      {paths[name]}
    </svg>
  );
};

// ─────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────
function fmtSize(b) {
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${(b/1024).toFixed(1)} KB`;
  return `${(b/(1024*1024)).toFixed(1)} MB`;
}
function fmtTris(n) {
  if (n == null) return "—";
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${(n/1000).toFixed(1)}K`;
  return `${(n/1_000_000).toFixed(2)}M`;
}

// Deterministic author per model id
const AUTHORS = [
  { name: "You",            handle: "@me",         color: "oklch(0.74 0.13 230)" },
  { name: "makerworld/3dx", handle: "makerworld",  color: "oklch(0.72 0.15 145)" },
  { name: "printables/jhk", handle: "printables",  color: "oklch(0.74 0.16 50)"  },
  { name: "thingiverse/aw", handle: "thingiverse", color: "oklch(0.70 0.14 295)" },
  { name: "鈴木一郎",         handle: "@suzuki",     color: "oklch(0.72 0.13 20)"  },
  { name: "김지훈",            handle: "@jihoon",     color: "oklch(0.74 0.12 195)" },
  { name: "github/cnc-lab",  handle: "cnc-lab",     color: "oklch(0.72 0.10 260)" },
];
function modelAuthor(model) {
  return AUTHORS[model.id % AUTHORS.length];
}

// Deterministic added/modified dates from model id (2024 → 2026 range)
function modelDates(model) {
  const seed = (model.id * 2654435761) >>> 0;
  const addedMonths = seed % 18;             // 0..17 months ago
  const addedDay    = (seed >> 5) % 28 + 1;
  const ageOnDisk   = ((seed >> 12) % 90) + 1;
  // dates relative to a fixed "now" (May 06 2026)
  const now = new Date(2026, 4, 6);
  const added    = new Date(now.getFullYear(), now.getMonth() - addedMonths, addedDay);
  const modified = new Date(added.getTime() + ageOnDisk * 24 * 3600 * 1000);
  return { added, modified: modified > now ? now : modified };
}
function fmtDateShort(d, lang) {
  if (!d) return "—";
  if (lang === "ko") return `${d.getFullYear()}.${String(d.getMonth()+1).padStart(2,"0")}.${String(d.getDate()).padStart(2,"0")}`;
  if (lang === "ja") return `${d.getFullYear()}/${String(d.getMonth()+1).padStart(2,"0")}/${String(d.getDate()).padStart(2,"0")}`;
  return d.toLocaleDateString("en-US", { year: "numeric", month: "short", day: "numeric" });
}
function fmtRelative(d, lang) {
  if (!d) return "—";
  const now = new Date(2026, 4, 6);
  const days = Math.floor((now - d) / (24 * 3600 * 1000));
  const T = (en, ko, ja) => lang === "ko" ? ko : lang === "ja" ? ja : en;
  if (days < 1)   return T("today", "오늘", "今日");
  if (days < 2)   return T("yesterday", "어제", "昨日");
  if (days < 7)   return T(`${days}d ago`, `${days}일 전`, `${days}日前`);
  if (days < 30)  return T(`${Math.floor(days/7)}w ago`, `${Math.floor(days/7)}주 전`, `${Math.floor(days/7)}週前`);
  if (days < 365) return T(`${Math.floor(days/30)}mo ago`, `${Math.floor(days/30)}개월 전`, `${Math.floor(days/30)}ヶ月前`);
  return T(`${Math.floor(days/365)}y ago`, `${Math.floor(days/365)}년 전`, `${Math.floor(days/365)}年前`);
}
function tagColor(name) {
  // deterministic hue from name
  let h = 0;
  for (let i = 0; i < name.length; i++) h = (h * 31 + name.charCodeAt(i)) | 0;
  const hue = Math.abs(h) % 360;
  return `oklch(0.72 0.10 ${hue})`;
}

// Mesh health (deterministic from id) — manifold, normals, etc.
function meshHealth(model) {
  if (model.type === "Unknown") {
    return { score: 0, manifold: false, watertight: false, normals: false,
             components: 0, holes: 0, degenerate: 0, error: true };
  }
  const s = (model.id * 2654435761) >>> 0;
  const isCalibration = model.tags.includes("calibration") || model.tags.includes("test");
  const manifold   = isCalibration || (s % 11) !== 0;
  const watertight = manifold && (s % 13) !== 0;
  const normals    = (s % 17) !== 0;
  const components = (s % 5 === 0) ? 2 + ((s >> 4) % 4) : 1;
  const holes      = watertight ? 0 : 1 + (s % 4);
  const degenerate = manifold ? 0 : ((s >> 8) % 6);
  const score = (manifold ? 50 : 20) + (watertight ? 25 : 0) + (normals ? 15 : 0) + (holes === 0 ? 10 : 0);
  return { score: Math.min(100, score), manifold, watertight, normals,
           components, holes, degenerate, error: false };
}

// Print estimate at default 0.20mm profile, 15% infill, PLA 1.24g/cm³
function printEstimate(model) {
  if (!model.dims || model.tris == null) return null;
  const [x, y, z] = model.dims;
  const bbox = (x * y * z) / 1000; // cm³
  // approximate solid volume → 30% of bbox for typical part
  const partVol = bbox * 0.30;
  const shellVol = partVol * 0.45;
  const infillVol = partVol * 0.55 * 0.15; // 15% infill
  const printedVol = shellVol + infillVol;
  const grams = printedVol * 1.24;
  // ~ 0.6 g/min throughput for 0.4 nozzle 0.20 layer
  const minutes = Math.max(6, Math.round(grams / 0.6));
  const lengthM = +(grams / 2.98).toFixed(1); // PLA 1.75mm
  return {
    grams: +grams.toFixed(0),
    minutes,
    hours: Math.floor(minutes / 60),
    mins: minutes % 60,
    lengthM,
    layers: Math.ceil(z / 0.20),
    bedFit: x <= 256 && y <= 256 && z <= 256,
    bedSize: "Bambu P1S · 256 × 256 × 256",
  };
}

// Source / provenance
const SOURCE_HOSTS = [
  { host: "printables.com",  user: "ronan_co",       date: "Jan 12, 2026" },
  { host: "thingiverse.com", user: "makers_dad",     date: "Mar 04, 2025" },
  { host: "makerworld.com",  user: "studio_fold",    date: "Feb 21, 2026" },
  { host: "thangs.com",      user: "cnc-lab",        date: "Sep 03, 2025" },
  null, null, // some are local-only
];
function modelSource(model) {
  return SOURCE_HOSTS[model.id % SOURCE_HOSTS.length];
}

// Sample print history per model
function printHistory(model) {
  if (model.printed === 0) return [];
  const hist = [];
  const printers = ["Bambu P1S", "Snapmaker A350", "Prusa MK4"];
  const filaments = ["PLA Black", "PETG White", "PLA+ Gray", "ABS Black", "PLA Silk Blue"];
  const dates = ["May 04", "Apr 28", "Apr 21", "Apr 12", "Mar 30", "Mar 15"];
  const n = Math.min(model.printed, 4);
  for (let i = 0; i < n; i++) {
    const ok = !(i === 1 && model.id % 7 === 0);
    hist.push({
      when: dates[i],
      printer: printers[(model.id + i) % printers.length],
      filament: filaments[(model.id * 2 + i) % filaments.length],
      ok,
      note: ok ? null : "First layer adhesion failed at 12%",
    });
  }
  return hist;
}

// ─────────────────────────────────────────────────────────
// Sidebar
// ─────────────────────────────────────────────────────────
function Sidebar({ filter, setFilter, L }) {
  const folders = window.FOLDERS;
  const tags = window.TAGS;

  const SmartFilter = ({ id, icon, label, count }) => (
    <div className={`sidebar-item ${filter.kind === "smart" && filter.value === id ? "active" : ""}`}
         onClick={() => setFilter({ kind: "smart", value: id })}>
      <span className="icon"><Icon name={icon} /></span>
      <span className="label">{label}</span>
      <span className="count">{count}</span>
    </div>
  );

  const FolderItem = ({ f, depth = 0 }) => {
    const [open, setOpen] = useState(true);
    const cls = `sidebar-item ${filter.kind === "folder" && filter.value === f.path ? "active" : ""}` +
                (depth === 1 ? " nested" : depth === 2 ? " nested-2" : "");
    return (
      <>
        <div className={cls}
             onClick={() => setFilter({ kind: "folder", value: f.path })}>
          {f.children ? (
            <span className={`twist ${open ? "open" : ""}`}
                  onClick={(e) => { e.stopPropagation(); setOpen(!open); }}>
              <Icon name="chevron" size={9} stroke={2} />
            </span>
          ) : <span style={{ width: 10 }} />}
          <span className="icon"><Icon name={open && f.children ? "folder-open" : "folder"} /></span>
          <span className="label">{f.path.split("/").pop()}</span>
          <span className="count">{f.count}</span>
        </div>
        {open && f.children?.map(c => <FolderItem key={c.path} f={c} depth={depth + 1} />)}
      </>
    );
  };

  return (
    <div className="sidebar">
      <div className="sidebar-list" style={{ flex: "0 0 auto" }}>
        <div className="sidebar-section">
          <div className="sidebar-title">{L.sidebar_library}</div>
          <SmartFilter id="all"     icon="layers"  label={L.smart_all}     count={36} />
          <SmartFilter id="recent"  icon="clock"   label={L.smart_recent}  count={12} />
          <SmartFilter id="fav"     icon="star"    label={L.smart_fav}     count={9} />
          <SmartFilter id="printed" icon="print"   label={L.smart_printed} count={28} />
          <SmartFilter id="ready"   icon="box"     label={L.smart_ready}   count={4} />
          <SmartFilter id="dups"    icon="duplicate" label={L.smart_dups}  count={2} />
          <SmartFilter id="errors"  icon="error"   label={L.smart_errors}  count={1} />
        </div>
      </div>

      <div className="sidebar-list" style={{ flex: "1 1 auto", borderTop: "1px solid var(--line)" }}>
        <div className="sidebar-section">
          <div className="sidebar-title">
            {L.sidebar_folders}
            <span className="add"><Icon name="plus" size={12} stroke={2} /></span>
          </div>
          {folders.map(f => <FolderItem key={f.path} f={f} />)}
        </div>

        <div className="sidebar-section">
          <div className="sidebar-title">
            {L.sidebar_tags}
            <span className="add"><Icon name="plus" size={12} stroke={2} /></span>
          </div>
          {tags.slice(0, 10).map(t => (
            <div key={t.name}
                 className={`sidebar-item ${filter.kind === "tag" && filter.value === t.name ? "active" : ""}`}
                 onClick={() => setFilter({ kind: "tag", value: t.name })}>
              <span className="icon" style={{ display: "flex", alignItems: "center", justifyContent: "center" }}>
                <span style={{ width: 8, height: 8, borderRadius: "50%", background: tagColor(t.name) }} />
              </span>
              <span className="label">{t.name}</span>
              <span className="count">{t.count}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────
// Toolbar
// ─────────────────────────────────────────────────────────
function Toolbar({ view, setView, sort, setSort, search, setSearch, density, setDensity, L }) {
  return (
    <div className="toolbar">
      <div className="search">
        <Icon name="search" />
        <input value={search} onChange={(e) => setSearch(e.target.value)}
               placeholder={L.search_placeholder} />
        <span className="kbd">⌘K</span>
      </div>

      <div className="seg">
        <button className={view === "grid" ? "on" : ""} onClick={() => setView("grid")}>
          <Icon name="grid" size={12} /> {L.view_grid}
        </button>
        <button className={view === "masonry" ? "on" : ""} onClick={() => setView("masonry")}>
          <Icon name="layers" size={12} /> {L.view_masonry || "Masonry"}
        </button>
        <button className={view === "list" ? "on" : ""} onClick={() => setView("list")}>
          <Icon name="list" size={12} /> {L.view_list}
        </button>
      </div>

      <div className="seg" title="Thumbnail size">
        <button className={density === "s" ? "on" : ""} onClick={() => setDensity("s")} style={{ padding: "0 6px" }}>S</button>
        <button className={density === "m" ? "on" : ""} onClick={() => setDensity("m")} style={{ padding: "0 6px" }}>M</button>
        <button className={density === "l" ? "on" : ""} onClick={() => setDensity("l")} style={{ padding: "0 6px" }}>L</button>
      </div>

      <div className="divider" />

      <button className="btn" title="Sort">
        <Icon name="sort" size={13} />
        <span style={{ color: "var(--fg-1)" }}>{L.sort_name_asc}</span>
      </button>

      <div className="toolbar-spacer" />

      <button className="btn" title="Refresh"><Icon name="refresh" size={13} /></button>
      <button className="btn primary">
        <Icon name="plus" size={13} stroke={2} />
        {L.add_folder}
      </button>
    </div>
  );
}

// ─────────────────────────────────────────────────────────
// Filter chip bar
// ─────────────────────────────────────────────────────────
function FilterBar({ filter, setFilter, search, setSearch, count, L }) {
  const hasFilter = filter.kind !== "smart" || filter.value !== "all";
  const hasSearch = search.length > 0;
  if (!hasFilter && !hasSearch) {
    return (
      <div className="filterbar">
        <span className="label">{L.filter_all}</span>
        <span className="count-status">{L.filter_count(count)}</span>
      </div>
    );
  }

  return (
    <div className="filterbar">
      <span className="label">{L.filter_filtering}</span>
      {hasFilter && (
        <span className="chip active">
          {filter.kind === "folder" && <Icon name="folder" size={11} />}
          {filter.kind === "tag" && <span className="dot" style={{
            width: 7, height: 7, borderRadius: "50%", background: tagColor(filter.value)
          }} />}
          {filter.kind === "smart" && <Icon name={
            { recent: "clock", fav: "star", printed: "print", ready: "box",
              dups: "duplicate", errors: "error" }[filter.value] || "layers"
          } size={11} />}
          <span>{filter.value}</span>
          <span className="x" onClick={() => setFilter({ kind: "smart", value: "all" })}>
            <Icon name="close" size={9} stroke={2} />
          </span>
        </span>
      )}
      {hasSearch && (
        <span className="chip active">
          <Icon name="search" size={11} />
          <span>"{search}"</span>
          <span className="x" onClick={() => setSearch("")}>
            <Icon name="close" size={9} stroke={2} />
          </span>
        </span>
      )}
      <span className="count-status">{L.filter_count(count)}</span>
    </div>
  );
}

// ─────────────────────────────────────────────────────────
// Card
// ─────────────────────────────────────────────────────────
function Card({ model, selected, onSelect, density, aspect }) {
  const thumbStyle = aspect ? { aspectRatio: String(aspect) } : undefined;
  return (
    <div className={`card ${selected ? "selected" : ""}`}
         onClick={() => onSelect(model.id)}
         title={model.name}>
      <div className="thumb" style={thumbStyle}>
        <div className="thumb-bg-pattern" />
        <Thumb thumb={model.thumb} />
        <div className="card-badges">
          {model.status === "printed" && model.printed > 0 && (
            <span className="badge printed">
              <Icon name="print" size={9} stroke={2} /> {model.printed}
            </span>
          )}
          {model.status === "queued" && (
            <span className="badge queued">QUEUED</span>
          )}
          {model.status === "error" && (
            <span className="badge error">
              <Icon name="error" size={9} stroke={2} /> ERR
            </span>
          )}
        </div>
        {model.fav && (
          <div className="fav-mark">
            <Icon name="star" size={11} stroke={2} />
          </div>
        )}
      </div>
      <div className="card-body">
        <div className="card-name">{model.name}</div>
        <div className="card-meta">
          <span className="card-author" title={modelAuthor(model).name}>
            <span className="card-author-dot" style={{ background: modelAuthor(model).color }} />
            {modelAuthor(model).name}
          </span>
          <span className="sep" />
          <span>{fmtSize(model.size)}</span>
          {model.tris != null && (<><span className="sep" /><span>{fmtTris(model.tris)} tri</span></>)}
        </div>
        <div className="card-meta-2">
          <Icon name="clock" size={9} />
          <span>{fmtRelative(modelDates(model).modified, "en")}</span>
        </div>
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────
// List view
// ─────────────────────────────────────────────────────────
function ListView({ models, selectedId, onSelect }) {
  return (
    <div className="list">
      <div className="list-row head">
        <span></span>
        <span>Name</span>
        <span>Size</span>
        <span>Triangles</span>
        <span>Format</span>
        <span>Printed</span>
      </div>
      {models.map(m => (
        <div key={m.id}
             className={`list-row ${m.id === selectedId ? "selected" : ""}`}
             onClick={() => onSelect(m.id)}>
          <div className="thumb-mini"><Thumb thumb={m.thumb} /></div>
          <div className="name">{m.name}</div>
          <div className="num">{fmtSize(m.size)}</div>
          <div className="num">{fmtTris(m.tris)}</div>
          <div className="num">{m.type}</div>
          <div className="num">{m.printed > 0 ? `${m.printed}×` : "—"}</div>
        </div>
      ))}
    </div>
  );
}

// ─────────────────────────────────────────────────────────
// Detail panel
// ─────────────────────────────────────────────────────────
function DetailPanel({ model, L, settings, updateSettings }) {
  const [slicerMenu, setSlicerMenu] = useState(false);
  useEffect(() => {
    if (!slicerMenu) return;
    const onDoc = () => setSlicerMenu(false);
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [slicerMenu]);
  const SlicerAppIcon = ({ id }) => {
    if (id === "orca") return (
      <svg viewBox="0 0 24 24" width="22" height="22" style={{ display: "block" }}>
        <rect width="24" height="24" rx="6" fill="#0E1820"/>
        <path d="M5 14c2-4 6-6 9-5.5 1.4.2 2.6 1 3.5 2L19 8.5l-.4 4c.6 1.4.5 3-.5 4.5-1.7 2.4-5.5 3-8.5 1.5C7 17.4 5.6 16 5 14z" fill="#1FB8C8"/>
        <path d="M11.5 11.5c1.4-.4 3 .2 4 1.5l-2 1.5c-.6-.7-1.4-1.1-2.3-1z" fill="#fff"/>
        <circle cx="15.5" cy="12.2" r="0.7" fill="#0E1820"/>
      </svg>
    );
    if (id === "bambu") return (
      <svg viewBox="0 0 24 24" width="22" height="22" style={{ display: "block" }}>
        <rect width="24" height="24" rx="6" fill="#0F2419"/>
        <path d="M7 6c2 0 3 1 3 3v3c0 1.5 1 2.5 2.5 2.5S15 13.5 15 12V8c0-1.4 1-2 2-2v6c0 3-2 5-4.5 5S8 15 8 12V9c0-2-1-3-3-3z" fill="#16C47F"/>
        <circle cx="6.5" cy="7" r="1" fill="#16C47F"/>
        <circle cx="17.5" cy="7" r="1" fill="#16C47F"/>
      </svg>
    );
    if (id === "prusa") return (
      <svg viewBox="0 0 24 24" width="22" height="22" style={{ display: "block" }}>
        <rect width="24" height="24" rx="6" fill="#1A1208"/>
        <path d="M12 4 4 8.5v7L12 20l8-4.5v-7z" fill="#FA6831"/>
        <path d="M12 7 7 9.7v4.6L12 17l5-2.7V9.7z" fill="#1A1208"/>
        <text x="12" y="14" textAnchor="middle" fontFamily="system-ui, sans-serif" fontWeight="800" fontSize="7" fill="#FA6831">P</text>
      </svg>
    );
    if (id === "super") return (
      <svg viewBox="0 0 24 24" width="22" height="22" style={{ display: "block" }}>
        <rect width="24" height="24" rx="6" fill="#1A0E1A"/>
        <path d="M12 3 5 7v6.5C5 17 8 20 12 21c4-1 7-4 7-7.5V7z" fill="#E11D48"/>
        <path d="M9 11h6M9.5 8.5l-1.2-1.2M14.5 8.5l1.2-1.2M12 6.5V5" stroke="#fff" strokeWidth="1.4" strokeLinecap="round"/>
        <path d="m9.5 14 2.5 3 2.5-3" stroke="#fff" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" fill="none"/>
      </svg>
    );
    if (id === "cura") return (
      <svg viewBox="0 0 24 24" width="22" height="22" style={{ display: "block" }}>
        <rect width="24" height="24" rx="6" fill="#0B2434"/>
        <circle cx="12" cy="12" r="6.5" fill="none" stroke="#06D6E0" strokeWidth="2.2"/>
        <path d="M16.5 6.5 19 4M14.5 9.5 17 7" stroke="#06D6E0" strokeWidth="2.2" strokeLinecap="round"/>
      </svg>
    );
    return <span>{(id || "?").charAt(0).toUpperCase()}</span>;
  };
  const SLICERS = [
    { id: "orca",  name: "OrcaSlicer",   sub: "0.20mm Standard · PLA" },
    { id: "bambu", name: "Bambu Studio", sub: "P1S · 0.16mm Fine" },
    { id: "prusa", name: "PrusaSlicer",  sub: "MK4 · 0.20mm Speed" },
    { id: "super", name: "SuperSlicer",  sub: "Custom profile" },
    { id: "cura",  name: "UltiMaker Cura", sub: "Standard" },
  ];
  const defaultSlicerId = settings ? settings.slicer : "orca";
  const defaultSlicer = SLICERS.find(s => s.id === defaultSlicerId) || SLICERS[0];
  if (!model) {
    return (
      <div className="detail">
        <div className="detail-empty">
          <div className="ico"><Icon name="box" size={20} /></div>
          <div style={{ fontSize: 12.5, color: "var(--fg-1)" }}>{L.detail_select}</div>
          <div style={{ fontSize: 11.5, color: "var(--fg-2)" }}>
            {L.detail_select_hint}
          </div>
        </div>
      </div>
    );
  }

  const dims = model.dims;
  const history = printHistory(model);
  const health = meshHealth(model);
  const estimate = printEstimate(model);
  const source = modelSource(model);

  return (
    <div className="detail">
      <div className="detail-preview">
        <div className="thumb-bg-pattern" style={{ opacity: 0.6 }} />
        <Thumb thumb={model.thumb} />
        <div className="preview-controls">
          <div className="ctl" title="Rotate"><Icon name="rotate" size={12} /></div>
          <div className="ctl" title="Fullscreen"><Icon name="fullscreen" size={12} /></div>
        </div>
        <div className="preview-overlay">
          <div className="axis">
            <span className="ax x">X</span>
            <span className="ax y">Y</span>
            <span className="ax z">Z</span>
          </div>
          <div>{L.detail_orbit_hint}</div>
        </div>
      </div>

      <div className="detail-scroll">
        <div className="detail-name">{model.name}</div>
        <div className="detail-path">~/Library/3d/{model.folder}/</div>

        <div className="detail-actions">
          <div className="split-btn" onMouseDown={(e) => e.stopPropagation()}>
            <button className="btn primary split-main" title={`${L.detail_open_in} ${defaultSlicer.name}`}>
              <Icon name="slicer" size={12} /> {L.detail_open_slicer}
              <span className="split-sub">{defaultSlicer.name}</span>
            </button>
            <button className={"btn primary split-caret " + (slicerMenu ? "on" : "")}
                    title={L.detail_choose_slicer}
                    onClick={() => setSlicerMenu(v => !v)}>
              <Icon name="chevron" size={9} stroke={2.2} />
            </button>
            {slicerMenu && (
              <div className="slicer-menu" onMouseDown={(e) => e.stopPropagation()}>
                <div className="slicer-menu-head">{L.detail_open_with}</div>
                {SLICERS.map(s => (
                  <button key={s.id}
                          className={"slicer-menu-item " + (s.id === defaultSlicerId ? "active" : "")}
                          onClick={() => {
                            if (updateSettings) updateSettings({ slicer: s.id });
                            setSlicerMenu(false);
                          }}>
                    <span className="slicer-icon"><SlicerAppIcon id={s.id} /></span>
                    <span className="slicer-name">
                      <span>{s.name}</span>
                      <span className="slicer-sub">{s.sub}</span>
                    </span>
                    {s.id === defaultSlicerId && <Icon name="check" size={11} stroke={2} />}
                  </button>
                ))}
                <div className="slicer-menu-foot">
                  <button className="slicer-menu-link">
                    <Icon name="plus" size={10} stroke={2} /> {L.detail_add_slicer}
                  </button>
                </div>
              </div>
            )}
          </div>
          <button className="btn icon-only" title="Mark printed"><Icon name="print" size={13} /></button>
          <button className="btn icon-only" title="Favorite">
            <Icon name="star" size={13} stroke={model.fav ? 2 : 1.5} />
          </button>
        </div>

        <div className="detail-section">
          <h3>{L.detail_geometry}</h3>
          <div className="kv">
            <div className="k">{L.detail_format}</div>
            <div className="v">{model.type === "Unknown" ? <span style={{ color: "var(--error)" }}>{L.detail_format_unknown}</span> : (model.type === "Binary" ? L.detail_format_binary : L.detail_format_ascii)}</div>
            {model.tris != null && <><div className="k">{L.detail_triangles}</div><div className="v">{fmtTris(model.tris)}</div></>}
            {dims && <>
              <div className="k">{L.detail_dimensions}</div>
              <div className="v">
                {dims[0].toFixed(1)} <span style={{ color: "var(--fg-3)" }}>×</span>{" "}
                {dims[1].toFixed(1)} <span style={{ color: "var(--fg-3)" }}>×</span>{" "}
                {dims[2].toFixed(1)} <span style={{ color: "var(--fg-3)" }}>mm</span>
              </div>
              <div className="k">{L.detail_bbox}</div>
              <div className="v">{(dims[0]*dims[1]*dims[2]/1000).toFixed(1)} cm³ <span style={{ color: "var(--fg-3)" }}>· bbox</span></div>
              <div className="k">{L.detail_volume}</div>
              <div className="v">{(dims[0]*dims[1]*dims[2]/1000 * 0.30).toFixed(1)} cm³ <span style={{ color: "var(--fg-3)" }}>· est. solid</span></div>
              <div className="k">{L.detail_longest}</div>
              <div className="v">{Math.max(...dims).toFixed(1)} mm <span style={{ color: "var(--fg-3)" }}>· {Math.max(...dims) === dims[0] ? "X" : Math.max(...dims) === dims[1] ? "Y" : "Z"}</span></div>
            </>}
            <div className="k">{L.detail_filesize}</div>
            <div className="v">{fmtSize(model.size)}</div>
          </div>
        </div>

        {!health.error && (
          <div className="detail-section">
            <h3>
              {L.detail_health}
              <span className={"health-badge " + (health.score >= 90 ? "ok" : health.score >= 65 ? "warn" : "bad")}>
                {health.score}<span className="suffix">/100</span>
              </span>
            </h3>
            <div className="health-grid">
              <div className={"health-item " + (health.manifold ? "ok" : "bad")}>
                <Icon name={health.manifold ? "check" : "close"} size={11} stroke={2} />
                <span>{L.detail_manifold}</span>
              </div>
              <div className={"health-item " + (health.watertight ? "ok" : "warn")}>
                <Icon name={health.watertight ? "check" : "close"} size={11} stroke={2} />
                <span>{L.detail_watertight}</span>
              </div>
              <div className={"health-item " + (health.normals ? "ok" : "warn")}>
                <Icon name={health.normals ? "check" : "close"} size={11} stroke={2} />
                <span>{L.detail_normals}</span>
              </div>
              <div className={"health-item " + (health.components === 1 ? "ok" : "info")}>
                <Icon name="box" size={11} />
                <span>{health.components} {L.detail_components}</span>
              </div>
            </div>
            {(health.holes > 0 || health.degenerate > 0) && (
              <div className="health-issues">
                {health.holes > 0 && <span><span className="dot warn" />{health.holes} {L.detail_holes}</span>}
                {health.degenerate > 0 && <span><span className="dot warn" />{health.degenerate} {L.detail_degenerate}</span>}
                <button className="health-fix">{L.detail_repair}</button>
              </div>
            )}
          </div>
        )}

        {estimate && (
          <div className="detail-section">
            <h3>
              {L.detail_estimate}
              <span className="section-sub">0.20mm · 15% infill · PLA</span>
            </h3>
            <div className="estimate-row">
              <div className="est-item">
                <div className="est-label">{L.detail_est_time}</div>
                <div className="est-value">
                  {estimate.hours > 0 && <span>{estimate.hours}<span className="u">h</span></span>}
                  <span>{estimate.mins}<span className="u">m</span></span>
                </div>
              </div>
              <div className="est-item">
                <div className="est-label">{L.detail_est_filament}</div>
                <div className="est-value">{estimate.grams}<span className="u">g</span></div>
                <div className="est-sub">{estimate.lengthM} m</div>
              </div>
              <div className="est-item">
                <div className="est-label">{L.detail_est_layers}</div>
                <div className="est-value">{estimate.layers}</div>
              </div>
            </div>
            <div className={"bed-fit " + (estimate.bedFit ? "ok" : "warn")}>
              <Icon name={estimate.bedFit ? "check" : "close"} size={10} stroke={2} />
              <span>{estimate.bedFit ? L.detail_bed_fits : L.detail_bed_no_fit}</span>
              <code>{estimate.bedSize}</code>
            </div>
          </div>
        )}

        <div className="detail-section">
          <h3>{L.detail_tags}</h3>
          <div className="tags-row">
            {model.tags.map(t => (
              <span key={t} className="tag-pill">
                <span className="dot" style={{ background: tagColor(t) }} />
                {t}
              </span>
            ))}
            <span className="tag-pill add">
              <Icon name="plus" size={9} stroke={2} /> {L.detail_tag_add}
            </span>
          </div>
        </div>

        {history.length > 0 && (
          <div className="detail-section">
            <h3>{L.detail_history(model.printed)}</h3>
            {history.map((h, i) => (
              <div className="history-item" key={i}>
                <div className="when">{h.when}</div>
                <div className="body">
                  <div className="head">
                    <span className={`status ${h.ok ? "ok" : "fail"}`} />
                    <span>{h.printer}</span>
                  </div>
                  <div className="meta">{h.filament}{h.note ? ` · ${h.note}` : ""}</div>
                </div>
              </div>
            ))}
          </div>
        )}

        <div className="detail-section">
          <h3>{L.detail_notes}</h3>
          <div className="notes" contentEditable suppressContentEditableWarning>
            {model.id === 1 && "v2 fixes the M3 hole spacing — printed in PETG at 0.2mm."}
            {model.id === 7 && "Universal mount — works for 1kg and 250g spools."}
            {model.id !== 1 && model.id !== 7 && (
              <span style={{ color: "var(--fg-3)" }}>{L.detail_notes_empty}</span>
            )}
          </div>
        </div>

        {source && (
          <div className="detail-section">
            <h3>{L.detail_source}</h3>
            <div className="source-card">
              <div className="source-icon">{source.host.charAt(0).toUpperCase()}</div>
              <div className="source-body">
                <div className="source-host">{source.host}</div>
                <div className="source-meta">@{source.user} · {source.date}</div>
              </div>
              <button className="source-open" title={L.detail_source_open}>
                <Icon name="link" size={11} />
              </button>
            </div>
          </div>
        )}

        <div className="detail-section">
          <h3>{L.detail_file}</h3>
          <div className="kv">
            <div className="k">{L.detail_hash}</div>
            <div className="v" style={{ fontSize: 10.5 }}>
              {Array.from({ length: 8 }, (_, i) =>
                ((model.id * 17 + i * 13) % 16).toString(16) +
                ((model.id * 31 + i * 7) % 16).toString(16)
              ).join("")}…
            </div>
            <div className="k">{L.detail_modified}</div>
            <div className="v">
              {fmtDateShort(modelDates(model).modified, L === window.I18N.ko ? "ko" : L === window.I18N.ja ? "ja" : "en")}
              <span style={{ color: "var(--fg-3)", marginLeft: 6, fontSize: 11 }}>
                {fmtRelative(modelDates(model).modified, L === window.I18N.ko ? "ko" : L === window.I18N.ja ? "ja" : "en")}
              </span>
            </div>
            <div className="k">{L.detail_added}</div>
            <div className="v">
              {fmtDateShort(modelDates(model).added, L === window.I18N.ko ? "ko" : L === window.I18N.ja ? "ja" : "en")}
              <span style={{ color: "var(--fg-3)", marginLeft: 6, fontSize: 11 }}>
                {fmtRelative(modelDates(model).added, L === window.I18N.ko ? "ko" : L === window.I18N.ja ? "ja" : "en")}
              </span>
            </div>
            <div className="k">{L.detail_author || "Author"}</div>
            <div className="v" style={{ display: "flex", alignItems: "center", gap: 6 }}>
              <span style={{
                width: 16, height: 16, borderRadius: "50%",
                background: modelAuthor(model).color,
                display: "inline-flex", alignItems: "center", justifyContent: "center",
                fontSize: 9, fontWeight: 600, color: "oklch(0.12 0.02 250)",
              }}>{modelAuthor(model).name.charAt(0)}</span>
              <span>{modelAuthor(model).name}</span>
              <span style={{ color: "var(--fg-3)", fontFamily: "var(--mono)", fontSize: 11 }}>
                {modelAuthor(model).handle}
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────
// Status bar
// ─────────────────────────────────────────────────────────
function StatusBar({ count, total, selectedId, scanState, L }) {
  return (
    <div className="statusbar">
      <span className={`dot ${scanState.scanning ? "scanning" : ""}`} />
      {scanState.scanning ? (
        <>
          <span>{L.status_scanning} {scanState.current}…</span>
          <span style={{ color: "var(--fg-3)" }}>{scanState.found} · {scanState.skipped}</span>
        </>
      ) : (
        <>
          <span>{L.status_ready}</span>
          <span style={{ color: "var(--fg-3)" }}>{L.status_models(total)}</span>
        </>
      )}

      <div className="right">
        {selectedId && <span>{L.status_id(selectedId)}</span>}
        <span>{L.status_shown(count)}</span>
        <span>v0.1.0</span>
      </div>
    </div>
  );
}

// ─────────────────────────────────────────────────────────
// Settings dialog
// ─────────────────────────────────────────────────────────
function SettingsDialog({ open, onClose, settings, update, L }) {
  const [pane, setPane] = useState("appearance");
  if (!open) return null;

  const panes = [
    { id: "general",    icon: "sparkle",   label: L.settings_general },
    { id: "appearance", icon: "sun",       label: L.settings_appearance },
    { id: "library",    icon: "folder",    label: L.settings_library },
    { id: "thumbnails", icon: "box",       label: L.settings_thumbnails },
    { id: "slicer",     icon: "slicer",    label: L.settings_slicer },
    { id: "hotkeys",    icon: "cmd",       label: L.settings_hotkeys },
    { id: "advanced",   icon: "sparkle",   label: L.settings_advanced },
    { id: "about",      icon: "info",      label: L.settings_about },
  ];

  const Row = ({ label, hint, children }) => (
    <div className="settings-row">
      <div className="settings-row-label">
        <div>{label}</div>
        {hint && <div className="hint">{hint}</div>}
      </div>
      <div className="settings-row-control">{children}</div>
    </div>
  );

  const Seg = ({ value, options, onChange }) => (
    <div className="settings-seg">
      {options.map(o => (
        <button key={o.value}
                className={value === o.value ? "on" : ""}
                onClick={() => onChange(o.value)}>
          {o.label}
        </button>
      ))}
    </div>
  );

  const Swatch = ({ value, options, onChange }) => (
    <div style={{ display: "flex", gap: 8 }}>
      {options.map(o => (
        <button key={o.value}
                onClick={() => onChange(o.value)}
                title={o.label}
                className={"settings-swatch " + (value === o.value ? "on" : "")}
                style={{ background: o.color }} />
      ))}
    </div>
  );

  return (
    <div className="settings-backdrop" onClick={onClose}>
      <div className="settings-dialog" onClick={e => e.stopPropagation()}>
        <div className="settings-sidebar">
          <div className="settings-app">
            <window.AppIcon size={44} />
            <div>
              <div className="settings-app-name">ModelRack</div>
              <div className="settings-app-ver">v0.1.0</div>
            </div>
          </div>
          {panes.map(p => (
            <div key={p.id}
                 className={"settings-tab " + (pane === p.id ? "active" : "")}
                 onClick={() => setPane(p.id)}>
              <Icon name={p.icon} size={13} />
              <span>{p.label}</span>
            </div>
          ))}
        </div>

        <div className="settings-body">
          <div className="settings-header">
            <div className="settings-title">{panes.find(p => p.id === pane).label}</div>
            <button className="settings-close" onClick={onClose}>
              <Icon name="close" size={12} stroke={2} />
            </button>
          </div>

          <div className="settings-content">
            {pane === "general" && (
              <>
                <Row label={L.settings_lang}>
                  <Seg value={settings.lang}
                       options={[
                         { value: "en", label: "English" },
                         { value: "ko", label: "한국어" },
                         { value: "ja", label: "日本語" },
                       ]}
                       onChange={(v) => update({ lang: v })} />
                </Row>
                <Row label={L.settings_startup} hint={L.settings_startup_hint}>
                  <Seg value={settings.startupFolder}
                       options={[
                         { value: "last",  label: L.settings_startup_last },
                         { value: "empty", label: L.settings_startup_empty },
                       ]}
                       onChange={(v) => update({ startupFolder: v })} />
                </Row>
                <Row label={L.settings_default_view}>
                  <Seg value={settings.defaultView}
                       options={[
                         { value: "grid",    label: L.view_grid },
                         { value: "masonry", label: L.view_masonry },
                         { value: "list",    label: L.view_list },
                       ]}
                       onChange={(v) => update({ defaultView: v })} />
                </Row>
                <Row label={L.settings_default_sort}>
                  <select className="settings-text" style={{ maxWidth: 220 }}
                          value={settings.defaultSort}
                          onChange={(e) => update({ defaultSort: e.target.value })}>
                    <option value="name-asc">{L.sort_name_asc}</option>
                    <option value="name-desc">{L.sort_name_desc}</option>
                    <option value="modified">{L.sort_modified}</option>
                    <option value="added">{L.sort_added}</option>
                    <option value="size">{L.sort_size}</option>
                    <option value="tris">{L.sort_tris}</option>
                  </select>
                </Row>
                <Row label={L.settings_date_format}>
                  <Seg value={settings.dateFormat}
                       options={[
                         { value: "auto", label: L.settings_date_auto },
                         { value: "iso",  label: "2026-05-08" },
                         { value: "us",   label: "May 8, 2026" },
                       ]}
                       onChange={(v) => update({ dateFormat: v })} />
                </Row>
                <Row label={L.settings_show_extensions}>
                  <SettingsToggle on={settings.showExtensions} onChange={(v) => update({ showExtensions: v })} />
                </Row>
                <Row label={L.settings_show_hidden} hint={L.settings_show_hidden_hint}>
                  <SettingsToggle on={settings.showHidden} onChange={(v) => update({ showHidden: v })} />
                </Row>
                <Row label={L.settings_confirm_delete}>
                  <SettingsToggle on={settings.confirmDelete} onChange={(v) => update({ confirmDelete: v })} />
                </Row>
              </>
            )}

            {pane === "appearance" && (
              <>
                <Row label={L.settings_theme}>
                  <Seg value={settings.theme}
                       options={[
                         { value: "dark",  label: L.settings_theme_dark },
                         { value: "light", label: L.settings_theme_light },
                         { value: "auto",  label: L.settings_theme_auto },
                       ]}
                       onChange={(v) => update({ theme: v })} />
                </Row>
                <Row label={L.settings_accent}>
                  <Swatch value={settings.accent}
                          options={[
                            { value: "teal",   color: "oklch(0.74 0.13 230)", label: "Teal" },
                            { value: "violet", color: "oklch(0.70 0.18 295)", label: "Violet" },
                            { value: "orange", color: "oklch(0.74 0.16 50)",  label: "Orange" },
                            { value: "green",  color: "oklch(0.74 0.15 155)", label: "Green" },
                          ]}
                          onChange={(v) => update({ accent: v })} />
                </Row>
                <Row label={L.settings_density}>
                  <Seg value={settings.density}
                       options={[
                         { value: "comfortable", label: L.settings_density_comfort },
                         { value: "compact",     label: L.settings_density_compact },
                       ]}
                       onChange={(v) => update({ density: v })} />
                </Row>
                <Row label={L.settings_sidebar_pos}>
                  <Seg value={settings.sidebarPos}
                       options={[
                         { value: "left",  label: L.settings_left },
                         { value: "right", label: L.settings_right },
                       ]}
                       onChange={(v) => update({ sidebarPos: v })} />
                </Row>
                <Row label={L.settings_card_label} hint={L.settings_card_label_hint}>
                  <Seg value={settings.cardLabel}
                       options={[
                         { value: "filename", label: L.settings_card_filename },
                         { value: "titled",   label: L.settings_card_titled },
                       ]}
                       onChange={(v) => update({ cardLabel: v })} />
                </Row>
                <Row label={L.settings_thumb_bg}>
                  <Seg value={settings.thumbBg}
                       options={[
                         { value: "solid",   label: L.settings_thumb_bg_solid },
                         { value: "checker", label: L.settings_thumb_bg_checker },
                         { value: "dot",     label: L.settings_thumb_bg_dot },
                       ]}
                       onChange={(v) => update({ thumbBg: v })} />
                </Row>
                <Row label={L.settings_reduce_motion}>
                  <SettingsToggle on={settings.reduceMotion} onChange={(v) => update({ reduceMotion: v })} />
                </Row>
              </>
            )}

            {pane === "library" && (
              <>
                <Row label={L.settings_lib_paths}>
                  <div className="settings-paths">
                    <div className="path-row">
                      <Icon name="folder" size={12} />
                      <code>~/Library/3d</code>
                      <span className="path-count">36</span>
                      <button className="path-x"><Icon name="close" size={10} stroke={2} /></button>
                    </div>
                    <div className="path-row">
                      <Icon name="folder" size={12} />
                      <code>~/Downloads/stl</code>
                      <span className="path-count">8</span>
                      <button className="path-x"><Icon name="close" size={10} stroke={2} /></button>
                    </div>
                    <button className="path-add">
                      <Icon name="plus" size={11} stroke={2} /> {L.add_folder}
                    </button>
                  </div>
                </Row>
                <Row label={L.settings_file_types}>
                  <div className="settings-checks">
                    {[["stl","STL"],["threeMF","3MF"],["step","STEP"],["obj","OBJ"]].map(([k,lbl]) => (
                      <label key={k} className="settings-check">
                        <input type="checkbox" checked={settings.fileTypes[k]}
                               onChange={(e) => update({ fileTypes: { ...settings.fileTypes, [k]: e.target.checked } })} />
                        <span>{lbl}</span>
                      </label>
                    ))}
                  </div>
                </Row>
                <Row label={L.settings_lib_recursive}>
                  <SettingsToggle on={settings.libRecursive} onChange={(v) => update({ libRecursive: v })} />
                </Row>
                <Row label={L.settings_lib_watch} hint={L.settings_lib_watch_hint}>
                  <SettingsToggle on={settings.libWatch} onChange={(v) => update({ libWatch: v })} />
                </Row>
                <Row label={L.settings_exclude} hint={L.settings_exclude_hint}>
                  <input className="settings-text"
                         value={settings.excludePatterns}
                         onChange={(e) => update({ excludePatterns: e.target.value })} />
                </Row>
                <Row label={L.settings_sidecar} hint={L.settings_sidecar_hint}>
                  <Seg value={settings.sidecar}
                       options={[
                         { value: "json", label: L.settings_sidecar_json },
                         { value: "db",   label: L.settings_sidecar_db },
                         { value: "none", label: L.settings_sidecar_none },
                       ]}
                       onChange={(v) => update({ sidecar: v })} />
                </Row>
                <Row label={L.settings_lib_size_cap}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <input className="settings-num" type="number"
                           value={settings.libSizeCapMB}
                           onChange={(e) => update({ libSizeCapMB: +e.target.value })} />
                    <span style={{ color: "var(--fg-3)", fontSize: 12 }}>MB</span>
                  </div>
                </Row>
              </>
            )}

            {pane === "thumbnails" && (
              <>
                <Row label={L.settings_thumb_style}>
                  <Seg value={settings.thumbStyle}
                       options={[
                         { value: "iso",    label: L.settings_thumb_iso },
                         { value: "wire",   label: L.settings_thumb_wire },
                         { value: "normal", label: L.settings_thumb_normal },
                       ]}
                       onChange={(v) => update({ thumbStyle: v })} />
                </Row>
                <Row label={L.settings_thumb_lighting}>
                  <Seg value={settings.thumbLighting}
                       options={[
                         { value: "studio", label: L.settings_lighting_studio },
                         { value: "even",   label: L.settings_lighting_even },
                         { value: "rim",    label: L.settings_lighting_rim },
                       ]}
                       onChange={(v) => update({ thumbLighting: v })} />
                </Row>
                <Row label={L.settings_thumb_aa}>
                  <Seg value={settings.thumbAA}
                       options={[
                         { value: "off",     label: "Off" },
                         { value: "msaa2x",  label: "MSAA 2×" },
                         { value: "msaa4x",  label: "MSAA 4×" },
                         { value: "msaa8x",  label: "MSAA 8×" },
                       ]}
                       onChange={(v) => update({ thumbAA: v })} />
                </Row>
                <Row label={L.settings_thumb_render_size} hint={L.settings_thumb_render_size_hint}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <input type="range" min="256" max="1024" step="128"
                           value={settings.thumbRenderSize}
                           onChange={(e) => update({ thumbRenderSize: +e.target.value })}
                           style={{ width: 160 }} />
                    <span className="settings-num-readout">{settings.thumbRenderSize} px</span>
                  </div>
                </Row>
                <Row label={L.settings_thumb_size}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <input type="range" min="1" max="10" step="1"
                           value={settings.thumbCacheGB}
                           onChange={(e) => update({ thumbCacheGB: +e.target.value })}
                           style={{ width: 160 }} />
                    <span className="settings-num-readout">{settings.thumbCacheGB} GB</span>
                  </div>
                </Row>
                <Row label={L.settings_cache_used}>
                  <div className="settings-meter">
                    <div className="settings-meter-bar"><div style={{ width: "58%" }} /></div>
                    <div className="settings-meter-text">
                      <span>1.16 GB / {settings.thumbCacheGB} GB</span>
                      <span style={{ color: "var(--fg-3)" }}>34 × 512px</span>
                    </div>
                  </div>
                </Row>
                <Row>
                  <div style={{ display: "flex", gap: 8 }}>
                    <button className="settings-action">
                      <Icon name="refresh" size={12} /> {L.settings_thumb_regen}
                    </button>
                    <button className="settings-action">
                      <Icon name="close" size={12} stroke={2} /> {L.settings_thumb_clear}
                    </button>
                  </div>
                </Row>
              </>
            )}

            {pane === "slicer" && (
              <>
                <Row label={L.settings_slicer_default}>
                  <Seg value={settings.slicer}
                       options={[
                         { value: "orca",  label: L.settings_slicer_orca },
                         { value: "bambu", label: L.settings_slicer_bambu },
                         { value: "prusa", label: L.settings_slicer_prusa },
                       ]}
                       onChange={(v) => update({ slicer: v })} />
                </Row>
                <Row label={L.settings_slicer_path}>
                  <div style={{ display: "flex", gap: 6, alignItems: "center", flex: 1 }}>
                    <input className="settings-text"
                           placeholder="/Applications/OrcaSlicer.app"
                           value={settings.slicerPath}
                           onChange={(e) => update({ slicerPath: e.target.value })} />
                    <button className="settings-action"
                            style={{ flexShrink: 0 }}>
                      <Icon name="folder" size={12} /> {L.settings_browse}
                    </button>
                  </div>
                </Row>
                <Row label={L.settings_default_profile} hint={L.settings_default_profile_hint}>
                  <input className="settings-text" style={{ maxWidth: 280 }}
                         value={settings.defaultProfile}
                         onChange={(e) => update({ defaultProfile: e.target.value })} />
                </Row>
                <Row label={L.settings_post_export}>
                  <Seg value={settings.postExport}
                       options={[
                         { value: "open",   label: L.settings_post_open },
                         { value: "reveal", label: L.settings_post_reveal },
                         { value: "none",   label: L.settings_post_none },
                       ]}
                       onChange={(v) => update({ postExport: v })} />
                </Row>
              </>
            )}

            {pane === "hotkeys" && (
              <div className="settings-hotkeys">
                {[
                  { group: L.hk_group_global, items: [
                    { label: L.hk_search,        keys: ["⌘", "F"] },
                    { label: L.hk_settings,      keys: ["⌘", ","] },
                    { label: L.hk_new_folder,    keys: ["⌘", "⇧", "N"] },
                    { label: L.hk_import,        keys: ["⌘", "O"] },
                    { label: L.hk_quick_action,  keys: ["⌘", "K"] },
                  ]},
                  { group: L.hk_group_view, items: [
                    { label: L.hk_view_grid,     keys: ["⌘", "1"] },
                    { label: L.hk_view_masonry,  keys: ["⌘", "2"] },
                    { label: L.hk_view_list,     keys: ["⌘", "3"] },
                    { label: L.hk_toggle_detail, keys: ["⌘", "I"] },
                    { label: L.hk_density_up,    keys: ["⌘", "+"] },
                    { label: L.hk_density_down,  keys: ["⌘", "-"] },
                  ]},
                  { group: L.hk_group_model, items: [
                    { label: L.hk_open_slicer,   keys: ["⌘", "⏎"] },
                    { label: L.hk_reveal,        keys: ["⌘", "⇧", "R"] },
                    { label: L.hk_fav,           keys: ["F"] },
                    { label: L.hk_tag,           keys: ["T"] },
                    { label: L.hk_rename,        keys: ["⏎"] },
                    { label: L.hk_delete,        keys: ["⌫"] },
                  ]},
                ].map(g => (
                  <div key={g.group} className="hk-group">
                    <div className="hk-group-title">{g.group}</div>
                    {g.items.map(it => (
                      <div key={it.label} className="hk-row">
                        <div className="hk-label">{it.label}</div>
                        <div className="hk-keys">
                          {it.keys.map((k, i) => <kbd key={i}>{k}</kbd>)}
                        </div>
                      </div>
                    ))}
                  </div>
                ))}
              </div>
            )}

            {pane === "advanced" && (
              <>
                <Row label={L.settings_adv_workers}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <input type="range" min="1" max="16" step="1"
                           value={settings.workers}
                           onChange={(e) => update({ workers: +e.target.value })}
                           style={{ width: 160 }} />
                    <span className="settings-num-readout">{settings.workers}</span>
                  </div>
                </Row>
                <Row label={L.settings_adv_gpu} hint={L.settings_adv_gpu_hint}>
                  <SettingsToggle on={settings.gpu} onChange={(v) => update({ gpu: v })} />
                </Row>
                <Row label={L.settings_cache_location}>
                  <input className="settings-text"
                         value={settings.cacheLocation}
                         onChange={(e) => update({ cacheLocation: e.target.value })} />
                </Row>
                <Row label={L.settings_log_level}>
                  <select className="settings-text" style={{ maxWidth: 160 }}
                          value={settings.logLevel}
                          onChange={(e) => update({ logLevel: e.target.value })}>
                    <option value="error">error</option>
                    <option value="warn">warn</option>
                    <option value="info">info</option>
                    <option value="debug">debug</option>
                    <option value="trace">trace</option>
                  </select>
                </Row>
                <Row label={L.settings_dev_menu}>
                  <SettingsToggle on={settings.devMenu} onChange={(v) => update({ devMenu: v })} />
                </Row>
                <Row label={L.settings_adv_telemetry}
                     hint={L.settings_adv_telemetry_hint}>
                  <SettingsToggle on={settings.telemetry} onChange={(v) => update({ telemetry: v })} />
                </Row>
                <Row>
                  <div style={{ display: "flex", gap: 8 }}>
                    <button className="settings-action">
                      <Icon name="folder" size={12} /> {L.settings_open_logs}
                    </button>
                    <button className="settings-action danger">{L.settings_adv_reset}</button>
                  </div>
                </Row>
              </>
            )}

            {pane === "about" && (
              <div className="settings-about">
                <window.AppIcon size={96} />
                <div className="settings-about-name">ModelRack</div>
                <div className="settings-about-tag">A workshop tool for your 3D model library</div>
                <div className="settings-about-grid">
                  <div className="k">{L.settings_about_version}</div><div className="v">v0.1.0 (alpha)</div>
                  <div className="k">{L.settings_about_build}</div><div className="v" style={{fontFamily:"var(--mono)"}}>2026.05.06+a3f291</div>
                  <div className="k">{L.settings_about_repo}</div><div className="v" style={{fontFamily:"var(--mono)"}}>github.com/modelrack/modelrack</div>
                  <div className="k">{L.settings_about_license}</div><div className="v">MIT</div>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function SettingsToggle({ on, onChange }) {
  return (
    <button className={"settings-toggle " + (on ? "on" : "")}
            onClick={() => onChange(!on)}>
      <span className="knob" />
    </button>
  );
}

// ─────────────────────────────────────────────────────────
// App
// ─────────────────────────────────────────────────────────
function App() {
  const [t, setTweak] = useTweaks(TWEAK_DEFAULTS);
  const [settings, updateSettings] = useSettings();
  const [settingsOpen, setSettingsOpen] = useState(false);

  const [view, setView] = useState("grid");
  const [density, setDensity] = useState("m");
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState({ kind: "smart", value: "all" });
  const [selectedId, setSelectedId] = useState(1);
  const [scanState] = useState({ scanning: false, current: "", found: 36, skipped: 1 });

  // Tweaks panel mirrors settings (theme + lang)
  useEffect(() => {
    if (t.theme !== settings.theme) updateSettings({ theme: t.theme });
  }, [t.theme]);
  useEffect(() => {
    if (t.lang !== settings.lang) updateSettings({ lang: t.lang });
  }, [t.lang]);

  // Resolve theme: dark | light | auto
  const resolvedTheme = useMemo(() => {
    if (settings.theme === "auto") {
      return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
    }
    return settings.theme;
  }, [settings.theme]);

  useEffect(() => {
    document.documentElement.dataset.theme = resolvedTheme;
    document.documentElement.dataset.accent = settings.accent;
  }, [resolvedTheme, settings.accent]);

  // Cmd+, opens settings
  useEffect(() => {
    const onKey = (e) => {
      if ((e.metaKey || e.ctrlKey) && e.key === ",") {
        e.preventDefault();
        setSettingsOpen(true);
      } else if (e.key === "Escape" && settingsOpen) {
        setSettingsOpen(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [settingsOpen]);

  const L = (window.I18N[settings.lang] || window.I18N.en);

  const models = window.MODELS;

  const filtered = useMemo(() => {
    let out = models;
    if (filter.kind === "smart") {
      if (filter.value === "fav") out = out.filter(m => m.fav);
      else if (filter.value === "printed") out = out.filter(m => m.printed > 0);
      else if (filter.value === "ready") out = out.filter(m => m.status === "ready" || m.status === "queued");
      else if (filter.value === "errors") out = out.filter(m => m.status === "error");
      else if (filter.value === "recent") out = out.slice(0, 12);
      else if (filter.value === "dups") out = out.filter(m => m.id === 19 || m.id === 20);
    } else if (filter.kind === "folder") {
      out = out.filter(m => m.folder === filter.value || m.folder.startsWith(filter.value + "/"));
    } else if (filter.kind === "tag") {
      out = out.filter(m => m.tags.includes(filter.value));
    }
    if (search) {
      const q = search.toLowerCase();
      out = out.filter(m => m.name.toLowerCase().includes(q) ||
                            m.tags.some(t => t.toLowerCase().includes(q)) ||
                            m.folder.toLowerCase().includes(q));
    }
    return out;
  }, [filter, search, models]);

  const selectedModel = filtered.find(m => m.id === selectedId) || models.find(m => m.id === selectedId);

  // Density → grid min size
  const gridMin = density === "s" ? 130 : density === "l" ? 220 : 168;

  return (
    <div className="stage">
      <div className="window">
        <div className="titlebar">
          <div className="traffic">
            <span className="light close" />
            <span className="light min" />
            <span className="light max" />
          </div>
          <div className="titlebar-mark"><window.AppMark size={16} /></div>
          <div className="titlebar-title">ModelRack <span className="sep">—</span> <span className="path">{L.titlebar_path}</span></div>
          <div className="titlebar-spacer" />
          <button className="titlebar-btn" title={L.cmd_settings}
                  onClick={() => setSettingsOpen(true)}>
            <Icon name="sparkle" size={13} />
          </button>
        </div>

        <div className="app">
          <Sidebar filter={filter} setFilter={setFilter} L={L} />

          <div className="content">
            <Toolbar view={view} setView={setView}
                     sort="name-asc" setSort={() => {}}
                     search={search} setSearch={setSearch}
                     density={density} setDensity={setDensity} L={L} />
            <FilterBar filter={filter} setFilter={setFilter}
                       search={search} setSearch={setSearch}
                       count={filtered.length} L={L} />
            <div className="grid-wrap">
              {view === "grid" && (
                <div className="grid" style={{
                  gridTemplateColumns: `repeat(auto-fill, minmax(${gridMin}px, 1fr))`,
                }}>
                  {filtered.map(m => (
                    <Card key={m.id} model={m}
                          selected={m.id === selectedId}
                          onSelect={setSelectedId}
                          density={density} />
                  ))}
                </div>
              )}
              {view === "masonry" && (
                <div className="masonry" style={{ columnWidth: `${gridMin}px` }}>
                  {filtered.map(m => {
                    // deterministic aspect ratio per model, between 0.75 and 1.45
                    const r = (((m.id * 2654435761) >>> 0) % 1000) / 1000;
                    const aspect = 0.75 + r * 0.7;
                    return (
                      <div className="masonry-item" key={m.id}>
                        <Card model={m}
                              selected={m.id === selectedId}
                              onSelect={setSelectedId}
                              density={density}
                              aspect={aspect} />
                      </div>
                    );
                  })}
                </div>
              )}
              {view === "list" && (
                <ListView models={filtered} selectedId={selectedId} onSelect={setSelectedId} />
              )}
            </div>
            <StatusBar count={filtered.length} total={36}
                       selectedId={selectedId} scanState={scanState} L={L} />
          </div>

          <DetailPanel model={selectedModel} L={L} settings={settings} updateSettings={updateSettings} />
        </div>
      </div>

      <SettingsDialog open={settingsOpen}
                      onClose={() => setSettingsOpen(false)}
                      settings={settings}
                      update={updateSettings}
                      L={L} />

      <TweaksPanel>
        <TweakSection label="Appearance">
          <TweakRadio label="Theme" value={t.theme}
                      options={["dark", "light"]}
                      onChange={(v) => setTweak("theme", v)} />
          <TweakRadio label="Lang" value={t.lang}
                      options={["en", "ko", "ja"]}
                      onChange={(v) => setTweak("lang", v)} />
        </TweakSection>
      </TweaksPanel>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<App />);
