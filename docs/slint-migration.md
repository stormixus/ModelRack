# Slint Migration Plan

This document is the current UI stack direction and reflects the repository state as of v0.0.3.

## Decision

ModelRack uses Slint as the active desktop UI shell. The Rust domain logic remains
framework-neutral where practical, and wgpu remains a focused visualization subsystem
for STL previews, thumbnail rendering, offscreen rendering, and future mesh inspection
tools.

## Current Source of Truth

- `src/main.rs` unconditionally enters `slint_shell::run()`.
- `Cargo.toml` declares Slint directly and does not expose an alternate egui/default-shell feature gate.
- Historical egui/eframe prototype-shell language should be treated as migration history, not the current runtime contract.
- New UI work should target the Slint shell first. Domain logic should stay reusable through `src/view_model.rs`, `src/scanner.rs`, and focused integration helpers.

## Why

ModelRack users spend most of their time scanning compact UI: filenames, tags, paths,
print history, geometry values, and mixed Korean/English metadata. Typography quality is
therefore part of the product contract, not decoration. Slint owns the text-heavy desktop
shell; wgpu should not expand back into a full-application UI renderer.

## Migration Slices

1. **Extract app model boundary** ✅
   - Scanner, metadata sidecars, file watching, thumbnail cache, slicer launch, and
     visualization code remain independent from Slint widgets where practical.
   - `src/view_model.rs` provides shared app snapshot, filtering, sorting, settings,
     sidebar, browser card, and detail-facing data structures.

2. **Make Slint the active shell** ✅
   - `src/main.rs` runs the Slint shell directly.
   - The shell rebuilds the titlebar/body/status layout in `ui/modelrack.slint`.
   - Bundled fonts are registered at launch: Inter for Latin, Pretendard for Korean
     fallback, and JetBrains Mono for path/hash/count surfaces.

3. **Port primary panes** ✅ / continuing polish
   - Sidebar smart filters/folders/tags are Slint-backed.
   - Toolbar search, view mode, density, sort direction, and folder actions are Slint-backed.
   - Grid/browser cards, detail metadata editing, print-history editing, and settings panel
     are Slint-backed.
   - Remaining work is polish and small feature completion, not a shell-selection decision:
     slicer candidate discovery/dropdown, visual QA evidence, and cross-platform QA.

4. **Embed visualization surfaces** in progress
   - Keep wgpu isolated from text-heavy UI.
   - Reuse focused thumbnail/preview rendering paths for STL visualization.
   - Validate GPU thumbnail/preview behavior through visual QA and cross-platform release QA.

5. **Retire stale migration assumptions** in progress
   - Do not plan new work around an egui/default-shell bridge unless a future decision
     explicitly reintroduces one.
   - Preserve native smoke screenshots and add deterministic pixel comparison on top of the
     visual QA artifact harness.

## Non-goals

- Do not rewrite scanner, sidecar metadata, slicer integration, thumbnail cache, or file
  watching just to change UI documentation.
- Do not adopt a web UI layer.
- Do not expand wgpu back into a full-application UI renderer.
- Do not revive stale egui/feature-gated assumptions without a new ADR.

## Remaining Follow-ups

- Deterministic visual QA artifacts with nonblank reference/current images, diffs, and
  report metadata.
- macOS frameless titlebar/traffic-light behavior verification and green maximize/restore
  resolution or explicit masking/deferral.
- Slicer candidate discovery/dropdown while preserving current manual/default launcher behavior.
- Windows/Linux release QA evidence for Slint launch, wgpu preview/thumbnail paths, HiDPI,
  folder picker, sidecar writes, and slicer launch.
- Warning baseline and release-gate triage.
