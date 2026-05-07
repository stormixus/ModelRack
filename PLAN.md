# ModelRack — Project Plan

## Goal

Desktop-native 3D model management application for:

- organizing STL/3MF/STEP files
- visual browsing
- tagging/searching
- print history tracking
- quick re-print workflows
- slicer integration

Primary target users:

- heavy 3D printing users
- homelab/maker users
- people with large unorganized STL collections

---

# Core Philosophy

> "Plex for 3D print files"
>
> +  
> "Asset manager for makers"

Not just a file browser.

The app should answer:

- “Where is that Raspberry Pi mount?”
- “Did I already print this?”
- “Which version worked best?”
- “What filament/settings did I use?”

---

# Tech Stack

## Core

| Layer | Tech |
|---|---|
| Language | Rust |
| GUI | Slint |
| DB | SQLite |
| ORM | rusqlite |
| GPU Renderer | wgpu |
| 3D Viewer | custom wgpu viewer embedded as visualization surface |
| File Watcher | notify |
| File Scanner | walkdir |
| Hashing | blake3 |
| Image/Thumbnail | image |
| STL Parser | stl_io |
| Config | serde + toml |

## UI Stack Direction

The Slint migration plan in `docs/slint-migration.md` supersedes the original
full-application `egui + wgpu` direction.

ModelRack is a dense desktop workshop tool where typography is part of the interface:
filenames, mixed Korean/English metadata, tags, print history, path text, and compact rows
must render consistently. The long-term UI layer should therefore be Slint, with bundled
fonts and explicit typography ownership.

Current egui/eframe code is a working prototype shell. It should be treated as a bridge,
not the final UI architecture. New UI-heavy work should be planned for Slint unless it is
strictly needed to keep the prototype usable.

Rendering ownership:

- Slint owns menus, sidebar, toolbar, settings, metadata panels, search, filenames, tags,
  dialogs, and print-history UI.
- wgpu owns STL preview viewports, thumbnail rendering, offscreen rendering, orbit camera,
  and future mesh inspection surfaces.
- Rust remains the application foundation and owns scanner, sidecar metadata, slicer launch,
  thumbnail cache, file watching, and domain logic.

Typography direction:

- Bundle UI fonts instead of relying on uncontrolled OS fallback chains.
- Prefer Inter for Latin UI text and Pretendard for Korean fallback.
- Keep monospace values for paths, hashes, sizes, counts, and timestamps.

---

# Project Structure

```text
modelrack/
├── Cargo.toml
├── assets/
├── data/
│   ├── thumbnails/
│   ├── previews/
│   └── modelrack.db
├── src/
│   ├── main.rs
│   ├── app.rs
│   ├── config.rs
│   ├── db/
│   ├── scanner/
│   ├── viewer/
│   ├── thumbnail/
│   ├── library/
│   ├── slicer/
│   ├── tags/
│   ├── search/
│   └── ui/
└── docs/
```

---

# MVP Features

# Phase 1 — Foundation

## File Library

- add library folders
- recursive scanning
- supported formats:
  - STL
  - 3MF
  - OBJ
  - STEP
- automatic rescanning
- duplicate detection using hash

## Database

Store:

- file path
- hash
- file size
- format
- created date
- modified date
- thumbnail path
- tags
- notes

---

# Phase 2 — Visual Browser

## Grid View

- thumbnail grid
- adjustable thumbnail size
- fast scrolling
- virtualized rendering

## Preview Panel

Show:

- 3D preview
- metadata
- dimensions
- triangle count
- tags
- notes

---

# Phase 3 — Tagging System

## Tags

Examples:

```text
rackmount
raspberry-pi
mini-pc
server
homelab
poe
gmktec
bambulab
snapmaker
functional
printed
favorite
failed
```

## Smart Filters

- printed only
- favorites
- recent
- duplicates
- unsupported
- no-thumbnail

---

# Phase 4 — Print Tracking

## Print History

Track:

- print date
- printer used
- filament
- nozzle
- success/failure
- notes
- photos

## Quick Actions

- mark as printed
- reprint
- open in slicer

---

