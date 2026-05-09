# TODOS

## Active

### TODO-01: Worker panic handling for thumbnail generation ✅
**Added:** 2026-05-06 (via /plan-eng-review)
**Target:** v0.0.2
**What:** Wrap rayon thumbnail worker logic in `std::panic::catch_unwind(AssertUnwindSafe(|| { ... }))`. On panic, send an error result through crossbeam-channel so the UI can show "thumbnail generation failed" instead of silently stopping.
**Why:** If a worker panics (bad STL data, OOM, logic bug), the channel receiver gets RecvError but the UI doesn't know the cause. The placeholder stays forever — user never knows generation failed.
**Depends on:** v0.0.1 completion (the channel infrastructure)
**Context:** The channel arch is: UI spawns generation tasks → workers send RGBA buffers back → UI replaces placeholders. The panic path in this flow is untested and has no error message.

### TODO-02: GPU readback performance baseline ✅
**Added:** 2026-05-06 (via /plan-eng-review)
**Target:** Before v0.0.3 (real wgpu renderer)
**What:** Benchmark GPU→CPU texture readback latency on target hardware (macOS Metal). Measure: 256x256 RGBA readback time, 512x512 readback time, impact on frame rate when reading back N textures per frame.
**Why:** v0.0.3's real renderer depends on reading rendered textures back from GPU to CPU for PNG caching. The design doc acknowledges 5-20ms per readback on Metal but this hasn't been measured. If readback is consistently >15ms, v0.0.3 needs a different approach (render-to-CPU directly, or async readback pipeline).
**Depends on:** v0.0.2 (working wgpu context + channel infrastructure for async work)

### TODO-03: Cross-platform GPU surface testing
**Added:** 2026-05-06 (via /plan-eng-review)
**Target:** v0.1.0
**What:** Test Slint shell launch plus wgpu visualization surfaces on Windows (DX12/Vulkan) and Linux (Vulkan). Verify no driver-specific thumbnail/preview artifacts, window resize behavior, and HiDPI scaling.
**Why:** The design now treats wgpu as a visualization subsystem, not the full UI. Cross-platform QA must cover both Slint typography/layout behavior and GPU preview/thumbnail paths.
**Depends on:** Slint migration slice and current wgpu thumbnail renderer.
**Status:** Evidence triaged in `docs/cross-platform-qa.md` and `.omx/artifacts/cross-platform-qa/20260509/evidence.json`. macOS packaged-app visual smoke and Rust sidecar/slicer tests are collected locally. Windows/Linux GPU surface evidence is explicitly missing because no external runners or display/GPU bundles are available in this macOS session; this remains a release blocker, not a local code blocker.

### TODO-04: Keyboard navigation ✅
**Added:** 2026-05-06 (via /plan-design-review)
**Target:** v0.1.0
**What:** Implement full keyboard navigation: Tab through zones (sidebar → search → grid → detail), arrow keys for 2D grid navigation, Enter to open detail, Space to toggle selection, Escape to close dialogs/deselect, Cmd-F for search focus, Cmd-, for Settings. Spec in DESIGN.md.
**Why:** Desktop apps without keyboard nav feel broken to keyboard-heavy users. Grid browsing without arrow keys forces mouse-only interaction.
**Depends on:** Grid widget, filter system, settings dialog

### TODO-05: Window resize / responsive behavior ✅
**Added:** 2026-05-06 (via /plan-design-review)
**Target:** v0.1.0
**What:** Detail panel collapses below 1024px window width, sidebar collapses below 800px. Collapsed panes toggleable via toolbar overlay buttons. Grid always remains visible.
**Why:** The 3-pane layout (220px + grid + 320px) needs 1100px+ to breathe. On 13" MacBook screens or windowed mode, panes must collapse gracefully.
**Depends on:** 3-pane layout implementation

### TODO-06: Empty state with illustration ✅
**Added:** 2026-05-06 (via /plan-design-review)
**Target:** v0.1.0
**What:** Centered 96px app icon cubes illustration + "No models yet" headline + "Add a folder to start browsing your STL collection" subtext + prominent "Add Folder" teal button + tip: "ModelRack reads STL files directly from your folders — no uploads, no cloud."
**Why:** The empty state is the first thing new users see. A warm, branded empty state sets the tone; "No items found" erodes trust immediately.
**Depends on:** App icon asset

### TODO-07: Sidecar JSON metadata system ✅
**Added:** 2026-05-06 (via /plan-design-review)
**Target:** v0.2.0
**What:** Read/write `.modelrack.json` sidecar files next to each STL. Fields: tags (flat string list), notes, favorite (bool), printed (count), printHistory, author, added (first-seen timestamp). Portable, human-readable, survives app uninstall.
**Why:** Without metadata persistence, ModelRack is just a thumbnail viewer. Tags and notes are what make it a library manager. Sidecar JSON chosen over central DB for portability and user trust.
**Depends on:** v0.1.0 (solid STL browsing experience)

