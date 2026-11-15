# VentaCore Implementation Roadmap & Continuation Guide

This document is the execution plan and durable continuation guide for VentaCore. It distinguishes verified implementation from planned behaviour. A feature is complete only when verified by working code and passing tests.

---

## Instructions for Future Development Sessions & Agents

This roadmap is the canonical continuation document for all contributors and autonomous coding agents.

### Repository Entry Point & Required Reading
- **Repository**: [https://github.com/StellarEVE/VentaCore](https://github.com/StellarEVE/VentaCore)
- **Target Platform**: Windows 10/11 first (reference target: dual-core CPU with entry-level NVIDIA MX150).

Before modifying code, read these documents in order:
1. `README.md`
2. `docs/DEVELOPMENT_STATUS.md` (the concise snapshot of verified implementation)
3. `docs/ROADMAP.md` (this document)
4. `docs/PLAN.md` (the product plan, charter rules, and architectural definitions)

### Naming Conventions
- Use **VentaCore** in ordinary prose, headings, and branding.
- Use `ventacore` for crate directories (`crates/core`, `crates/audio`, etc.) and package identifiers (`ventacore-core`, `ventacore-audio`, `ventacore-engine`, etc.).

### Required Git and CI Workflow
- Work directly on `main` branch.
- Keep in-progress work cohesive and push milestone or stage batches. Avoid streams of tiny speculative commits.
- Run `cargo test --workspace` and `cargo build --release --workspace` locally before pushing.
- After every implementation commit, update `docs/DEVELOPMENT_STATUS.md` and `docs/ROADMAP.md` in the same commit batch.
- Report results honestly: distinguish local tests, CI workflow results, and physical hardware verification.

### Architectural Rules (The Charter)
1. **Local files only**: Zero accounts, zero uploads, zero online tracking.
2. **Whole-number time**: All edit mathematics runs on integer `Tick` (254,016,000,000 ticks/sec).
3. **Every change is a command**: Copy state, apply change, validate invariants, commit. On error, discard candidate leaving state untouched.
4. **Codec isolation**: Only `ventacore-platform` touches codec libraries (`ffmpeg`).
5. **Graphics first, never graphics only**: Every GPU path has a software fallback.
6. **Hardware proven by trial**: Real frames decoded or real files encoded before reporting support.
7. **Staged analysis**: Probe -> Quick Pass -> Full Pass with cancellation and caching.
8. **Never change what the user touched**: Pinned, trimmed, or moved clips are protected permanently.
9. **Safe data**: Atomic JSON persistence with `.bak` backup and separate autosave.
10. **UI is a shell**: `egui`/`eframe` exists only in `ventacore-app`.
11. **Strict downward dependency**: A module may only use modules in lower layers. Enforced by `crates/engine/tests/workspace_layer_check.rs`.

---

## Stage Delivery Roadmap

### Milestone A: First Cut (CLI) — Stages 0 to 10

- [x] **Stage 0: Foundation** (Completed: commit `bc2b3e6`)
  - Workspace with 8 crates: `core`, `audio`, `platform`, `media`, `render`, `engine`, `app`, `cli`.
  - GPL-3.0-or-later license and README with license status note.
  - Workspace architecture layer enforcement integration test (`workspace_layer_check.rs`).
  - Panic-safe line logger (`logger.rs`).
  - Deterministic click-train fixture generator (`fixtures.rs`).
  - CI pipelines: Linux fast CI (`linux-ci.yml`) and Windows release CI (`windows-release.yml`).
  - *Exit gate passed*: Workspace builds, all tests pass, layer check passes and fails on wrong samples, logger writes 1,000 lines intact after crash simulation.

- [x] **Stage 1: Core: The Project's Memory** (Completed: commit `02ff624`)
  - Integer `Tick` arithmetic (254,016,000,000 ticks/sec) with exact zero-error conversions for 24, 25, 30, 50, 60 fps, 23.976, 29.97, 59.94 fps, 44.1 kHz, 48 kHz.
  - Project model: `Asset`, `Track`, `Item`, `Transition`, `Marker`, `SongEntry`, `GenerationSettings`, `Project`.
  - Command pattern (`apply_command`): copy-on-write, validation, atomic commit/rollback.
  - Snapshot undo/redo (`History`).
  - Atomic JSON persistence (`save_project_atomic` with `.bak` backup, `save_autosave`, `load_project`, migration chain).
  - Property-based command fuzzing (`command_fuzz.rs` via `proptest`).
  - *Exit gate passed*: Exact tick conversions verified, 100 random multi-command sequences never leave invalid timeline, invalid commands rejected with zero mutation, save/load round-trips byte-for-byte, crash simulation preserves original file, undo/redo restores exact states.

- [x] **Stage 2: Song Analysis** (Completed: commit `eec147e`)
  - Pure-Rust decoding trait and `SymphoniaDecoder` implementation with platform fallback hook.
  - Multiband spectral flux onset detection using `rustfft` (Low: 0-250 Hz, Mid: 250-2500 Hz, High: >2500 Hz).
  - Autocorrelation tempo estimation (60-200 BPM) with perceptual prior weighting, harmonic octave disambiguation, and sub-bin parabolic peak interpolation.
  - Grid generation: `GridMode::Grid` (mathematical tempo/phase) and `GridMode::Transient` (snapped to strong onsets).
  - Bar and phrase derivations.
  - Loudness envelope and peak-delta change point detection (verses, drops, breakdowns).
  - Waveform peaks, loudness envelope, and per-second/per-bar energy curves for the planner.
  - Pure manual correction functions: `tap_tempo`, `half_time`, `double_time`, `shift_grid`, `set_downbeat`.
  - *Exit gate passed*: Click trains at 90, 120, 140 BPM measure within $\pm 1$ BPM; alternating octave ambiguity resolved; manual corrections tested; change points detected in quiet-to-loud signal; 100% deterministic results across runs.

- [x] **Stage 3: The Planner** (Completed: Stage 3 checkpoint)
  - Deterministic montage planner inside `ventacore-engine::planner`.
  - Takes project snapshot, song structure, footage analysis (provisional or full), settings, seed, and pinned/touched clips.
  - Emits timeline plan with ranked alternative candidates per slot.
  - Planner controls: clip variance, source reuse penalty, spread, continue start points, placement modes (`WindowedWalk`, `EvenWindows`), duration limits, cut offset, scene snapping, dark edge snapping, energy density scaling, grid modes, presets.
  - Pinned and touched clips are strictly respected and preserved byte-for-byte.
  - *Exit gate passed*: Same inputs + seed produce identical clip list; refinement leaves pinned clips byte-for-byte unchanged; no clip crosses trims, exclusions, or black ranges; cuts land on grid with offset; higher energy produces shorter cuts; alternatives ranked without self-inclusion.

- [x] **Stage 4: Platform: Video Files and Graphics Hardware** (Completed: Stage 4 checkpoint)
  - In-process FFmpeg integration behind `VideoDecoder` and `VideoEncoder` traits in `ventacore-platform`.
  - Hardware decoding and encoding context probing with real-frame trial execution (Charter Rule 6).
  - Variable frame rate (VFR) detection and binary search timecode indexing.
  - Container rotation transforms (0°, 90°, 180°, 270°) and 10-bit source conversion (`Yuv420p10le` -> `Rgba8`).
  - Windows build environment configuration (`FFMPEG_DLL_PATH`, `FFMPEG_INCLUDE_DIR`, `LIBCLANG_PATH`).
  - *Exit gate passed*: Real frames decoded and encoded; hardware capability report accurately identifies adapters and proven codecs; VFR streams indexed to exact ticks; rotation and 10-bit color verified; software fallbacks proven.

- [x] **Stage 5: Footage Analysis** (Completed: Stage 5 & 6 checkpoint)
  - Media crate: content fingerprinting (size + start/mid/end hash in < 1ms).
  - Staged analysis pipeline: Stage 0 (probe container), Stage 1 (quick pass thumbnails/coarse scenes/motion), Stage 2 (full pass exact scenes, black ranges, motion, best moments).
  - Content-addressed disposable disk cache under fingerprint with full purge support.
  - *Exit gate passed*: Fingerprints invariant across moves; thumbnails generated at 160x90; exact scene cuts and black ranges detected matching ground truth; cache round-trip verified.

- [x] **Stage 6: Pipeline and Live Cut Building** (Completed: Stage 5 & 6 checkpoint)
  - Conductor in `ventacore-engine`: background worker threads, channels, and cooperative cancellation tokens.
  - Breadth-before-depth execution: song analysis -> quick passes on all videos -> emit provisional live cut -> prioritized full passes on placed timeline clips -> refine provisional clips.
  - Freeze action (`freeze_timeline_cuts`, `unfreeze_timeline_cuts`) and status reporting.
  - *Exit gate passed*: Provisional cut emitted live after quick passes; prioritized full passes refine timeline cuts; cooperative cancellation halts cleanly; freeze action pins clips permanently.

- [x] **Stage 7: GPU Render Core** (Completed: Stage 7 checkpoint)
  - `wgpu` compositor in `ventacore-render` with WGSL shaders (`blit.wgsl`, `transition.wgsl`).
  - Hard cuts, cross dissolves, transforms, fit modes (crop, black bars letterboxing, blur padding).
  - Resolution tiers (`Full`, `Half`, `Quarter`, `Eighth`) with even dimension scaling.
  - CPU reference oracle (`CpuReferenceOracle`) with analytical rendering and automatic fallback.
  - *Exit gate passed*: 10 render tests passed; hardware acceleration verified on NVIDIA GeForce MX150; max difference between GPU output and CPU oracle is 0 (satisfies Charter Rule 5 $\le 1$ color step).

- [x] **Stage 8: Preview Speed: Monitor Resolution & Proxies** (Completed: Stage 8 & 9 checkpoint)
  - Intra-frame proxy generation (`ProxyTier`) cached under `<fingerprint>/proxies/<tier>/` with $O(1)$ random seek.
  - In-memory bounded LRU frame cache (`FrameCache`) with 128 MB default capacity and eviction.
  - Dynamic performance governor (`PerformanceGovernor`) for viewport-aware tier selection and drop compensation.
  - Audio master clock synchronization (`MasterClock`) with integer tick arithmetic and drift correction (`SyncAction::Display`, `Drop`, `Repeat`).
  - *Exit gate passed*: 4 preview tests and proxy seek integration test passed.

- [x] **Stage 9: Export and Render Cache** (Completed: Stage 8 & 9 checkpoint)
  - Multitrack stereo audio mixer (`AudioMixer`) with gain, fade-in/fade-out envelopes, mute/solo, and soft-limiting.
  - Content-addressed disk render cache (`RenderCache`) with FNV-1a keying and purge support.
  - Full-resolution timeline export (`export_timeline`) with frame-by-frame composite rendering.
  - Atomic export finalization (write to `.part` file, rename on completion; delete on error or cancellation).
  - *Exit gate passed*: 3 mixer tests, render cache test, and atomic export integration tests passed.

- [ ] **Stage 10: Command Line (Milestone A Exit Gate)** (Next up)
  - `ventacore-cli` front end: capabilities, analyze-song, plan, export, run, purge-cache.
  - End-to-end integration test: song + videos -> beat-synced exported MP4.
  - **Milestone A Pause**: Owner review of cuts and CLI execution.

---

### Milestone B: Watch It Build (Desktop Shell) — Stages 11 to 14

- [ ] **Stage 11: App Shell**
  - `egui`/`eframe` native shell in `ventacore-app`.
  - Startup logging, headless graphics probe, four-page footer navigation (Media, Cut, Edit, Export).
  - Dark silver/black theme with teal highlight and rounded corners.

- [ ] **Stage 12: Media Page**
  - Drag-and-drop import, thumbnail filmstrips, trims, exclude/include ranges, favourite moments.
  - Waveform with beat/bar overlay and manual grid corrections.
  - Fingerprint-based relinking for moved files.

- [ ] **Stage 13: Cut Page**
  - Live timeline skeleton with provisional hatching.
  - Thumbnail popping, pin/unpin, ranked candidate menu, generator controls, preview player.

- [ ] **Stage 14: Export Page (Milestone B Exit Gate)**
  - Codec selection showing proven hardware encoders.
  - Export queue with background progress and cancellation.
  - **Milestone B Pause**: Owner review on real Windows laptop.

---

### Milestone C: Version 1.0 (Full Editor & Packaging) — Stages 15 to 19

- [ ] **Stage 15: Edit Page: The Basics**
  - Custom multitrack timeline (video and audio tracks).
  - Move, trim, split, ripple delete, replace, snap to beats/bars, inspector, audio mixer.

- [ ] **Stage 16: Edit Page: Power Tools**
  - 16a: Transitions (GPU shaders: dissolve, dip, push, slide, zoom, twist).
  - 16b: Effects & adjustment layers (brightness, contrast, blur, shake, strobe, glitch, beat pulse).
  - 16c: Keyframes & curve editor (transform, opacity, volume, effect parameters).
  - 16d: Text & split-screen layouts.
  - 16e: Background rendering of heavy regions.

- [ ] **Stage 17: Reliability & Polish**
  - Autosave crash recovery prompt, app-wide presets, shortcut profiles, accessible UI.

- [ ] **Stage 18: Packaging & Release (Milestone C Exit Gate)**
  - Windows release CI producing portable ZIP and Inno Setup installer with license notices.

- [ ] **Stage 19: Hardware Proof**
  - Guided measurement kit on owner's dual-core NVIDIA MX150 laptop.
