# Changelog

## v0.0.3 — macOS maker-library alpha

### Added
- Native macOS app bundle packaging with generated `.icns` app icon assets.
- Localized Korean UI coverage for browser controls, sidebars, detail panels, settings content, status text, and relative times.
- Sort dropdown with name, modified date, added date, file size, format, triangle count, dimensions, mesh health, and print count options.
- Model context menus for rename, slicer launch, Finder reveal, detail view, favorite toggle, print count, and path/name copy actions.
- Sidebar resizing/toggling, renamed model-file flow, folder collapse animation, and tag drag/drop handling.
- Printer/profile-backed print estimate selection and slicer app discovery/selection flows.

### Changed
- Refined STL/3MF preview rendering, smoothing, plate selection, and thumbnail cache behavior.
- Polished light/dark theme surfaces, search field layout, and grouped toolbar controls.
- Reworked the app icon for macOS Dock sizing with more negative space, softer material depth, and no hard vector outline.
- macOS packaging script now supports release builds and reads the version from `Cargo.toml`.

### Known gaps
- Cross-platform Windows/Linux release evidence is still deferred.
- Tag sidebar drag/drop remains marked for manual QA.
- The macOS app bundle is ad-hoc signed, not notarized.
