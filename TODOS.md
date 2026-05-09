# TODOS

## Active

### Pending manual QA: Sidebar tag drag/drop
**Added:** 2026-05-10
**State:** Implemented locally, manual QA deferred
**What:** Drag a model card or list row onto an existing `TAGS` sidebar row to add that tag to the model. If the model already has the tag, the operation must no-op without duplicating it.
**Verify later:** Confirm card/list drag starts reliably, tag rows highlight during drag, dropping persists `.modelrack.json` for real library files, duplicate tags are skipped, and status text reports added vs already-present outcomes.
**Current evidence:** Rust persistence path covered by `tag_drop_adds_missing_tag_and_persists_sidecar` and `tag_drop_skips_existing_tag_without_duplication`; full pointer interaction still needs human/manual app QA.

**Current release reality (2026-05-09):** local macOS implementation/polish work is largely complete and warning-clean. TODO-15 macOS traffic-light behavior is accepted complete by latest user verification. Remaining release blockers are external Windows/Linux evidence collection plus final release hygiene. See `docs/release-checklist.md`.

**Status index**

| TODO | State | Release meaning |
| --- | --- | --- |
| 01, 02, 04-11, 13-16 | Done locally | Implemented and verified/accepted in the current local macOS Slint shell. |
| 03, 12 | Blocked externally | Require Windows/Linux runner evidence before public release. |
| 09, 16 | Evidence refreshed locally | Fresh packaged screenshot and masked visual QA artifacts exist for commit `1ad452a`; final tag still needs a clean working tree and any release-owner signoff. |

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

### TODO-03: Cross-platform GPU surface testing 🚧 External release blocker
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

### TODO-11: Thumbnail disk cache ✅
**Added:** 2026-05-07 (via /autopilot)
**Target:** v0.2.0
**What:** Persist generated thumbnails by hash under the platform cache directory and invalidate when file hash changes.
**Why:** Large libraries should not regenerate every thumbnail on each app launch.
**Status:** Implemented for the active Slint shell. Scan results now create deterministic generated PNG thumbnails under the platform cache directory (`thumbnails/v1`) using the scanned file hash as the cache key, so content changes invalidate by writing a new hash-addressed file while unchanged models reuse the existing PNG. Browser cards and the detail preview bind arbitrary cached `slint::Image` thumbnails when available and fall back to the existing SVG `thumb-key` assets if generation/loading fails. The current generator is a CPU wireframe/dimension renderer over scanner mesh/metadata; future wgpu thumbnail rendering can replace the generator without changing the cache contract.

### TODO-12: Cross-platform release QA 🚧 External release blocker
**Added:** 2026-05-07 (via /autopilot)
**Target:** v0.1.0 release gate
**What:** Verify Slint UI launch, wgpu thumbnail/preview rendering, HiDPI scaling, folder picker, sidecar writes, and slicer launch on Windows and Linux.
**Why:** The app is intended to be cross-platform; current local verification is macOS-only.
**Status:** External-only runners still required. Local macOS evidence bundle is recorded in `docs/cross-platform-qa.md` and `.omx/artifacts/cross-platform-qa/20260509/evidence.json`; Windows and Linux entries are marked `missing_external_runner` with required evidence enumerated.

### TODO-13: Slint UI migration ✅
**Added:** 2026-05-07 (via Slint migration direction)
**Target:** v0.2.0 architecture
**What:** Keep the active Slint application shell as the source of truth while preserving reusable Rust domain logic, scanner, sidecar metadata, file watcher, thumbnail cache, slicer integration, and focused thumbnail/preview rendering modules. The original migration target was a Slint 3-pane layout with sidebar, toolbar/search, grid/list/masonry, settings, and detail metadata panels; that local migration scope is complete, with only release evidence tracked separately.
**Why:** The Slint migration direction identifies typography and mixed Korean/English rendering as product-critical. Slint should own the desktop UI shell; wgpu should remain a focused rendering backend for STL preview, orbit viewport, and thumbnail generation surfaces rather than wrapping the whole app.
**Status:** Completed locally for the active Slint shell. Source-of-truth reconciled: `src/main.rs` runs Slint directly and `Cargo.toml` has no alternate egui/default-shell feature gate, so old “feature-gated bridge” language is migration history. Framework-neutral view-model types and filtering/sorting logic live in `src/view_model.rs`. Shared snapshot data covers sidebar counts, folder/tag summaries, browser card rows, browser totals, status text, and sort labels. The Slint shell handles Open Folder, search, view mode, density, sort direction, smart sidebar filters, settings, persisted preferences, restored last library, sidecar-backed favorites/tags/author/notes/printed counts/print history, thumbnail cache binding, slicer discovery/default/manual launch behavior, native macOS wrapper/titlebar/menu semantics, bundled fonts, visual QA masks, and a zero-warning gate. Remaining work belongs to release evidence, not UI-stack migration: TODO-03/TODO-12 Windows/Linux evidence and the checklist in `docs/release-checklist.md`.

### TODO-14: Bundled typography system ✅
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
**Status:** Completed/accepted locally for the intended macOS hybrid chrome. The active Slint shell creates the winit backend with full-size transparent native content, disables Slint's default menu bar, keeps the NSWindow wrapper for native rounded corners/shadow/full-screen Space transitions, hides native standard buttons so only one custom Slint traffic-light set is visible, and routes custom red/yellow/green to hide, minimize, and native `toggleFullScreen:`. Latest user verification on 2026-05-09 accepts the red hide/restore behavior as complete. Automated packaged smoke `.omx/artifacts/window-chrome-qa/window-chrome-20260509T075353Z/window-chrome-report.json` still records an older/script-sensitive red check failure, but TODO-15 is not a current release blocker.

### TODO-16: Finish Slint mockup parity polish ✅
**Added:** 2026-05-08 (Ralph visual parity pass)
**Target:** v0.2.0 Slint shell polish
**What:** Maintain the completed masked ModelRack mockup parity pass for the active Slint shell. The local polish scope covered toolbar/filter/search/count language, slicer picker, visual masks, card density, path compaction, detail metadata alignment, theme elevations, sidebar hover, grid scroll extent, health score, and build-plate fit row.
**Why:** The current app matches the main mockup state at the large structural level and has completed local polish for the known UI gaps. Future work here should be evidence refresh or regression repair, not another architecture pass.
**Status:** Completed locally as a masked visual parity/polish pass. Packaged-app screenshot capture works through `scripts/capture-smoke.sh` with frontmost window bounds, deterministic capture prefs, and visual QA `report.json` output. Toolbar/filter-bar/search/count language, slicer picker/discovery, warning policy, rectangular visual masks, card density/body stack, `~/...` path compaction, mesh-health two-column layout, darker mockup-aligned theme elevations, sidebar hover rows, grid scroll extent, health score pill, and build-plate fit row are implemented. Latest refreshed evidence for commit `1ad452a`: `.omx/artifacts/runtime/modelrack-smoke-20260509T075344Z.png` and `.omx/artifacts/visual-qa/modelrack-smoke-20260509T075344Z/current/report.json` (`failure_reason=none`, non-blank image, `docs/visual-masks/todo16-browser-grid.json` masks applied, no reference path so result is `not_compared`). Latest local gates also passed: `cargo fmt --check`, `python3 scripts/check-warning-baseline.py`, `cargo build`, and `cargo test` (73 tests).
