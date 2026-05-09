# Window Chrome QA Gate

This checklist is the P0b gate before enforcing visual pixel thresholds.

## Source of truth

- The active UI shell is Slint (`src/main.rs` enters `slint_shell::run()`).
- Startup capture geometry is requested through the winit window bridge at 1480×920 with a 960×640 minimum.
- Native frame decorations are disabled because the Slint shell draws the visible titlebar and traffic-light controls; packaged smoke captures must not show a second macOS titlebar above it.
- The custom green traffic-light callback uses winit maximize/restore for frameless windows, with native macOS `zoom:` only as a fallback when the Slint window handle is unavailable; it must not enter a separate full-screen Space.
- Native full-screen remains available through the macOS View menu path.

## Manual packaged-app smoke

Run against `build/ModelRack.app`, not only `cargo run`:

1. Launch app and confirm the first visible window is 1480×920 or record the actual captured geometry in visual QA metadata.
2. Drag custom titlebar; the window should move without selecting sidebar/grid content.
3. Red traffic light hides the app; Dock icon restores the window.
4. Yellow traffic light minimizes; Dock/window restore returns to the same app state.
5. Green traffic light maximizes through the winit window state; pressing it again restores the previous size. It must not enter a separate full-screen Space.
6. Run `scripts/capture-smoke.sh`; verify it emits a screenshot path plus the current artifact `report.json`.

## Pixel-threshold rule

Do not enforce P1 pixel thresholds until this gate is one of:

- **fixed:** geometry and traffic lights are deterministic;
- **masked:** native chrome/shadow variance is represented in the visual QA mask list;
- **tracked:** any remaining green maximize/restore backend defect remains a known TODO-15 follow-up and is excluded from pixel scoring with explicit evidence.
