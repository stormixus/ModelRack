# ModelRack — Design System Reference

Extracted from `Mockups/design.md` on 2026-05-06.
Canonical reference for implementers. The mockup document remains the narrative/rationale doc.

## Stack Direction Notice

The Slint migration direction in `docs/slint-migration.md` supersedes the original
full-application `egui + wgpu` implementation assumption. The visual/component rules in
this file remain valid, but the target UI framework is now Slint. wgpu should be treated
as the STL preview, thumbnail rendering, and mesh-visualization subsystem only.

Typography quality is a product requirement. Future implementation should bundle and
own its font stack instead of relying on uncontrolled OS fallback chains.

## Color Tokens

All colors use `oklch()`. Dark theme is the default; light theme inverts luminance.

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

Accent variants (Settings → Appearance): teal (default), violet, orange, green.

## Typography

| Role | Font | Sizes |
|---|---|---|
| UI | Inter | 10.5 (overlines), 11.5–12 (meta), 12.5 (body), 13 (titles), 14 (settings titles) |
| Display | Inter | 22 (about-screen name) |
| Mono | JetBrains Mono | paths, hashes, file sizes, dates, numbers |

CJK fallback chain: `Pretendard → Apple SD Gothic Neo → Noto Sans KR → Hiragino Kaku Gothic ProN → Yu Gothic → Noto Sans JP`

## Corner Radius Tiers

| Element | Radius |
|---|---|
| Cards, buttons, chips | 6px |
| Dialogs | 8px |
| Window | 12px |

## Elevation

Background-color steps (`--bg-0` → `--bg-5`), not box-shadows.
No gradients on functional surfaces.

## Borders

Hairline 1px borders throughout, using `oklch()` tokens.

## Icons

Single-stroke 16×16 lucide-style, inline SVG, `currentColor`.
Stroke widths: 1.5 (default), 2 (emphasized: chevrons, plus, close).

## Spacing

- Sidebar: 220px (resizable)
- Detail panel: 320px (resizable)
- Grid card: 168px min width (density S/M/L: 130/168/220)
- Filter chips: inline, 4px gap
- Section dividers: 1px hairline

## Layout — 3-Pane

```
┌─ Titlebar ─────────────────────────────────────────────┐
│ ●●●  ▣ ModelRack — ~/Library/3d              ⚙        │
├──────────┬────────────────────────────────┬───────────┤
│ Sidebar  │ Toolbar (search, view, sort)   │ Detail    │
│ 220px    ├────────────────────────────────┤ 320px     │
│          │ Filter chip bar                │           │
│ Smart    ├────────────────────────────────┤ Preview   │
│ Filters  │                                │           │
│ (primary)│ Grid / Masonry / List          │ Geometry  │
│          │                                │ Tags      │
│ Folders  │                                │ History   │
│          │                                │ Notes     │
│ Tags     │                                │ File      │
│          ├────────────────────────────────┤           │
│          │ Status bar                     │           │
└──────────┴────────────────────────────────┴───────────┘
```

Window resize behavior:
- < 1024px: detail panel collapses (toggleable overlay)
- < 800px: sidebar collapses (toggleable overlay)
- Grid always remains

## View Modes

1. **Grid** — CSS Grid `auto-fill, minmax(168px, 1fr)`, uniform aspect-1 cards
2. **Masonry** — CSS multi-column, `break-inside: avoid`, per-model aspect via `id * 2654435761`
3. **List** — table rows: 40px thumb, name, size, triangles, format, print count

Density: S=130, M=168, L=220 px card widths.

## Component Patterns

### Thumbnail Card
- Dark bg (`bg-2`), 6px radius, square aspect-1
- 3D wireframe preview (isometric, dark bg, light lines)
- Filename (truncated, tooltip for full path), size + tris in mono
- Selected: `--accent-line` border + 1px outer ring + `bg-5` fill
- Hover: lighter bg
- Error: red checkerboard + "ERR" badge

### List Row
- 40px thumbnail icon, metadata columns in mono
- Selected: `--accent-dim` background

### Sidebar Items
- Smart filters: primary weight (larger type, counts in mono)
- Folder tree: indent + chevron
- Tags: colored dots
- Selected: left vertical accent bar (2px, inset 6px) + tinted bg

### Filter Chips
- Colored pills with × close
- Search + filter compose orthogonally
- Active filter visible as chip in toolbar area

### Detail Panel Sections (top to bottom)
1. Preview (3D wireframe + axis gizmo + orbit controls hint)
2. Title block (filename + monospace path)
3. Actions (Open in Slicer primary teal, Mark Printed, Favorite toggle)
4. Geometry (Format, Triangles, Dimensions, Volume, File size)
5. Tags (colored pills + `+ add` ghost pill)
6. Print History (vertical timeline, green/red dots)
7. Notes (`contentEditable`, monospace placeholder)
8. File (b3 hash truncated, modified, added, author — in mono)

Section titles: 11px uppercase, 0.08em tracking, color `--fg-2`.
Section dividers: 1px hairlines.

## Interaction States

| State | Spec |
|---|---|
| Empty (no folders) | Centered 96px app icon cubes + "No models yet" + "Add Folder" teal button + tip text |
| Scanning | Pulse dot (teal, 8px, pulsing opacity) + running counters "34 STL files found · 2 skipped" |
| Library full | 36 sample models, mixed CJK/ASCII names |
| Filter active | Chip bar reflects state |
| Search active | Orthogonal to filter, both render as chips |
| Selection | Card: accent border + ring + bg fill. List: accent-dim bg |
| Parse error | Card: red checkerboard + ERR badge. Detail: "Unparseable" |
| Settings dialog | 760×540 modal, 2-column, backdrop blur, 7 tabs |
| Light theme | Toggle in Settings → Appearance |

## Keyboard Navigation

- Tab: cycle through sidebar → search → grid → detail
- Arrow keys: navigate grid (2D, wraps within row)
- Enter: open detail for selected model
- Space: toggle selection
- Escape: close dialog / deselect
- Cmd-F: focus search
- Cmd-,: open Settings
- Cmd-click: disjoint multi-select
- Shift-click: contiguous range select

## Data Model — Sidecar

Metadata stored as JSON sidecar files: `my_model.stl.modelrack.json`
Portable, human-readable, survives app uninstall.

Fields: tags (flat string list), notes, favorite, printed (count), printHistory, author, added (first-seen timestamp).

## i18n

Languages: English, Korean, Japanese.
Flat string objects, no plural forms.
Date formatting locale-aware.
Font fallback chain critical for CJK legibility.
Folder names are paths, not UI strings — stay in original language.

## Settings Dialog

760×540 modal, 2-column (sidebar tabs + body), backdrop blur.
Close: backdrop click, ✕, Escape.
Tabs: General, Appearance, Library, Thumbnails, Slicer, Advanced, About.
State persisted per user.
