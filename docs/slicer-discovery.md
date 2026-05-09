# Slicer Discovery

ModelRack keeps the existing safe launcher contract:

- an empty slicer path uses the platform default `.stl` opener;
- a configured executable/app path is persisted in preferences and launched directly;
- macOS `.app` bundles are opened with `open -a <bundle> <model>`.

## Candidate discovery

The Settings → Slicer panel shows a small picker list:

1. **System default STL opener** — always available and selected when `slicer_path` is empty.
2. **Discovered slicers** — narrow, deterministic candidates found in well-known platform locations.
3. **Manual selection** — preserved when the saved path does not match a discovered candidate.

Current platform search surface:

- macOS: `/Applications` and `~/Applications` for `OrcaSlicer.app`, `BambuStudio.app`, `PrusaSlicer.app`, `UltiMaker Cura.app`, `Cura.app`, `SuperSlicer.app`, and `ideaMaker.app`.
- Windows: common Program Files / LocalAppData install roots for the same narrow slicer family.
- Linux/Unix: executable names on `PATH` (`orca-slicer`, `bambu-studio`, `prusa-slicer`, `cura`, `superslicer`).

The manual **Choose Slicer** button remains as fallback for custom installs and uncommon slicers.

## Verification

- `cargo test slicer_` covers discovered/manual/default picker rows and macOS app-bundle discovery.
- Existing launcher tests continue to verify default opener behavior, configured executable behavior, and macOS `.app` bundle launch arguments.
