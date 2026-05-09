# Release Checklist

Current target: **v0.2.0 local stabilization → external release QA**.
Last updated: 2026-05-09.

## Current release reality

ModelRack is locally implementation-complete for the active macOS Slint shell, with a zero-warning Rust baseline. TODO-15 macOS traffic-light behavior is accepted complete by latest user verification. The remaining release blockers are external Windows/Linux evidence and final packaging/tag hygiene.

## Latest local evidence refresh — 2026-05-09

Refreshed from commit `1ad452a62648e0c9f525fde69390523b7a3b2039` after rebuilding and re-signing `build/ModelRack.app`.

Passed:

- `cargo fmt --check`
- `python3 scripts/check-warning-baseline.py` → `warning baseline ok: current=0 baseline=0`
- `cargo build`
- `cargo test` → 73 passed
- `scripts/capture-smoke.sh` with `MODELRACK_VISUAL_MASK_FILE=docs/visual-masks/todo16-browser-grid.json`
  - Screenshot: `.omx/artifacts/runtime/modelrack-smoke-20260509T075344Z.png`
  - Visual report: `.omx/artifacts/visual-qa/modelrack-smoke-20260509T075344Z/current/report.json`
  - Result: `failure_reason=none`, non-blank image, masks applied, `not_compared` because no reference path was supplied.

Script-sensitive historical note:

- `scripts/window-chrome-smoke.py`
  - Report: `.omx/artifacts/window-chrome-qa/window-chrome-20260509T075353Z/window-chrome-report.json`
  - Overall result: `failed` in automation, but latest user verification accepts red hide/restore as complete.
  - Passed checks: initial packaged geometry, native macOS menu bar, no duplicate native traffic lights, custom titlebar drag, yellow minimize/restore, green native full-screen restore, capture-smoke artifact emission, native rounded corners.
  - Script-sensitive failed check: `red_custom_light_hides_and_activation_restores` (`hidden_observed=false`, `visible_after_restore=true`). Not treated as a current release blocker after user signoff.

## Local gates before tagging

Run from the repository root and attach output to the release notes or handoff:

- [ ] `git status --short` is clean. Latest command was clean before this documentation refresh; re-run after committing these docs.
- [x] `cargo fmt --check` passes.
- [x] `python3 scripts/check-warning-baseline.py` passes with `current=0 baseline=0`.
- [x] `cargo build` passes.
- [x] `cargo test` passes.
- [x] `scripts/capture-smoke.sh` produces a fresh packaged-app screenshot artifact.
- [x] TODO-16 visual QA is refreshed against `docs/visual-masks/todo16-browser-grid.json`; current report is non-blank and mask-applied, but not pixel-compared because no reference path was supplied.
- [x] `scripts/window-chrome-smoke.py` local chrome evidence is recorded. Automation still has a script-sensitive red-check failure, but latest user verification accepts red hide/restore as complete.

## macOS product smoke

- [ ] Launch packaged `ModelRack.app`.
- [x] Native wrapper shows rounded corners/shadow; no duplicate native traffic lights are visible.
- [x] Custom red hides/restores via Dock/app activation. Accepted complete by latest user verification.
- [x] Custom yellow minimizes/restores.
- [x] Custom green enters/exits native macOS full-screen.
- [x] macOS menu bar includes ModelRack Settings, File Open Library, File Close Window, and View Enter Full Screen.
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
- [x] Confirm bundled font licenses remain beside `assets/fonts`.
- [x] Confirm warning baseline remains zero after packaging changes.
- [ ] Commit with Lore trailers including tested/not-tested evidence.
- [ ] Push `main` and tag only after the external blockers above are either passed or explicitly deferred by release owner.

## TODO mapping

- Local implementation completed: TODO-01, TODO-02, TODO-04 through TODO-11, TODO-13 through TODO-16.
- External release blockers: TODO-03 and TODO-12.
- Evidence refreshed locally: TODO-09/TODO-16 visual QA artifacts for commit `1ad452a`.
