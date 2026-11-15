# VentaCore: Product Plan and AI Build Guide

## Overview

### What it is
VentaCore is a free, local-first desktop video editor for Windows that builds beat-synchronised music videos with you. You provide one song and a pool of video footage. It listens to the song, analyses the footage, and constructs a deterministic first cut where every cut lands on the beat. The user can export that cut directly or open the multitrack timeline editor to refine it.

### Who it is for
Creators making music videos from footage they own or have the right to use: fan edits, gaming montages, film and series tributes, and personal event videos. Designed to feel smooth and responsive even on a modest laptop (reference test target: dual-core processor with entry-level NVIDIA MX150 graphics).

### How it works
1. **Import media**: Add song and video files instantly without background blocking. Set intro/outro trims, include/exclude ranges, or favourite moments.
2. **Listen to the song**: Detect beats, bars, phrases, energy curves, and structural change points (verses, chorus drops, breakdowns). Provides pure manual correction functions (tap tempo, half time, double time, shift grid, set downbeat).
3. **Study footage**: Staged analysis (Probe, Quick Pass, Full Pass) finds scene cuts, dark moments, and motion estimates, cached under content fingerprints.
4. **Build first cut**: Clips drop onto the timeline live as analysis proceeds. Busy passages receive faster cuts; calm passages receive longer ones. Provisional clips are marked and refine as full analysis finishes.
5. **User remains in charge**: Pinning any clip permanently protects it from automated replacement. Request ranked alternative suggestions for any slot or regenerate with new seeds/settings.
6. **Timeline refinement**: Multitrack editor with move, trim, split, ripple delete, replace, audio gain/fades, transitions, effects, and text.
7. **Export**: Full-quality MP4/H.264 export with synchronized stereo AAC audio and content-addressed render cache.

### Local-First Guarantee
Everything runs offline on the user's computer. No accounts, no subscriptions, no watermarks, no analytics, and zero cloud uploads. Original media is never modified. Projects save atomically with backups.

---

# Part 1. Brief for the AI

## Your Role
You are building VentaCore, a Windows desktop music-video editor written in Rust. You give it one song and a pool of video files. It analyses the song for beats, bars and energy, analyses the footage for scenes, dark parts and motion, and then builds a first cut with every cut landing on the music. The user can export that cut directly or refine it in a full multitrack timeline editor. It runs entirely offline on the user's computer.

The owner is not a programmer. The reference test machine is a low-end laptop with a dual-core processor and an entry-level NVIDIA MX150 graphics chip. It is a test case, not the only target.

## How to Behave
Build only the stage you are asked to build. Do not start later stages early. Do not change earlier stages except to fix a bug, and say so when you do. If a Charter rule or an open decision blocks you, stop and ask rather than guessing.

If you were given the whole document, start at Stage 0 and work through the stages in order. Finish each stage completely, give the report, then continue to the next stage. Stop and wait for the owner at the end of each Milestone (Stage 10, Stage 14, Stage 18). If a reply is not long enough to finish a stage, finish the current file or step cleanly, say exactly where you stopped, and continue when told.

If you were given only one stage, build that stage, give the report, then stop and wait.

## What You Must Hand Back After Every Stage
The code and the tests, organised as the stage describes.

A short report with five headings:
1. **What was built.**
2. **Tests run and their results.**
3. **What could not be tested and why.**
4. **Decisions made and their reasons.**
5. **Any Charter rule that was tempting to bend.**

If you cannot compile or run code in your environment, say so at the very top of the report and do not describe the code as working. Software rendering in tests proves correctness, never speed. A sandbox without a graphics card proves fallback paths, not accelerated ones.

## How Tests Are Organised
- **Unit tests**: test individual rules and functions.
- **Generated-case (property) tests**: e.g., random command sequences via `proptest` that must never leave an invalid timeline.
- **Fixture tests**: generated songs and videos with known ground truth (tempos, scene cuts, flashes on beats).
- **Reference comparisons**: GPU picture must match processor reference oracle within one colour step.
- **Layer test**: architectural test that fails if any module reaches upward or violates layer boundaries.
- **Screenshot tests**: visual regression tests for each page state.
- **End-to-end test**: validates the pipeline from song and videos to finished file with cuts on beats.