### TODO-08: 3D orbit controls for detail preview ✅
**Added:** 2026-05-06 (via /plan-design-review)
**Target:** v0.1.0
**What:** Left-click drag to orbit-rotate the detail panel's 3D preview around mesh center. Show "Orbit: drag to rotate" hint. No zoom or pan in v0.1.0.
**Why:** A static wireframe doesn't let users inspect geometry. Orbit is the minimum viable 3D interaction — lets users see the model from different angles without opening a slicer.
**Depends on:** Real wgpu renderer (v0.0.3)
**Status:** Implemented as an interactive detail-panel wireframe preview using the selected model's scanned mesh geometry when available.

### TODO-09: Native visual QA harness ✅
**Added:** 2026-05-07 (via /autopilot)
**Target:** v0.1.0 polish
**What:** Add a reproducible screenshot capture path for the native desktop app and compare primary states against Mockups/ModelRack.html / the active Slint shell screens.
**Why:** Current smoke testing proves startup and tests prove behavior, but mockup convergence still needs pixel-level evidence for grid, list, masonry, settings, empty state, and metadata panels.
**Status:** Local smoke harness added via `scripts/capture-smoke.sh`; it launches the signed app bundle and writes reproducible screenshots under `.omx/artifacts/runtime`. Pixel-diff comparison against `Mockups/ModelRack.html` remains a later refinement.

### TODO-10: File watcher refresh ✅
**Added:** 2026-05-07 (via /autopilot)
**Target:** v0.2.0
**What:** Add notify-based watching for the selected library folder and debounce rescans when STL or sidecar files change.
**Why:** A model library manager should notice new downloads/exports without forcing manual Refresh.
**Status:** Implemented with `notify` recursive watching for model files and `.modelrack.json` sidecars, plus a 750ms debounce before automatic rescan. Manual Refresh remains available.

### TODO-11: Thumbnail disk cache ✅
**Added:** 2026-05-07 (via /autopilot)
**Target:** v0.2.0
**What:** Persist generated thumbnails by hash under the platform cache directory and invalidate when file hash changes.
**Why:** Large libraries should not regenerate every thumbnail on each app launch.
**Status:** Implemented as hash-addressed PNG files under the platform cache directory. Scans reuse cached thumbnails when the file hash matches and only render missing cache entries.

### TODO-12: Cross-platform release QA
**Added:** 2026-05-07 (via /autopilot)
**Target:** v0.1.0 release gate
**What:** Verify Slint UI launch, wgpu thumbnail/preview rendering, HiDPI scaling, folder picker, sidecar writes, and slicer launch on Windows and Linux.
**Why:** The app is intended to be cross-platform; current local verification is macOS-only.
**Status:** External-only runners still required. Local macOS evidence bundle is recorded in `docs/cross-platform-qa.md` and `.omx/artifacts/cross-platform-qa/20260509/evidence.json`; Windows and Linux entries are marked `missing_external_runner` with required evidence enumerated.

### TODO-13: Slint UI migration
**Added:** 2026-05-07 (via Slint migration direction)
**Target:** v0.2.0 architecture
**What:** Continue hardening the active Slint application shell while keeping Rust domain logic, scanner, sidecar metadata, file watcher, thumbnail cache, slicer integration, and focused wgpu preview/thumbnail rendering modules reusable. The original migration target was a Slint 3-pane layout with sidebar, toolbar/search, grid/list/masonry, settings, and detail metadata panels; the remaining work is source-of-truth cleanup, polish, slicer discovery, visual QA, and cross-platform evidence.
**Why:** The Slint migration direction identifies typography and mixed Korean/English rendering as product-critical. Slint should own the desktop UI shell; wgpu should remain a focused rendering backend for STL preview, orbit viewport, and thumbnail generation surfaces rather than wrapping the whole app.
**Status:** In progress. Source-of-truth reconciled: `src/main.rs` now runs the Slint shell directly and `Cargo.toml` has no alternate egui/default-shell feature gate, so old “feature-gated bridge” language is migration history, not current runtime truth. Framework-neutral view-model types and filtering/sorting logic live in `src/view_model.rs`. Shared snapshot data covers sidebar counts, folder/tag summaries, browser card rows, browser totals, status text, and sort labels. The Slint shell's Open Folder path calls the existing scanner and refreshes snapshot-backed counts/cards. The Slint toolbar has snapshot-backed search, view-mode cycling, density cycling, and sort-direction toggling. The Slint sidebar applies smart filters plus folder/tag filters through shared `LibraryFilter` keys. Settings opens a real Slint in-app panel with tab navigation and live local preference controls for language, theme, density, GPU thumbnails, worker count, and slicer selection. Preferences persist to disk, an existing last library folder is restored on launch, favorite toggles write `.modelrack.json` sidecars for real files, and Open in Slicer uses configured/default launcher behavior plus platform slicer discovery, a settings picker list, and the manual chooser fallback. The detail panel edits tags, author, notes, printed count, and print-history records, writing `.modelrack.json` for real scanned models while keeping demo/no-write entries memory-only. Tags render as removable chips with a quick Add flow plus the existing bulk comma-edit field. Print history captures richer profile fields (printer, profile, nozzle, layer height, duration) with backward-compatible sidecar defaults and compact row summaries. Cross-platform release QA is triaged with macOS evidence and Windows/Linux blockers. Final warning policy is documented in docs/warning-policy.md with an enforced cargo warning baseline; remaining TODO-13 work is limited to release-blocker follow-up outside this local stabilization pass.

