# ModelRack — Design Document

**Version:** v0.1.0 mockup
**Target platform:** macOS (eframe + egui native app)
**Document status:** living — describes the current hi-fi mockup and the design decisions behind it

---

## 1. Product positioning

ModelRack is a **workshop tool** for managing a 3D model library on disk, not a document app. The closest design references are Blender, OBS Studio, and PrusaSlicer — dense, dark, technical UIs that prioritize density and direct manipulation over whitespace and "delight."

The product is **not**:
- A cloud sync service (files stay on disk)
- A model marketplace
- A slicer (it launches one)
- An organizer with hidden state (no proprietary database, no `~/Library/MyApp/cache.sqlite` users can't delete)

The product **is**:
- A fast, native browser for `~/Library/3d/` (or wherever the user keeps STLs)
- A way to attach metadata (tags, notes, print history) to local files via a sidecar approach
- A 3D-preview hub that hands off to the user's slicer for actual printing

---

## 2. Visual system

### 2.1 Aesthetic direction

**Linear/Arc-style modern dark.** Not Blender-dense, not Apple-minimal — somewhere between. Specifically:
- Hairline 1px borders rendered with `oklch` so they survive theme swaps
- 6px corner radius on most surfaces (cards, buttons, chips); 8px on dialogs; 12px on the window
- No gradients on functional surfaces (only on the app icon and titlebar where it reads as physical depth)
- Subtle elevation via background-color steps (`bg-0` → `bg-5`), not box-shadows
- Type scale topping out at ~22px; most UI sits at 11.5–13px

### 2.2 Color tokens

All colors are defined in `oklch()` so brightness shifts cleanly between dark and light themes via `oklch(from var(...) calc(l ± n) c h)`.

| Token | Dark | Light | Role |
|---|---|---|---|
| `--bg-0` | `oklch(0.13 …)` | `oklch(0.97 …)` | Window background (deepest) |
| `--bg-1` | `oklch(0.16 …)` | `oklch(0.95 …)` | Sidebar, titlebar |
| `--bg-2..5` | step lighter | step darker | Toolbars, cards, hover, selected |
| `--fg-0..3` | `0.92 → 0.42` | `0.20 → 0.70` | Primary → disabled text |
| `--accent` | `oklch(0.74 0.13 230)` (teal) | `oklch(0.55 0.16 230)` | Selection, focus, primary CTA |
| `--accent-dim` | `accent / 0.18` | `accent / 0.10` | Selected backgrounds |
| `--accent-line` | `accent / 0.35` | `accent / 0.30` | Focused borders |
| `--warn` | `oklch(0.78 0.12 75)` | — | Scanning, queued |
| `--error` | `oklch(0.66 0.18 25)` | — | Parse errors |

**Accent variants** (Settings → Appearance → Accent): teal (default), violet, orange, green. Each has light-theme companion values to keep contrast legal.

### 2.3 Typography

- **UI:** Inter, with CJK fallback chain `Pretendard → Apple SD Gothic Neo → Noto Sans KR → Hiragino Kaku Gothic ProN → Yu Gothic → Noto Sans JP`. CJK metrics differ enough that the fallback ordering matters; Pretendard sits first because it harmonizes best with Inter.
- **Mono:** JetBrains Mono — used for paths, hashes, file sizes, dates, anything tabular.
- Sizes: 10.5 (overlines/uppercase labels), 11.5–12 (meta), 12.5 (body), 13 (titles), 14 (settings titles), 22 (about-screen name).

### 2.4 Iconography

Single-stroke 16×16 lucide-style icons drawn inline as React components, sized via wrapper, color via `currentColor`. No icon library dependency. Stroke widths 1.5 (default) and 2 (emphasized — chevrons, plus signs, close buttons).

---

## 3. Layout

### 3.1 Window chrome

A custom dark titlebar that mimics macOS but doesn't ape it. Traffic lights on the left, app mark + title + breadcrumb path in the middle, settings (sparkle icon) button on the right. Titlebar is a drag region except for the settings button.

### 3.2 Three-pane layout

```
┌─ Titlebar ─────────────────────────────────────────────┐
│ ●●●  ▣ ModelRack — ~/Library/3d              ⚙        │
├──────────┬────────────────────────────────┬───────────┤
│ Sidebar  │ Toolbar (search, view, sort)   │ Detail    │
│ 220px    ├────────────────────────────────┤ 320px     │
│          │ Filter chip bar                │           │
│ Library  ├────────────────────────────────┤ Preview   │
│ Folders  │                                │           │
│ Tags     │ Grid / Masonry / List          │ Geometry  │
│          │                                │ Tags      │
│          │                                │ History   │
│          │                                │ Notes     │
│          │                                │ File      │
│          ├────────────────────────────────┤           │
│          │ Status bar                     │           │
└──────────┴────────────────────────────────┴───────────┘
```

- **Sidebar (220px)** — Library smart filters, folder tree (with indent + chevron), tag list with colored dots
- **Center (1fr)** — Toolbar, filter chips, content area, status bar
- **Detail (320px)** — Selected model's preview + metadata, scrollable

The two side panes have fixed widths in the mockup; in the real app both will be resizable with persisted widths.

### 3.3 View modes

Three view modes for the center pane, toggled in the toolbar:

1. **Grid** — uniform aspect-1 cards, CSS Grid with `auto-fill, minmax(168px, 1fr)`
2. **Masonry** — CSS multi-column with `break-inside: avoid`, per-model deterministic aspect ratio (0.75–1.45) via `id * 2654435761` hash. Better for scanning visually-distinct models.
3. **List** — table with thumb-mini, name, size, triangles, format, print count

Density (S/M/L) toggle controls column width: 130 / 168 / 220 px.

---

## 4. Interaction patterns

### 4.1 Selection

Single selection via click. Selected card gets `--accent-line` border + 1px outer ring and `bg-5` fill. The detail pane updates immediately. List view uses a tinted row background (`--accent-dim`) instead of a border.

The sidebar's smart filters and folder/tag entries use the same selection treatment — left vertical accent bar (2px wide, inset by 6px) plus tinted background.

### 4.2 Filtering

Filters are a single source of truth: `{ kind: "smart" | "folder" | "tag", value: string }`. The sidebar sets it; the filter chip bar reflects it; clicking the chip's × resets to `{ kind: "smart", value: "all" }`.

Search composes orthogonally: filter narrows the set, search narrows further. Both can be active simultaneously and both render as chips.

### 4.3 Settings

Settings open via:
- ⌘, (Cmd-comma) — standard macOS convention
- Sparkle button in the titlebar
- (Future) menu bar → ModelRack → Preferences

Modal dialog: 760×540, two-column (sidebar + body), centered, with backdrop blur. Closes on backdrop click, ✕ button, or Escape. State persists to `localStorage` keyed `modelrack.settings`.

Tabs: General, Appearance, Library, Thumbnails, Slicer, Advanced, About. The About tab shows the full app icon at 96px.

### 4.4 Tweaks panel

A floating panel (separate from Settings) for live design tweaking — currently exposes Theme and Language. Theme/lang changes propagate to Settings so Settings remains the canonical store. Hidden when the host disables tweak mode.

---

## 5. Data model (mockup)

The mockup treats models as flat records with these fields:

```js
{
  id: number,
  name: string,             // filename including extension
  folder: string,           // relative to library root
  size: number,             // bytes
  tris: number | null,      // null when unparseable
  dims: [w, h, d] | null,   // mm
  type: "Binary" | "ASCII" | "Unknown",
  tags: string[],
  printed: number,          // count
  fav: boolean,
  status: "ready" | "queued" | "printed" | "error",
  thumb: string,            // identifier into the SVG thumbnail bank
}
```

**Derived fields** (computed deterministically from `id`, not stored):
- `author` — picked from a 7-entry pool (mix of "You", platform handles, CJK names)
- `added`, `modified` dates — hash-seeded within a 18-month window relative to a fixed "now" (May 6, 2026)
- Print history — reconstructed when `printed > 0`, with printer/filament/date

In the real app, these will become real fields backed by:
- `name`, `folder`, `size`, `modified` — from filesystem
- `tris`, `dims`, `type` — from STL parser
- `tags`, `notes`, `printed`, `printHistory`, `fav`, `author` — from sidecar `.modelrack.json` or app DB
- `added` — first-seen timestamp from app DB
- Thumbnails — from STL renderer cached in `~/Library/Caches/modelrack/`

---

## 6. Internationalization

Three languages bundled: English, Korean, Japanese. Strings live in `i18n.js` as `window.I18N[lang]`. Each language is a flat object — no nesting, no plural forms (English uses "items" / Korean uses "개" without inflection). Functions handle interpolation:

```js
filter_count: (n) => `${n} items`        // en
filter_count: (n) => `${n}개`            // ko
filter_count: (n) => `${n}件`            // ja
```

Date formatting is locale-aware via small helpers (`fmtDateShort`, `fmtRelative`) that branch on lang. Korean uses `2026.05.06`, Japanese uses `2026/05/06`, English uses `May 6, 2026`. Relative dates ("3w ago" / "3주 전" / "3週前") render alongside absolutes in the detail pane.

The CJK font fallback chain in CSS is critical — Inter has no Korean or Japanese glyphs, so without fallback the text shows tofu. Order matters: Pretendard (modern, harmonizes with Inter) → system Korean fonts → system Japanese fonts.

Filter strings (chip labels, search placeholder) all flow through `L.*`. The folder tree uses literal folder names — folders called `한국어_프로젝트` stay in Korean regardless of UI language, because they're paths, not UI strings.

---

## 7. App icon

A squircle (macOS Big Sur+ rounded square, ~22.37% radius) containing three isometric cubes — two on the bottom, one stacked centered on top — over a deep teal-ink gradient background. The motif visualizes the product name: "rack" of "models." The two-tone face shading (top bright, left mid, right dark) reads as 3D even at 16×16, which matters because the smallest size used is the 16px titlebar mark.

Two components:
- `<AppIcon size={N} />` — full squircle with background, used in About and (eventually) Dock
- `<AppMark size={N} />` — bare cubes only, no background, used in titlebar; pulls accent color from CSS vars so it matches the user's chosen accent

---

## 8. Detail panel sections

In order, top to bottom:

1. **Preview** — 3D thumbnail with axis indicator (X/Y/Z colored gizmo), rotate + fullscreen controls, "orbit · drag to rotate" hint
2. **Title block** — filename + monospace path
3. **Actions** — primary "Open in Slicer" + icon-only Mark Printed + Favorite toggle
4. **Geometry** — Format · Triangles · Dimensions · Volume estimate · File size
5. **Tags** — colored pill list with `+ add` ghost pill at the end
6. **Print History** — vertical timeline with date column + status dot (green ok, red fail) + printer + filament + optional failure note
7. **Notes** — `contentEditable` block, monospace placeholder when empty
8. **File** — Hash (b3, truncated mono) · Modified · Added · Author

Sections are separated by 1px hairlines; section titles are 11px uppercase tracked at 0.08em, color `--fg-2`.

---

## 9. States to mock (mockup coverage)

| State | Status | Notes |
|---|---|---|
| Empty (no folders added) | not yet | Will be a centered illustration + "Add Folder" CTA |
| Scanning in progress | partial | Status bar shows pulse dot + "Scanning {file}…" + counts |
| Library full | ✓ | 36 sample models across mixed CJK/ASCII names |
| Filter active | ✓ | Chip bar reflects state |
| Search active | ✓ | Live filter |
| Selection / detail | ✓ | Click any card |
| Parse error | ✓ | Card shows red ERR badge; detail says "Unparseable" |
| Settings dialog | ✓ | All 7 panes |
| Light theme | ✓ | Toggle in Tweaks or Settings |
| Korean / Japanese UI | ✓ | Toggle in Settings → General |

---

## 10. Open questions

- **Drag-and-drop import** — should users drop STL files onto the window? Or only "Add Folder" via dialog?
- **Multi-select** — Cmd-click extend? Shift-click range? Affects bulk-tag, bulk-delete UX.
- **3D preview controls** — orbit only, or also pan/zoom? Persist camera per model or reset per-open?
- **Sidecar format** — JSON next to STL (`my_model.stl.modelrack.json`) vs central DB? Sidecar is more "files on disk" but pollutes the folder; central DB is cleaner but loses portability.
- **Print history capture** — manual ("Mark printed" button) or auto via slicer integration / Bambu/Prusa cloud?
- **Tag taxonomy** — flat string list or hierarchical (e.g. `printer/upgrades`, `printer/spool`)?

---

## 11. Out of scope (for v0.1.0)

- 3D editing / mesh repair — link out to slicer
- Cloud sync / cross-device library
- STEP, 3MF, OBJ formats — STL only in Phase 1
- Print queue management — display history, don't manage queue
- User accounts, sharing, public libraries
