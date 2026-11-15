# VentaCore Session Handoff & Development Context

This document captures the full context of the development sessions up to **Stage 2** so that any session or autonomous agent opened inside the VentaCore workspace can immediately continue development without missing any context.

---

## 1. Project Identity & Location
- **Local Directory**: `C:\Users\Creative Workshop\Documents\GitHub\VentaCore`
- **GitHub Remote**: `https://github.com/StellarEVE/VentaCore.git`
- **Active Branch**: `main`
- **Author/Owner**: StellarEVE (`stellarpmv@proton.me`)

---

## 2. Completed Milestones & Stages

### Stage 0: Foundation (Commit `bc2b3e6`)
- Workspace defined with 8 crates:
  - `ventacore-core` (Layer 1)
  - `ventacore-audio` (Layer 2)
  - `ventacore-platform` (Layer 2)
  - `ventacore-media` (Layer 3)
  - `ventacore-render` (Layer 3)
  - `ventacore-engine` (Layer 4)
  - `ventacore-app` (Layer 5)
  - `ventacore-cli` (Layer 5)
- Automated layer architecture check (`crates/engine/tests/workspace_layer_check.rs`) verifying strict downward dependency flow, UI toolkit containment, and codec isolation.
- Panic-safe line logger (`crates/core/src/logger.rs`).
- Deterministic click-train fixture generator (`crates/core/src/fixtures.rs`).
- CI workflows: `.github/workflows/linux-ci.yml` (Linux fast test) and `.github/workflows/windows-release.yml` (Windows release packaging).
- GPL-3.0-or-later license file and plain-words README.

### Stage 1: Core: The Project's Memory (Commit `02ff624`)
- Exact integer tick arithmetic (`crates/core/src/time.rs`): 254,016,000,000 ticks/sec with exact conversions for all standard video rates (23.976, 24, 25, 29.97, 30, 50, 59.94, 60 fps) and audio rates (44.1 kHz, 48 kHz).
- Full Project data model (`crates/core/src/model.rs`).
- Copy-on-write command pattern (`crates/core/src/command.rs`) enforcing Charter Rule 3.
- Invariant validation engine (`crates/core/src/validation.rs`).
- State snapshot undo/redo history (`crates/core/src/history.rs`).
- Atomic JSON persistence with `.bak` backup and separate autosave (`crates/core/src/storage.rs`).
- Randomized command fuzzing via `proptest` (`crates/core/tests/command_fuzz.rs`).

### Stage 2: Song Analysis (Commit `eec147e`)
- Pure-Rust `AudioDecoder` trait and `SymphoniaDecoder` implementation (`crates/audio/src/decoder.rs`).
- Multiband spectral flux onset detection using `rustfft` (`crates/audio/src/spectral.rs`).
- Autocorrelation tempo estimation (60-200 BPM) with perceptual prior weighting, harmonic octave disambiguation, and sub-bin parabolic peak interpolation (`crates/audio/src/tempo.rs`). Tested on 90, 120, 140 BPM click trains to $\pm 1$ BPM accuracy.
- Grid construction in `GridMode::Grid` and `GridMode::Transient` (`crates/audio/src/grid.rs`).
- RMS loudness envelopes and macro turning-point detection (`crates/audio/src/loudness.rs`).
- Waveform display downsampling, per-second energy curves, and per-bar energy curves (`crates/audio/src/curves.rs`).
- Pure manual correction functions: `tap_tempo`, `half_time`, `double_time`, `shift_grid`, `set_downbeat` (`crates/audio/src/corrections.rs`).
- Comprehensive integration suite (`crates/audio/tests/song_analysis_tests.rs`).

### Stage 3: The Planner
- Deterministic montage planner inside `ventacore-engine::planner`:
  - Inputs: Project snapshot, song structure (`SongStructure`), footage analysis (`FootageAnalysis`), generator settings (`GenerationSettings`), and pinned/touched clips.
  - Outputs: `MontagePlan` with ordered timeline cut items and ranked alternative candidate clips per slot.
- Controls implemented:
  - Clip variance, source reuse penalty, spread, continue start points.
  - Placement modes: `WindowedWalk` and `EvenWindows`.
  - Duration limits (`min_clip_duration`, `max_clip_duration`).
  - Cut offset (landing ahead of beats, e.g. -40ms).
  - Snap to scene cuts and snap to dark edges.
  - Energy density scaling (higher bar energy -> shorter cuts).
  - Seeded repeatability via pure-Rust SplitMix64 PRNG.
  - Presets (`Balanced`, `FastCuts`, `Cinematic`, `NarrativeWalk`, `MontageShuffle`).
- Hard constraints strictly enforced:
  - Pinned and touched clips are preserved byte-for-byte across planning.
  - Intro/outro trims, exclude ranges, and black ranges are never crossed.
  - Alternatives ranked without self-inclusion.
- Integration tests in `crates/engine/tests/planner_tests.rs`.

### Documentation System (Commit `6503513`)
- `docs/PLAN.md`: Full product brief, 13 Charter rules, and stages 0-19.
- `docs/ROADMAP.md`: Operational continuation guide and milestone acceptance criteria.
- `docs/DEVELOPMENT_STATUS.md`: Live tracking of test results and verified capabilities.

