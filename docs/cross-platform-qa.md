# Cross-Platform QA Evidence

Current evidence bundle: `.omx/artifacts/cross-platform-qa/20260509/evidence.json`.

## Gate status

| Platform | Status | Release implication |
| --- | --- | --- |
| macOS | Collected locally | Usable for current packaged-app smoke and Rust behavior tests. |
| Windows | Missing external runner | Release blocker until a Windows bundle is attached. |
| Linux | Missing external runner | Release blocker until a Linux bundle is attached. |

## macOS evidence collected in this session

- **Launch / window capture:** `scripts/capture-smoke.sh` launches `build/ModelRack.app`, activates `modelrack`, captures the frontmost window rectangle, and writes a visual QA `report.json`.
- **Backend / scale:** current capture report records macOS host metadata and viewport size; latest clean report: `.omx/artifacts/visual-qa/modelrack-smoke-20260509T004921Z/current/report.json`.
- **Visual artifact:** reference/current/diff artifact exists at `.omx/artifacts/visual-qa/ultragoal-g004-final/`; diff currently fails because outer-window/shadow and live-library content are not masked/normalized yet.
- **Folder picker:** code path uses `rfd::FileDialog::pick_folder` and compiles; native interactive picker automation was not collected in this session.
- **Sidecar write/read:** covered by Rust tests for favorite, metadata, tags, printed count, print history, and scanner sidecar round-trip.
- **Slicer launch:** covered by launcher tests for system default opener, configured executable, missing-path preflight, helper failures, and macOS `.app` bundle launch arguments.
- **Thumbnail / preview smoke:** current Slint isometric thumbnail/detail preview surfaces are visible in smoke images; no independent wgpu backend smoke exists in the current repo state.

## Required external evidence bundles

Windows and Linux evidence should attach the same checklist as the macOS bundle:

1. app launch screenshot plus backend/scale metadata;
2. visual QA artifact directory with `reference/`, `current/`, `diff/`, and reports;
3. folder picker open/cancel/select behavior;
4. sidecar write/read on a real STL folder;
5. slicer default/configured launch behavior;
6. GPU thumbnail/detail-preview smoke for the active backend.

Until those bundles exist, cross-platform QA remains a tracked release blocker rather than a code blocker for local macOS stabilization.
