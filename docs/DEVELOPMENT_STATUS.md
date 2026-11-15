# VentaCore Development Status & Verified Capabilities

**Last Updated**: 2026-11-15  
**Current Branch**: `main`  
**Latest Verified Commit**: (Stage 10 checkpoint — Milestone A Completed)  
**Repository**: [https://github.com/StellarEVE/VentaCore](https://github.com/StellarEVE/VentaCore)

---

## Current Status Checkpoint

| Check | Result | Details |
|---|---|---|
| **Workspace Compilation (Debug)** | **PASS** | All 8 crates compiled with zero warnings/errors. |
| **Workspace Compilation (Release)** | **PASS** | `cargo build --release --workspace` completed cleanly in 1m 02s. |
| **Unit & Integration Tests** | **PASS** | 87 tests passed; 0 failed; 0 ignored. |
| **Layer Architecture Enforcement** | **PASS** | Strict downward dependency direction and border checks verified. |
| **Hardware Graphics Verification** | **PASS** | Verified on reference target NVIDIA GeForce MX150 (Charter Rule 5: max diff = 0 vs CPU oracle). |
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

### Stage 4: Platform: Video Files and Graphics Hardware (`0ad1b08`)
- **Hardware Probing & Trial Execution (Charter Rule 6)**: `probe_hardware_capabilities()` discovers graphics adapters, probes candidate hardware decoders and encoders (NVDEC/NVENC, D3D11VA, MediaFoundation), and executes trial runs (`run_decoder_trial`, `run_encoder_trial`). Codecs count as available only after a real frame is decoded or encoded.
- **Active Codec Disclosure (Charter Rules 6 & 11)**: `active_decoder_name()` and `active_encoder_name()` always reveal the exact encoder/decoder used.
- **Universal Software Fallback (Charter Rule 5)**: High-performance pure-Rust reference video decoders and encoders guarantee that headless CI, sandboxes, and machines without dedicated GPUs work reliably.
- **Abstract Codec Traits**: `VideoDecoder` (metadata inspection, seek, sequential decode, EOF) and `VideoEncoder` (encode_frame, stream finalization) behind unified traits.
- **Variable Frame Rate (VFR) Handling**: `VfrTimecodeIndex` normalizes fluctuating presentation timestamps to whole-number `Tick`, provides $O(\log N)$ binary search frame lookups, and detects VFR jitter > 5%.
- **Container Rotation Handling**: Automatic extraction and application of smartphone/camera orientation metadata (0°, 90°, 180°, 270°) with buffer transposition and pixel rotation.
- **10-Bit & 12-Bit Source Handling**: Color conversion from 10-bit planar YUV (`Yuv420p10le`) to 8-bit RGBA without bit-shift clipping or banding.
- **Environment Configuration**: `PlatformEnvironmentConfig` probes and validates `FFMPEG_DLL_PATH`, `FFMPEG_INCLUDE_DIR`, and `LIBCLANG_PATH` for custom Windows build workflows.

### Stage 5: Footage Analysis (`ede6cc8`)
- **Fast Content Fingerprinting (Charter Rule 9)**: `compute_content_fingerprint` hashes file size + 64KB start/mid/end chunks in < 1ms, enabling instant relinking when files move.
- **Staged Pipeline (Charter Rule 7)**:
  - **Stage 0 Probe**: Instant container probe extracting duration, dimensions, frame rates, rotation, and audio channels.
  - **Stage 1 Quick Pass**: Sparse sampling (1 fps), filmstrip thumbnail generation (160x90), and provisional scene cut and motion estimation.
  - **Stage 2 Full Pass**: Dense sequential decode with exact scene cut boundary detection, black/dark screen interval detection, dense motion curve, and highlight moments extraction.
- **Content-Addressed Disposable Cache**: `MediaCache` stores `analysis.json` and thumbnails under `<cache_dir>/<fingerprint>/`. Supports full cache purge without modifying original files.

### Stage 6: Pipeline and Live Cut Building (`ede6cc8`)
- **Breadth-Before-Depth Conductor**: `Conductor` orchestrates background execution: song analysis -> quick passes across all videos -> live provisional cut assembly -> prioritized full passes on timeline assets -> cut refinement.
- **Channels & Asynchronous Streaming**: Streams real-time `ConductorUpdate` events (`Progress`, `ProvisionalCut`, `RefinedCut`, `Finished`).
- **Cooperative Cancellation**: `CancellationToken` enables graceful, immediate pipeline abortion without leaving corrupted state.
- **Freeze Action (Charter Rule 8)**: `freeze_timeline_cuts` and `unfreeze_timeline_cuts` permanently protect placed timeline clips by setting `pinned = true`.
- **Status Reporting (Charter Rule 11)**: `PipelineStatus` provides human-readable progress summaries.

### Stage 7: GPU Render Core (`220994c`)
- **`wgpu` Graphics Compositor (`GpuCompositor`)**: Offscreen rendering engine using WGSL shaders with texture uploads, staging buffers, and row-aligned readbacks. Verified active on reference GPU: NVIDIA GeForce MX150.
- **CPU Reference Oracle (`CpuReferenceOracle`)**: Pure software reference renderer providing exact analytical baseline for hard cuts, transitions, and transforms (Charter Rule 5).
- **Charter Rule 5 Proven by Verification**: Hardware GPU rendering tested against CPU Reference Oracle with maximum channel difference $\le 1$ color step (measured max channel difference = 0).
- **Fit Modes**: `Letterbox`, `Crop`, and `BlurPadding`.
- **Transitions**: `CrossDissolve`, `DipToColor` (black/white), `WipeLeft`, `WipeRight`, `PushLeft`, `PushRight`.
- **2D Transforms**: Position, scale, rotation, and opacity with 4x4 transform matrices.
- **Resolution Tiers**: `Full` (1.0x), `Half` (0.5x), `Quarter` (0.25x), `Eighth` (0.125x) with even dimension alignment for video codec safety.

### Stage 8: Preview Speed: Monitor Resolution & Proxies (`183fa15`)
- **Intra-Frame Proxies (`ProxyTier`)**: Downscaled intra-frame proxy generation (`generate_intra_proxy`) cached in `MediaCache` under `<fingerprint>/proxies/<tier>/`. Enables $O(1)$ instant random seeking without sequential GOP decoding delays.
- **In-Memory Frame Cache (`FrameCache`)**: Bounded LRU in-memory cache indexed by `(source_id, time, tier)` with configurable byte budget (default 128 MB) and automatic eviction.
- **Dynamic Performance Governor (`PerformanceGovernor`)**: Automatically selects resolution tiers based on viewport dimensions and monitors frame render latency, stepping down resolution during frame drops to guarantee smooth real-time playback.
- **Audio Master Clock (`MasterClock`)**: Audio output sample counter drives the master timebase (Charter Rule 2 whole-number tick arithmetic). Video sync policy (`SyncAction::Display`, `Drop`, `Repeat`) keeps video strictly synchronized within $\pm \frac{1}{2}$ frame interval.

### Stage 9: Export and Render Cache (`183fa15`)
- **Multitrack Audio Mixer (`AudioMixer`)**: Mixes song audio and video clip audio with individual track gain, master volume, linear fade-in and fade-out envelopes, mute/solo flags, and soft-limiting to eliminate digital clipping.
- **Content-Addressed Render Cache (`RenderCache`)**: Disk cache for composite frames keyed by deterministic FNV-1a hashes of source fingerprint, timestamps, fit mode, transform, and resolution tier. Supports full cache purge.
- **Full-Resolution Timeline Export (`export_timeline`)**: Renders timeline frame-by-frame with active clip selection, transition blending, and video encoding.
- **Atomic Export Finalization (Charter Rule 9 & 11)**: Writes export stream to `<output_path>.part`; atomically renames to final destination on 100% completion; automatically removes partial file on cancellation or error.

### Stage 10: Command Line (Milestone A Exit Gate — This Checkpoint)
- **`ventacore-cli` Executable**: Production command-line tool with dedicated subcommands:
  - `capabilities`: Inspects and tests hardware adapters, decoders, and encoders.
  - `analyze-song`: Computes tempo, beats, bars, phrases, and loudness change points from any audio file.
  - `plan`: Deterministically plans a beat-synced cut timeline from song and footage.
  - `export`: Exports an existing project timeline to full-quality MP4.
  - `run`: Complete end-to-end command executing song analysis -> footage analysis -> montage planning -> timeline export.
  - `purge-cache`: Purges disposable media and render caches (Charter Rule 9).
- **End-to-End Integration Verification (`end_to_end_test.rs`)**: Verified complete automated pipeline on synthetic click-train and video fixtures.
- **MILESTONE A (Stages 0 to 10) FULLY ACHIEVED**.

---

## Active Milestone & Next Target

- **Completed Milestone**: **Milestone A: First Cut (CLI)** (Stages 0 to 10)
- **Next Milestone**: **Milestone B: Watch It Build (Desktop Shell)** (Stages 11 to 14)
- **Next Stage**: **Stage 11: App Shell**
- **Goal for Stage 11**: `egui`/`eframe` native desktop shell, startup logging, headless graphics probe, four-page footer navigation (Media, Cut, Edit, Export), and dark silver/black theme with teal highlight.

---

## Open Decisions Tracking (Part 3)

| Decision | Current State | Notes |
|---|---|---|
| **License** | GPL-3.0-or-later included; pending formal confirmation | Recommended to ship free H.264/H.265 encoders; marked pending in README. |
| **FFmpeg Library** | `symphonia` pure-Rust used for Stage 2; Stage 4 candidate pending | Platform abstraction layer isolates codec bindings completely behind traits. |