---

# Part 2. The Charter: Rules That Must Never Be Broken

A stage that breaks one of these rules is not finished.

1. **Local files only**: No accounts, uploads or online media services, ever.
2. **Whole-number time**: All positions and durations are integer ticks at 254,016,000,000 per second. No decimal seconds in edit maths.
3. **Every change is a command**: Copy the state, apply the change, check it, then keep it. An invalid timeline must be impossible to build or save.
4. **Video and audio files only through the platform module**: No other module names a codec library. FFmpeg runs inside the program, never as a separate process.
5. **Graphics first, never graphics only**: Every GPU path has a working software fallback.
6. **Hardware is proven by trial**: A decoder or encoder counts as available only after a real frame was decoded or a real file was encoded. The name of the encoder actually used is always shown.
7. **Analysis is staged**: Probe on import, a quick pass over every video, then one full decode per video with progress reported per chunk.
8. **Never change what the user touched**: Pinned, trimmed, moved or locked clips are never replaced by automation.
9. **Safe data**: One readable project file, saved atomically with a backup. Files are identified by content fingerprint. All derived data lives in one disposable cache that the user can purge. Original media is never modified.
10. **The interface is a shell**: Nothing below the app module depends on the interface toolkit. Page tabs appear only in the footer.
11. **Nothing pretends**: A control that cannot work is absent, or disabled with its reason printed. Progress bars show real progress and end at exactly full.
12. **The owner's decisions stay the owner's**: Do not silently decide anything listed under open decisions in Part 3. Mark it and ask.
13. **Honest reports**: State what was tested, how, and what could not be tested.

---

# Part 3. Scope Guard and Open Decisions

## Scope Guard
Deliberate limits for the current plan:
- No media library (projects own their files).
- No online services of any kind.
- No sorting footage by tags or facial recognition.
- No song library.
- No AI footage modification (no background removal, no auto-framing).
- No importing beat lists from other tools (VentaCore listens to the song itself).
- No automatic trimming of silence.
- No live-performance mode.
- No downloadable transition/effect marketplace.
- One timeline per project.
- No export to third-party NLE timeline formats.
- Text appears only when the user explicitly creates it.

## Decisions Already Made
- **Name**: VentaCore.
- **Primary OS**: Windows first.
- **Language**: Rust, with WGSL for graphics shaders as the only second language.
- **UI Framework**: `egui` with `eframe` as a thin shell over engine commands.
- **Graphics**: `wgpu` as main path; CPU reference oracle in tests.
- **Codecs**: In-process FFmpeg via `ventacore-platform` behind abstract `Decoder` and `Encoder` traits.
- **Hardware Decoding**: Probed via FFmpeg hardware contexts.
- **Analysis Execution**: Runs on processor first on small frames, with clean GPU extension points.
- **Preview**: Resolution tiers (Full, Half, Quarter, Eighth, Auto) plus lightweight intra-frame proxies.
- **Time Representation**: Exact 64-bit integer ticks at 254,016,000,000 ticks/sec.
- **Persistence**: Single readable, versioned JSON file with atomic rename and `.bak` backup.
- **Visual Design**: Black and silver theme with teal highlights and rounded corners.

## Open Decisions (Ask the Owner; Do Not Decide Silently)
1. **Licence**: GPL-3.0-or-later is recommended because it enables shipping the best free H.264/H.265 encoders. Apache 2.0 is the alternative with a smaller guaranteed export codec set. Until confirmed by owner, README marks licence as pending owner confirmation.
2. **FFmpeg Connection Library**: Which Rust FFmpeg binding crate to standardize on (`rsmpeg` vs `ffmpeg-next`). Proposed for Stage 4; awaits owner confirmation.
3. **Interface Details**: Exact color hex values, icon pack, and widget spacing.
4. **Real-Machine Proof**: Hardware performance verification on owner's laptop (Stage 19).
5. **Live vs. Stable Cut**: Whether the cut continues improving as full analysis completes (recommended default) or is planned once and only sharpens boundaries.

---

# Part 4. Architecture