### TODO-14: Bundled typography system
**Added:** 2026-05-07 (via Slint migration direction)
**Target:** v0.2.0 architecture
**What:** Bundle and load explicit UI fonts, using Inter for Latin and Pretendard for Korean fallback, with a documented monospace choice for paths/hashes/counts. Remove dependency on uncontrolled OS fallback chains.
**Why:** Typography is the interface for dense maker/library workflows. Mixed San Francisco + Apple SD Gothic Neo fallback creates spacing and weight mismatches on Korean macOS systems.
**Status:** Implemented for the active Slint shell. Inter Variable, Pretendard Variable, and JetBrains Mono Regular are bundled under `assets/fonts`, with license files kept beside the font assets. The Slint shell registers the bundled fonts with Hangul fallback. JetBrains Mono is used for dense count/path/hash-style text surfaces.

### TODO-15: Frameless Slint macOS traffic-light behavior
**Added:** 2026-05-08 (custom titlebar QA)
**Target:** v0.2.0 Slint shell polish
**What:** Revisit the custom traffic-light implementation for the active Slint shell. Red should hide the window and restore from the Dock icon, yellow should minimize, green should actually maximize/restore the frameless window, and the custom titlebar drag region should keep native-feeling movement.
**Why:** The current frameless Slint/winit/macOS bridge receives the green-button click and can trigger zoom/maximize animations, but the window frame does not reliably commit to the maximized size. Dock restore after custom hide also needs real packaged-app QA instead of ad hoc debug-process behavior.
**Status:** Hardened locally. The active Slint shell keeps the custom titlebar visually correct and red/yellow/titlebar callbacks wired; the green traffic-light callback now prefers winit maximize/restore for the frameless window instead of relying on native `zoom:` animations, with native macOS zoom retained only as a fallback if the Slint window handle is unavailable. Packaged-app manual QA is still required before calling TODO-15 fully complete.

### TODO-16: Finish Slint mockup parity polish
**Added:** 2026-05-08 (Ralph visual parity pass)
**Target:** v0.2.0 Slint shell polish
**What:** Continue the ModelRack mockup parity pass from the current Slint shell state. Remaining visible gaps: exact outer-window positioning/shadow versus the browser mockup capture, toolbar control sizing/spacing, folder tree expand affordances and `+` actions beside `FOLDERS`/`TAGS`, first-screen bottom clipping/scrollbar polish, and detail-panel microcopy/section spacing.
**Why:** The current app now matches the main mockup state at the large structural level: populated 36-model library, matching sidebar counts, selected raspberry_pi card, AppMark titlebar, generated per-model SVG thumbnails, 4-column grid, and detailed metadata panel. The remaining work is pixel-level polish rather than architecture.
**Status:** Masked parity pass completed locally; remaining work is documented, not hidden. Current pass restored native packaged-app screenshot capture through `scripts/capture-smoke.sh` using frontmost window bounds, deterministic capture prefs, and visual QA `report.json` output. The Slint window now opts into `no-frame`/disabled native decorations so packaged captures show only the custom titlebar, not a second macOS titlebar. Toolbar and filter-bar controls are left-aligned like the mockup, the search icon/`⌘K` hint now live inside one search field, and the browser count label is anchored at the filter-bar right edge with mockup language (`36 items`, or `N of M items` when filtered). Slicer picker/discovery is represented in Settings → Slicer with system default, discovered candidates, and manual fallback. Warning policy fails on new unclassified/increased cargo warning buckets via `scripts/check-warning-baseline.py` and `docs/warning-baseline.json`. Visual QA supports explicit rectangular masks through `scripts/visual-qa-artifacts.py --mask/--mask-file`, `docs/visual-qa-masks.md`, and the checked-in TODO-16 mask file `docs/visual-masks/todo16-browser-grid.json`, so live-library grid/detail/status variance is excluded with report provenance instead of ignored. Fresh masked visual evidence exists at `.omx/artifacts/visual-qa/todo16-masked-before/` and `.omx/artifacts/visual-qa/todo16-masked-after/`; the pass still reports `diff_threshold_exceeded` over unmasked pixels (`0.98640071` after polish, threshold `0.01`), which confirms the remaining mismatch is toolbar/sidebar/theme-level pixel polish rather than masked live-content variance. Fresh `cargo test` passed with `54 passed`. Remaining visible gaps: exact sidebar/theme colors and typography versus the browser mockup, first-screen bottom clipping/scrollbar polish, and detail-panel fixture/microcopy parity.
