# Window Chrome QA Gate

This checklist is the P0b gate before enforcing visual pixel thresholds.

## Latest automated packaged smoke — 2026-05-09

Command: `scripts/window-chrome-smoke.py`

Report:
`.omx/artifacts/window-chrome-qa/window-chrome-20260509T075353Z/window-chrome-report.json`

Result: **accepted with script-sensitive note**.

Passed checks:

- `initial_packaged_geometry`
- `native_macos_menu_bar_contains_app_settings`
- `native_standard_traffic_lights_hidden_no_duplicate`
- `custom_titlebar_drag_moves_window`
- `yellow_minimizes_and_restores`
- `green_enters_native_fullscreen_and_restores`
- `capture_smoke_emits_artifacts`
- `native_rounded_window_corners_visible`

Script-sensitive note:

- `red_custom_light_hides_and_activation_restores`
  - Evidence: `hidden_observed=false`, `visible_after_restore=true`
  - Status interpretation: latest user verification accepts red hide/restore as complete; this automation result is kept as historical/script-sensitive evidence, not a current blocker.

## Source of truth

- The active UI shell is Slint (`src/main.rs` enters `slint_shell::run()`).
- Startup capture geometry is requested through the winit window bridge at 1480×920 with a 960×640 minimum.
- Native frame decorations stay enabled so macOS owns the window wrapper, rounded corners, shadow, and full-screen transition.
- Slint draws the visible custom titlebar and traffic-light controls inside the native wrapper; the native standard buttons are hidden.
- The custom green traffic light uses native macOS full-screen (`toggleFullScreen:`), matching the standard green-button behavior for this app.
- Native full-screen remains available through the macOS View menu path.

## Manual packaged-app smoke

Run against `build/ModelRack.app`, not only `cargo run`:

1. Launch app and confirm the first visible window is 1480×920 or record the actual captured geometry in visual QA metadata.
2. Drag the custom Slint titlebar; the native-wrapped window should move without selecting sidebar/grid content.
3. Custom red traffic light hides the app; Dock/app activation restores a usable window.
4. Custom yellow traffic light minimizes; Dock/window restore returns to the same app state.
5. Custom green traffic light enters native macOS full-screen; pressing it again exits full-screen and restores the previous size.
6. The packaged screenshot shows native wrapper rounded corners and shadow from macOS, not a Slint-painted fake frame.
7. Run `scripts/capture-smoke.sh`; verify it emits a screenshot path plus the current artifact `report.json`.

The same checklist can be run as an automated packaged smoke on macOS:

```bash
cargo build
cp target/debug/modelrack build/ModelRack.app/Contents/MacOS/modelrack
codesign --force --deep --sign - build/ModelRack.app
scripts/window-chrome-smoke.py
```

The script launches the packaged app with isolated preferences, drives the custom
traffic-light controls through CoreGraphics mouse events, verifies objective
window state through Accessibility, samples the captured screenshot for visible
rounded corners, and writes
`.omx/artifacts/window-chrome-qa/<run>/window-chrome-report.json`.

## Pixel-threshold rule

Do not enforce P1 pixel thresholds until this gate is one of:

- **fixed:** geometry and traffic lights are deterministic;
- **masked:** native chrome/shadow variance is represented in the visual QA mask list;
- **tracked:** any remaining green full-screen backend defect remains a known TODO-15 follow-up and is excluded from pixel scoring with explicit evidence.
