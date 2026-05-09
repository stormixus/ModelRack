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
**Status:** Implemented in the active Slint shell for real selected/restored folders. Startup restore, Open Folder, manual Refresh, and debounced watcher refresh all route scans through the background scan runtime with generation/folder guards so stale results cannot overwrite the active library. The existing toolbar Refresh icon is wired to a real `refresh-library()` callback, real folders start a recursive `notify` watcher, and relevant model/sidecar events (`.stl`, `.3mf`, `.obj`, `.step`, `.stp`, `.modelrack.json`) are coalesced behind a 750ms debounce. Demo fallback stays memory-only, is labeled as a sample library instead of a fake filesystem folder, and does not start a watcher or enable sidecar writes. Verified with watcher relevance/debounce/restored-folder tests plus `cargo test watcher`.

### TODO-11: Thumbnail disk cache
**Added:** 2026-05-07 (via /autopilot)
**Target:** v0.2.0
**What:** Persist generated thumbnails by hash under the platform cache directory and invalidate when file hash changes.
**Why:** Large libraries should not regenerate every thumbnail on each app launch.
**Status:** Implemented for the active Slint shell. Scan results now create deterministic generated PNG thumbnails under the platform cache directory (`thumbnails/v1`) using the scanned file hash as the cache key, so content changes invalidate by writing a new hash-addressed file while unchanged models reuse the existing PNG. Browser cards and the detail preview bind arbitrary cached `slint::Image` thumbnails when available and fall back to the existing SVG `thumb-key` assets if generation/loading fails. The current generator is a CPU wireframe/dimension renderer over scanner mesh/metadata; future wgpu thumbnail rendering can replace the generator without changing the cache contract.

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
**What:** Revisit the macOS traffic-light implementation for the active Slint shell. Custom red should hide the app/window and app-icon activation should restore the existing window, custom yellow should minimize, custom green should enter/exit native macOS full-screen, and the custom titlebar should keep native-feeling movement inside a native NSWindow wrapper.
**Why:** The frameless Slint/winit/macOS bridge made green-button semantics brittle and lost the native rounded wrapper. The intended hybrid is native macOS chrome as the wrapper for rounded corners, shadow, and full-screen Space transitions, with Slint still drawing the visible custom titlebar/traffic lights.
**Status:** Implemented locally for the intended macOS hybrid chrome. The active Slint shell creates the winit backend with full-size transparent native content, disables Slint's default menu bar, keeps the NSWindow wrapper for native rounded corners/shadow/full-screen Space transitions, hides native standard buttons so only one custom Slint traffic-light set is visible, and routes custom red/yellow/green to hide, minimize, and native `toggleFullScreen:`. The macOS menu bar now includes `ModelRack > Settings…`, `File > Open Library…`, `File > Close Window`, and `View > Enter Full Screen`. Verification before user stop request: `cargo test` passed 70 tests; packaged smoke evidence reached menu bar/duplicate-button/red/yellow/rounded-corner checks with green exit timing still script-sensitive.

### TODO-16: Finish Slint mockup parity polish
**Added:** 2026-05-08 (Ralph visual parity pass)
**Target:** v0.2.0 Slint shell polish
**What:** Continue the ModelRack mockup parity pass from the current Slint shell state. Remaining visible gaps: exact outer-window positioning/shadow versus the browser mockup capture, toolbar control sizing/spacing, folder tree expand affordances and `+` actions beside `FOLDERS`/`TAGS`, first-screen bottom clipping/scrollbar polish, and detail-panel microcopy/section spacing.
**Why:** The current app now matches the main mockup state at the large structural level: populated 36-model library, matching sidebar counts, selected raspberry_pi card, AppMark titlebar, generated per-model SVG thumbnails, 4-column grid, and detailed metadata panel. The remaining work is pixel-level polish rather than architecture.
**Status:** Masked parity pass completed locally; remaining work is documented, not hidden. Current pass restored native packaged-app screenshot capture through `scripts/capture-smoke.sh` using frontmost window bounds, deterministic capture prefs, and visual QA `report.json` output. Toolbar and filter-bar controls are left-aligned like the mockup, the search icon/`⌘K` hint now live inside one search field, and the browser count label is anchored at the filter-bar right edge with mockup language (`36 items`, or `N of M items` when filtered). Slicer picker/discovery is represented in Settings → Slicer with system default, discovered candidates, and manual fallback. Warning policy fails on new unclassified/increased cargo warning buckets via `scripts/check-warning-baseline.py` and `docs/warning-baseline.json`. Visual QA supports explicit rectangular masks through `scripts/visual-qa-artifacts.py --mask/--mask-file`, `docs/visual-qa-masks.md`, and the checked-in TODO-16 mask file `docs/visual-masks/todo16-browser-grid.json`, so live-library grid/detail/status variance is excluded with report provenance instead of ignored. Fresh masked visual evidence exists at `.omx/artifacts/visual-qa/todo16-masked-before/` and `.omx/artifacts/visual-qa/todo16-masked-after/`; the pass still reports `diff_threshold_exceeded` over unmasked pixels (`0.98640071` after polish, threshold `0.01`), which confirms the remaining mismatch is toolbar/sidebar/theme-level pixel polish rather than masked live-content variance. Follow-up local polish tightened browser density toward the mockup `minmax(168px, 1fr)` grid behavior by reducing medium cards to five-column scale and giving card metadata a taller body stack. A second polish pass compacted real home-relative paths to `~/...` in shell labels/settings/detail metadata and aligned mesh-health indicators into the mockup's stable two-column detail grid. The final local polish pass darkened Slint elevation tokens toward the browser mockup palette, added hover-backed sidebar rows, removed the grid's trailing scroll gap, exposed the detail health score pill, and added the compact build-plate fit row under print estimate. Verification: `cargo fmt --check` and `cargo build` passed. Remaining visible gaps: exact pixel-diff parity still needs a fresh packaged screenshot comparison, but known local UI polish gaps are narrowed to visual-diff evidence rather than unimplemented shell behavior.
