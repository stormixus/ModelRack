# Slint Migration Plan

This document is the current UI stack direction.

## Decision

ModelRack should migrate text-heavy application UI from the current egui prototype shell
to Slint. wgpu remains in the project as a rendering subsystem for STL previews,
thumbnail rendering, offscreen rendering, and future mesh inspection tools.

## Why

ModelRack users spend most of their time scanning compact UI: filenames, tags, paths,
print history, geometry values, and mixed Korean/English metadata. Typography quality is
therefore part of the product contract, not decoration. The current full-app egui shell
can be kept as a working bridge, but it should not be the long-term UI layer.

## Migration Slices

1. **Extract app model boundary**
   - Keep scanner, metadata sidecars, file watching, thumbnail cache, slicer launch, and
     wgpu renderer independent from UI widgets.
   - Create simple view-model structs for sidebar counts, model rows/cards, detail panel
     values, scan progress, and settings.

2. **Introduce a minimal Slint shell**
   - Rebuild the titlebar/body/status layout.
   - Use bundled fonts: Inter for Latin, Pretendard for Korean fallback.
     - Assets live in `assets/fonts/InterVariable.ttf` and `assets/fonts/PretendardVariable.ttf`.
     - Monospace surfaces use `assets/fonts/JetBrainsMono-Regular.ttf`.
     - Slint registers both fonts at launch and appends Pretendard as the Hangul fallback.
     - The egui bridge loads the same bundled fonts until the shell migration is complete.
   - Keep current egui app runnable until the Slint shell reaches feature parity.

3. **Port primary panes**
   - Sidebar smart filters/folders/tags.
   - Toolbar search, view mode, density, sort, folder actions.
   - Grid/list/masonry browser.
   - Detail metadata panel and settings dialog.

4. **Embed visualization surfaces**
   - Reuse the existing wgpu offscreen thumbnail renderer.
   - Add a Slint-compatible preview surface or image bridge for the selected-model preview.
   - Keep wgpu isolated from text-heavy UI.

5. **Retire egui shell**
   - Remove eframe/egui dependencies after Slint reaches functional parity.
   - Preserve native smoke screenshots and add pixel comparison on top of the harness.

## Non-goals

- Do not rewrite scanner, sidecar metadata, slicer integration, or thumbnail cache just to
  change UI frameworks.
- Do not adopt a web UI layer.
- Do not expand wgpu back into a full-application UI renderer.

## Current Bridge State

The current app uses egui/eframe as a prototype shell with `Glow`, while `wgpu` is already
limited to Metal/WGSL thumbnail rendering. This is a useful interim state, but all new
UI-heavy work should be weighed against the Slint migration path.