### Stage 4: Platform: Video Files and Graphics Hardware
- In-process FFmpeg integration behind `VideoDecoder` and `VideoEncoder` traits in `ventacore-platform`.
- Hardware capability probing with real-frame trial execution (`probe_hardware_capabilities`, `run_decoder_trial`, `run_encoder_trial`).
- Honest disclosure of active encoder/decoder names (`active_decoder_name`, `active_encoder_name`) conforming to Charter Rules 6 and 11.
- Pure-Rust software reference implementations (`SyntheticVideoDecoder`, `SyntheticVideoEncoder`) ensuring reliable testing and software fallbacks (Charter Rule 5).
- Variable Frame Rate (VFR) handling via `VfrTimecodeIndex` with binary search timeline lookup and jitter detection.
- Container rotation handling (0°, 90°, 180°, 270°) with buffer transposition and pixel rotation.
- 10-bit and 12-bit source color conversion (`Yuv420p10le` -> `Rgba8`) without clipping or banding.
- Windows build environment configuration via `PlatformEnvironmentConfig` (`FFMPEG_DLL_PATH`, `FFMPEG_INCLUDE_DIR`, `LIBCLANG_PATH`).
- Integration tests in `crates/platform/tests/platform_tests.rs`.

### Stage 5: Footage Analysis
- Fast content fingerprinting (`compute_content_fingerprint` in `crates/media/src/fingerprint.rs`) hashing file size + 64KB start/mid/end chunks in < 1ms.
- Staged analysis pipeline:
  - Stage 0 Probe (`probe.rs`): Instant container probe yielding `ProbeData`.
  - Stage 1 Quick Pass (`quick_pass.rs`): Sparse sampling (1 fps), filmstrip thumbnails (160x90), provisional scene cut and motion estimation.
  - Stage 2 Full Pass (`full_pass.rs`): Dense sequential decode, exact shot cut boundary detection, black/dark screen interval detection, dense motion curve, highlight moments extraction.
- Content-addressed disposable cache (`cache.rs`): Stores `analysis.json` and thumbnails under `<cache_dir>/<fingerprint>/`, with full cache purge support.
- Integration tests in `crates/media/tests/media_tests.rs`.

### Stage 6: Pipeline and Live Cut Building
- Breadth-before-depth conductor (`crates/engine/src/pipeline/conductor.rs`):
  1. Song analysis.
  2. Quick pass across all videos.
  3. Live provisional cut assembly.
  4. Prioritized full passes on placed timeline clips.
  5. Cut refinement.
- Channels & progress streaming via `ConductorUpdate` (`Progress`, `ProvisionalCut`, `RefinedCut`, `Finished`).
- Cooperative cancellation (`cancellation.rs`) via thread-safe `CancellationToken`.
- Freeze action (`freeze.rs`): `freeze_timeline_cuts` and `unfreeze_timeline_cuts` permanently protect clips by setting `pinned = true`.
- Pipeline status reporting (`status.rs`): Human-readable progress summaries.
- Integration tests in `crates/engine/tests/pipeline_tests.rs`.

### Stage 7: GPU Render Core
- `wgpu` graphics compositor (`GpuCompositor` in `crates/render/src/compositor.rs`):
  - Hardware offscreen rendering with WGSL shaders (`blit.wgsl`, `transition.wgsl`).
  - Offscreen render passes, staging buffers, and row-aligned byte readbacks.
  - Active and verified on physical hardware: NVIDIA GeForce MX150.
- CPU Reference Oracle (`CpuReferenceOracle` in `crates/render/src/oracle.rs`):
  - Pure software analytical reference implementation for hard cuts, transitions, transforms, and fit modes.
  - Serves as the test ground truth and guaranteed software fallback (Charter Rule 5).
- Verifications & Accuracy (Charter Rule 5):
  - GPU hardware render output matches CPU Reference Oracle within $\le 1$ color step (measured max channel diff = 0).
- Fit modes:
  - `Letterbox`: Aspect ratio preserving fit with solid black borders.
  - `Crop`: Center crop symmetrically covering the destination canvas.
  - `BlurPadding`: Outer bars padded with blurred, stretched copy of source frame.
- Transitions: `CrossDissolve`, `DipToColor` (dip to black/white), `WipeLeft`, `WipeRight`, `PushLeft`, `PushRight`.
- 2D Transforms: Position, scale, rotation, and opacity with 4x4 transform matrices.
- Resolution Tiers: `Full` (1.0x), `Half` (0.5x), `Quarter` (0.25x), `Eighth` (0.125x) with even dimension alignment.
- Automatic fallback: Graceful CPU oracle execution if GPU adapters or drivers are unavailable.
- Integration tests in `crates/render/tests/render_tests.rs` (10 tests).

---

## 3. Current Test & Build Status
- **Workspace Tests**: `cargo test --workspace` -> **75 tests passed; 0 failed**.
- **Release Build**: `cargo build --release --workspace` -> **Clean compilation in release profile**.

---

## 4. Next Task: Stage 8 (Preview Speed: Monitor Resolution & Proxies)
The next task to build is **Stage 8: Preview Speed** inside `crates/render` / `crates/media`:
1. Intra-frame proxy generation in cache.
2. Auto-resolution tiering and frame cache.
3. Audio master clock synchronization.