# Phase 5 — Slicer Integration

## Integrations

Support:

- OrcaSlicer
- Bambu Studio
- PrusaSlicer

## Features

- open model directly
- open grouped files
- open project files
- send multiple parts

---

# Phase 6 — Advanced Features

## 3MF Metadata Extraction

Extract:

- plate info
- filament settings
- printer profile
- thumbnails
- print time estimates

## Folder Watching

Real-time updates using notify.

## Search Engine

Search by:

- filename
- tags
- notes
- dimensions
- metadata

---

# Database Schema Draft

## models

```sql
id
filename
filepath
hash
format
filesize
thumbnail_path
notes
favorite
created_at
updated_at
```

## tags

```sql
id
name
color
```

## model_tags

```sql
model_id
tag_id
```

## print_history

```sql
id
model_id
printer
filament
success
notes
printed_at
```

---

# UI Layout

```text
┌─────────────────────────────────────┐
│ Sidebar │ Toolbar                  │
│          ├──────────────────────────┤
│ Tags     │ Thumbnail Grid           │
│ Filters  │                          │
│ Library  │                          │
│          │                          │
├──────────┴──────────────────────────┤
│ Metadata / Preview / History Panel │
└─────────────────────────────────────┘
```

---

# Performance Goals

## Requirements

- handle 100k+ files
- instant search
- low memory usage
- smooth scrolling
- async thumbnail generation

## Strategies

- SQLite indexes
- thumbnail caching
- background workers
- lazy loading
- virtualized grids

---

# Future Ideas

## Potential Features

- cloud sync
- mobile companion app
- makerworld integration
- printables integration
- AI auto-tagging
- similarity search
- duplicate geometry detection
- filament inventory
- printer fleet management
- print queue

---

# Development Roadmap

## Milestone 1

Library scanning + DB

## Milestone 2

Thumbnail browser

## Milestone 3

3D preview

## Milestone 4

Tagging/search

## Milestone 5

Print history

## Milestone 6

Slicer integration

## Milestone 7

Polish + packaging

---

# Packaging Targets

## Platforms

- macOS
- Windows
- Linux

## Distribution

- GitHub Releases
- Homebrew
- winget
- AppImage

---

# Initial Cargo Crates

```toml
slint
wgpu
notify
walkdir
rusqlite
serde
serde_json
toml
blake3
image
stl_io
rfd
rayon
tokio
```

---

# Design Decisions (from /plan-design-review 2026-05-06)

Reference: `Mockups/design.md` (narrative + rationale), `DESIGN.md` (canonical tokens + components).
Stack direction: `docs/slint-migration.md` (Slint UI + wgpu visualization subsystem).

Key decisions:
- **3-pane layout**: sidebar 220px + grid + detail 320px, resizable
- **Sidebar hierarchy**: smart filters primary weight, folders/tags secondary
- **Incremental rollout**: toolbar (search + view toggle + filter chips) ships v0.0.3, sidebar v0.0.4
- **Metadata storage**: sidecar JSON (`my_model.stl.modelrack.json`), portable, human-readable
- **Multi-select**: Cmd-click disjoint, Shift-click contiguous range
- **Tag taxonomy**: flat string list
- **3D preview**: orbit-only for v0.1.0 (drag to rotate, no zoom/pan)
- **Empty state**: centered illustration + "Add Folder" CTA
- **Scanning UX**: pulse dot + running counters, no progress bar
- **Onboarding**: subtle dismissible tip banner, no guided tour
- **Keyboard nav**: full spec in DESIGN.md (Tab, arrows, Enter, Escape, Cmd shortcuts)
- **Window resize**: collapse detail panel < 1024px, sidebar < 800px

## Approved Mockups

| Screen | Mockup | Direction | Notes |
|--------|--------|-----------|-------|
| Main grid | `Mockups/design.md` | Dark workshop tool, 3-pane, teal accent, Inter + CJK | Build toolbar first (v0.0.3), sidebar second (v0.0.4) |

# Long-Term Vision

> The definitive desktop library manager for makers and 3D printing enthusiasts.

Not just storage.

A complete operational memory system for physical fabrication.
