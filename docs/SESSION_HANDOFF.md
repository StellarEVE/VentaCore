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

---

## 3. Current Test & Build Status
- **Workspace Tests**: `cargo test --workspace` -> **54 tests passed; 0 failed**.
- **Release Build**: `cargo build --release --workspace` -> **Clean compilation in 4.85s**.

---

## 4. Next Task: Stage 5 (Footage Analysis)
The next task to build is **Stage 5: Footage Analysis** inside `crates/media`:
1. Media crate footage analysis pipeline:
   - Content fingerprinting (file size + start/mid/end hash).
   - Stage 0: Probe container (duration, resolution, fps, rotation, audio tracks).
   - Stage 1: Quick pass (thumbnails, coarse scene cuts, rough motion estimate).
   - Stage 2: Full pass (exact scene boundaries, black range detection, dense motion curve, best moments).
2. Content-addressed disk cache under fingerprint.
