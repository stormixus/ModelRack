# TODOS

## Active

### TODO-01: Worker panic handling for thumbnail generation
**Added:** 2026-05-06 (via /plan-eng-review)
**Target:** v0.0.2
**What:** Wrap rayon thumbnail worker logic in `std::panic::catch_unwind(AssertUnwindSafe(|| { ... }))`. On panic, send an error result through crossbeam-channel so the UI can show "thumbnail generation failed" instead of silently stopping.
**Why:** If a worker panics (bad STL data, OOM, logic bug), the channel receiver gets RecvError but the UI doesn't know the cause. The placeholder stays forever — user never knows generation failed.
**Depends on:** v0.0.1 completion (the channel infrastructure)
**Context:** The channel arch is: UI spawns generation tasks → workers send RGBA buffers back → UI replaces placeholders. The panic path in this flow is untested and has no error message.

### TODO-02: GPU readback performance baseline
**Added:** 2026-05-06 (via /plan-eng-review)
**Target:** Before v0.0.3 (real wgpu renderer)
**What:** Benchmark GPU→CPU texture readback latency on target hardware (macOS Metal). Measure: 256x256 RGBA readback time, 512x512 readback time, impact on frame rate when reading back N textures per frame.
**Why:** v0.0.3's real renderer depends on reading rendered textures back from GPU to CPU for PNG caching. The design doc acknowledges 5-20ms per readback on Metal but this hasn't been measured. If readback is consistently >15ms, v0.0.3 needs a different approach (render-to-CPU directly, or async readback pipeline).
**Depends on:** v0.0.2 (working wgpu context + channel infrastructure for async work)

### TODO-03: Cross-platform GPU surface testing
**Added:** 2026-05-06 (via /plan-eng-review)
**Target:** v0.1.0
**What:** Test egui + wgpu window creation and basic rendering on Windows (DX12/Vulkan) and Linux (Vulkan). Verify no driver-specific rendering artifacts, window resize behavior, and HiDPI scaling.
**Why:** The design commits to cross-platform distribution but Phase 1 is macOS-only. GPU surface creation has platform-specific failure modes (adapter enumeration, swapchain creation, present modes). Finding these at v0.1.0 instead of v1.0 avoids a "works on my machine" launch.
**Depends on:** v0.0.3 (real renderer working on macOS)