VentaCore is arranged in five strictly ordered layers. A module may only depend on modules in lower layers.

- **Layer 1: `ventacore-core`** (`crates/core`):
  Project memory: timeline model, integer `Tick` arithmetic, edit commands, snapshot undo/redo, validation invariants, atomic JSON storage. Knows nothing of codecs, graphics, or windows.
- **Layer 2: `ventacore-audio`** (`crates/audio`) & **`ventacore-platform`** (`crates/platform`):
  - `audio`: Listens to songs, multiband spectral flux onsets, tempo autocorrelation, beat/bar/phrase grids, energy curves, change points, manual corrections.
  - `platform`: Sole home of FFmpeg bindings, audio/video codecs, graphics adapter discovery, hardware capability trials.
- **Layer 3: `ventacore-media`** (`crates/media`) & **`ventacore-render`** (`crates/render`):
  - `media`: Footage analysis (fingerprints, thumbnails, scenes, dark ranges, motion curves, cache).
  - `render`: wgpu WGSL shaders, compositing, resolution tiers, CPU reference oracle, export mixing.
- **Layer 4: `ventacore-engine`** (`crates/engine`):
  Conductor: deterministic cut planner, background job runner, live cut building.
- **Layer 5: `ventacore-app`** (`crates/app`) & **`ventacore-cli`** (`crates/cli`):
  - `app`: Native desktop GUI shell with four-page footer navigation (`egui`/`eframe`).
  - `cli`: Headless CLI executing the same engine commands for automation and verification.

---

# Part 5. Stages Overview

- **Milestone A (First Cut - CLI)**: Stages 0 to 10
  - Stage 0: Foundation (Workspace, 8 crates, layer tests, logger, fixtures, CI)
  - Stage 1: Core: the project's memory (Tick, model, commands, validation, history, storage)
  - Stage 2: Song analysis (Multiband spectral flux, autocorrelation tempo, grids, change points, corrections)
  - Stage 3: The planner (Deterministic cut planning, candidate ranking, controls, scene/dark snapping)
  - Stage 4: Platform (FFmpeg bindings, Decoder/Encoder traits, capability report)
  - Stage 5: Footage analysis (Fingerprinting, 3-stage analysis pipeline, cache)
  - Stage 6: Pipeline and live cut building (Breadth-before-depth conductor, channels, cancellation, freeze)
  - Stage 7: GPU render core (wgpu compositor, WGSL shaders, CPU reference oracle)
  - Stage 8: Preview speed (Resolution tiers, proxies, frame cache)
  - Stage 9: Export and render cache (Full-resolution export, audio mixing, content-addressed render cache)
  - Stage 10: Command line (Milestone A CLI interface, end-to-end beat-synced test)
- **Milestone B (Watch It Build - Desktop Shell)**: Stages 11 to 14
  - Stage 11: App shell (Startup logging, graphics probe, four-page footer, theme)
  - Stage 12: Media page (Import, list, trims/ranges, waveform, manual grid correction, relink)
  - Stage 13: Cut page (Live cut skeleton, thumbnail popping, settings, preview player, freeze/pin)
  - Stage 14: Export page (Codec choices, capability display, queue, presets)
- **Milestone C (Version 1.0 - Full Editor & Packaging)**: Stages 15 to 19
  - Stage 15: Edit page: the basics (Custom multitrack timeline, editing actions, inspector, audio mix)
  - Stage 16: Edit page: power tools (16a Transitions, 16b Effects, 16c Keyframes, 16d Text, 16e Background rendering)
  - Stage 17: Reliability and polish (Autosave recovery prompt, app presets, shortcut profiles, error language)
  - Stage 18: Packaging and release (Windows CI release pipeline, portable ZIP, installer, notices)
  - Stage 19: Hardware proof (Guided measurement kit on owner's dual-core MX150 laptop)

---

# Part 6. Final Checklist
- All stage exit criteria ("Finished when") met with green tests.
- 5-heading reports provided after each stage.
- Zero Charter rules bent.
- Downward layer dependencies verified by automated layer tests.
- Pinned and touched clips permanently preserved across regenerations.
- Storage writes atomically with backups.
