# Release Checklist

Current target: **v0.2.0 local stabilization → external release QA**.
Last updated: 2026-05-09.

## Current release reality

ModelRack is locally implementation-complete for the active macOS Slint shell, with a zero-warning Rust baseline. The remaining release blockers are evidence and packaging gates, not another UI-stack rewrite.

## Local gates before tagging

Run from the repository root and attach output to the release notes or handoff:

- [ ] `git status --short` is clean.
- [ ] `cargo fmt --check` passes.
- [ ] `python3 scripts/check-warning-baseline.py` passes with `current=0 baseline=0`.
- [ ] `cargo build` passes.
- [ ] `cargo test` passes.
- [ ] `scripts/capture-smoke.sh` produces a fresh packaged-app screenshot artifact.
- [ ] `scripts/window-chrome-smoke.py` passes or records a current, explicit blocker in `docs/window-chrome-qa.md`.
- [ ] TODO-16 visual QA is refreshed against `docs/visual-masks/todo16-browser-grid.json`; any remaining diff is explained in the report rather than silently ignored.

## macOS product smoke

- [ ] Launch packaged `ModelRack.app`.
- [ ] Native wrapper shows rounded corners/shadow; no duplicate native traffic lights are visible.
- [ ] Custom red hides/restores via Dock/app activation.
- [ ] Custom yellow minimizes/restores.
- [ ] Custom green enters/exits native macOS full-screen.
- [ ] macOS menu bar includes ModelRack Settings, File Open Library, File Close Window, and View Enter Full Screen.
- [ ] Open Folder scans a real model folder.
- [ ] File watcher notices added/changed model or `.modelrack.json` sidecar after debounce.
- [ ] Favorite/tags/author/notes/printed count/print history persist to `.modelrack.json` for real files.
- [ ] Thumbnail cache reuses generated PNGs across refresh/relaunch.
- [ ] Open in Slicer works for system default and configured/discovered slicer paths.

## External release blockers

These cannot be closed from the current macOS-only local session without external runners or machines:

- [ ] Windows Slint launch and screenshot evidence.
- [ ] Windows folder picker, sidecar write/read, slicer launch, HiDPI behavior.
- [ ] Windows thumbnail/detail-preview rendering smoke.
- [ ] Linux Slint launch and screenshot evidence.
- [ ] Linux folder picker, sidecar write/read, slicer launch, HiDPI behavior.
- [ ] Linux thumbnail/detail-preview rendering smoke.

Required evidence shape is documented in `docs/cross-platform-qa.md`.

## Packaging / release hygiene

- [ ] Decide version bump in `Cargo.toml` before tag.
- [ ] Add or create a changelog/release note if publishing outside local builds.
- [ ] Confirm bundled font licenses remain beside `assets/fonts`.
- [ ] Confirm warning baseline remains zero after packaging changes.
- [ ] Commit with Lore trailers including tested/not-tested evidence.
- [ ] Push `main` and tag only after the external blockers above are either passed or explicitly deferred by release owner.

## TODO mapping

- Local implementation completed: TODO-01, TODO-02, TODO-04 through TODO-11, TODO-13 through TODO-16.
- External release blockers: TODO-03 and TODO-12.
- Evidence refresh before tag: TODO-09/TODO-16 visual QA artifacts and `docs/window-chrome-qa.md` smoke.
