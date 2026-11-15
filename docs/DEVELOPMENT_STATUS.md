# VentaCore Development Status & Verified Capabilities

**Last Updated**: 2026-11-15  
**Current Branch**: `main`  
**Latest Verified Commit**: (Stage 4 checkpoint)  
**Repository**: [https://github.com/StellarEVE/VentaCore](https://github.com/StellarEVE/VentaCore)

---

## Current Status Checkpoint

| Check | Result | Details |
|---|---|---|
| **Workspace Compilation (Debug)** | **PASS** | All 8 crates compiled with zero warnings/errors. |
| **Workspace Compilation (Release)** | **PASS** | `cargo build --release --workspace` completed in 4.85s. |
| **Unit & Integration Tests** | **PASS** | 54 tests passed; 0 failed; 0 ignored. |
| **Layer Architecture Enforcement** | **PASS** | Strict downward dependency direction and border checks verified. |
| **Property-Based Invariant Fuzzing** | **PASS** | 100 randomized multi-command test runs verified with `proptest`. |
| **CI Pipelines Configured** | **PASS** | Linux fast CI (`linux-ci.yml`) and Windows release CI (`windows-release.yml`). |

---

## Verified Capabilities by Stage

### Stage 0: Foundation (`bc2b3e6`)
- **8-Crate Workspace**: `ventacore-core`, `ventacore-audio`, `ventacore-platform`, `ventacore-media`, `ventacore-render`, `ventacore-engine`, `ventacore-app`, `ventacore-cli`.
- **Architectural Layer Enforcement**: Automated test `workspace_layer_check.rs` verifies that crates never import upward or across disallowed peer boundaries, that only `ventacore-platform` touches codec libraries, and that only `ventacore-app` touches GUI frameworks (`egui`/`eframe`). Includes tests proving deliberate failure on invalid samples.
- **Panic-Safe Line Logger**: `append_log_line` writes line-by-line, opening and closing each time, flushing to disk with `sync_all()`, and wrapping in `catch_unwind(AssertUnwindSafe)` to guarantee no panics under any conditions. Verified by writing 1,000 lines intact through abrupt stop simulation.
- **Deterministic Audio Click-Train Generator**: `generate_click_train` produces identical, repeatable synthetic click trains at specified BPM, duration, and sample rate.
- **Licensing & Documentation**: Complete GPL-3.0-or-later license file and plain-language README.

### Stage 1: Core: The Project's Memory (`02ff624`)
- **Exact Integer Tick Time**: `Tick(i64)` at 254,016,000,000 ticks/sec with exact zero-rounding-error conversions verified for 24, 25, 30, 50, 60 fps, 23.976, 29.97, 59.94 fps, and audio sample rates 44,100 Hz and 48,000 Hz.
- **Project Data Model**: `Asset` (fingerprint, probe, trims, include/exclude ranges, favourite moments), `Track` (video/audio, mute, solo, lock, hidden), `Item` (clips, source_in, duration, pin, touch, transition, effect text stacks, volume), `Transition`, `Marker`, `SongEntry`, `GenerationSettings`.
- **Command Architecture (Charter Rule 3)**: `apply_command` copies state, mutates, validates invariants, and commits on success. On any error, the candidate is discarded, leaving the state untouched.
- **Validation Engine**: Enforces unique IDs, positive clip durations, referential integrity, and non-overlapping items on tracks.
- **Snapshot History**: `History` supporting state snapshots, undo, redo, and undo depth limits.
- **Atomic Storage**: `save_project_atomic` writes to a unique temporary file, flushes/syncs, creates a `.bak` backup, and atomically renames the temporary file over target. `save_autosave` persists `.autosave.json`. Migration chain initialized for schema version 1.
- **Property-Based Fuzzing**: Proptest running 100 randomized multi-command sequences verifies that timelines never become invalid and failing commands leave project state untouched.

### Stage 2: Song Analysis (`eec147e`)
- **Pure-Rust Decoding Trait**: `AudioDecoder` trait with default `SymphoniaDecoder` implementation supporting MP3, WAV, FLAC, AAC, OGG/Vorbis, and MP4/M4A, plus hook for platform fallback.
- **Multiband Spectral Flux Onsets**: STFT analysis with Hanning windowing and `rustfft` separating Low (0-250 Hz), Mid (250-2500 Hz), and High (>2500 Hz) bands with local peak picking.
- **Autocorrelation Tempo Estimation**: Evaluates lags for 60-200 BPM, incorporates perceptual prior weighting (120 BPM center), harmonic octave disambiguation, and sub-bin parabolic peak interpolation. Verified to measure 90, 120, and 140 BPM click trains within $\pm 1$ BPM.
- **Grid Modes**: `GridMode::Grid` (regular mathematical spacing) and `GridMode::Transient` (beats snapped to nearby strong onsets).
- **Macro Loudness Change Points**: Inflection peak detection on sliding RMS loudness envelopes identifying verses, chorus drops, and breakdowns.
- **Display Peaks & Energy Curves**: Downsampled waveform peaks (50 pts/sec), continuous energy per second, and segmented energy per bar for cut-density scaling.
- **Pure Manual Correction Functions**: `tap_tempo`, `half_time`, `double_time`, `shift_grid`, `set_downbeat`.

### Stage 3: The Planner (`322d01b`)
- **Deterministic Montage Planner**: `plan_montage` in `ventacore-engine::planner` takes project snapshot, song structure, footage analysis, and settings to build a complete cut timeline with ranked alternatives per slot.
- **Charter Rule 8 Enforcement**: Pinned and touched clips are strictly respected and preserved byte-for-byte; open intervals are automatically computed around them.
- **Hard Constraint Safety**: Candidate footage windows never cross intro trims, outro trims, exclude ranges, or black ranges.
- **Beat Grid Alignment & Cut Offset**: Cuts land synchronously on beats, with configurable lead offset (`cut_offset`) to ensure cuts hit right on visual action transients.
- **Energy Density Cut Scaling**: Bar energy dynamically scales cut intervals—high-energy passages yield rapid cuts (1 beat) while low-energy passages yield longer, steady cuts (4-8 beats).
- **Candidate Ranking & Selection**: Multi-factor scoring incorporating song-to-footage motion alignment, favourite moments bonus, scene cut snap bonus, and source reuse penalty. Alternatives are ranked in descending score order with guaranteed zero self-inclusion.
- **Placement Modes**: Supports both `PlacementMode::WindowedWalk` (with `continue_start_points` and spread jump controls) and `PlacementMode::EvenWindows`.
- **Command Integration**: `apply_plan_to_project` applies cut timelines strictly through atomic, validated Project commands (`RemoveItem` for unpinned items, `InsertItem` for new cuts).

### Stage 4: Platform: Video Files and Graphics Hardware
- **Hardware Probing & Trial Execution (Charter Rule 6)**: `probe_hardware_capabilities()` discovers graphics adapters, probes candidate hardware decoders and encoders (NVDEC/NVENC, D3D11VA, MediaFoundation), and executes trial runs (`run_decoder_trial`, `run_encoder_trial`). Codecs count as available only after a real frame is decoded or encoded.
- **Active Codec Disclosure (Charter Rules 6 & 11)**: `active_decoder_name()` and `active_encoder_name()` always reveal the exact encoder/decoder used (e.g. `h264_nvenc (Hardware)` or `h264 (pure software fallback)`).
- **Universal Software Fallback (Charter Rule 5)**: High-performance pure-Rust reference video decoders and encoders guarantee that headless CI, sandboxes, and machines without dedicated GPUs work reliably.
- **Abstract Codec Traits**: `VideoDecoder` (metadata inspection, seek, sequential decode, EOF) and `VideoEncoder` (encode_frame, stream finalization) behind unified traits.
- **Variable Frame Rate (VFR) Handling**: `VfrTimecodeIndex` normalizes fluctuating presentation timestamps to whole-number `Tick`, provides $O(\log N)$ binary search frame lookups, and detects VFR jitter > 5%.
- **Container Rotation Handling**: Automatic extraction and application of smartphone/camera orientation metadata (0°, 90°, 180°, 270°) with buffer transposition and pixel rotation.
- **10-Bit & 12-Bit Source Handling**: Color conversion from 10-bit planar YUV (`Yuv420p10le`) to 8-bit RGBA without bit-shift clipping or banding.
- **Environment Configuration**: `PlatformEnvironmentConfig` probes and validates `FFMPEG_DLL_PATH`, `FFMPEG_INCLUDE_DIR`, and `LIBCLANG_PATH` for custom Windows build workflows.

---

## Active Milestone & Next Target

- **Current Milestone**: Milestone A: First Cut (CLI)
- **Current Stage**: **Stage 5: Footage Analysis**
- **Goal for Stage 5**: Media crate footage analysis pipeline: content fingerprinting (file size + start/mid/end hash), Stage 0 container probe, Stage 1 quick pass (thumbnails, coarse scene cuts, rough motion), Stage 2 full pass (exact scene boundaries, black range detection, motion curve), and content-addressed disk caching.

---

## Open Decisions Tracking (Part 3)

| Decision | Current State | Notes |
|---|---|---|
| **License** | GPL-3.0-or-later included; pending formal confirmation | Recommended to ship free H.264/H.265 encoders; marked pending in README. |
| **FFmpeg Library** | `symphonia` pure-Rust used for Stage 2; Stage 4 candidate pending | Platform abstraction layer isolates codec bindings completely behind traits. |
| **UI Theme & Icons** | Dark silver/black theme with teal accents specified | Exact hex colors and widget layout to be finalized in Stage 11. |
| **Hardware Baseline** | Windows 10/11 x86_64 target | Physical verification on dual-core MX150 laptop scheduled for Stage 19. |
