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
| GUI | egui + eframe |
| DB | SQLite |
| ORM | rusqlite |
| Renderer | wgpu |
| 3D Viewer | custom viewer or egui_wgpu |
| File Watcher | notify |
| File Scanner | walkdir |
| Hashing | blake3 |
| Image/Thumbnail | image |
| STL Parser | stl_io |
| Config | serde + toml |

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
eframe
egui
egui_extras
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

# Long-Term Vision

> The definitive desktop library manager for makers and 3D printing enthusiasts.

Not just storage.

A complete operational memory system for physical fabrication.
