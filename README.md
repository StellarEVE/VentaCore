# VentaCore

> **License Notice:** The repository currently includes a `GPL-3.0-or-later` license file as recommended in the project brief. The formal license decision remains pending owner confirmation (GPL-3.0-or-later vs. Apache-2.0).

## What it is

VentaCore is a free video editor for Windows that makes music videos with you. You give it one song and a pile of video footage. It listens to the song, looks through the footage, and builds a first cut where every cut lands on the beat. You can export that cut as it is, or open the editor and change anything you like.

## Who it is for

People who make music videos from footage they own or have the right to use: fan edits, gaming montages, film and series tributes, personal event videos. You do not need to be technical. It is meant to feel smooth even on a modest laptop.

## How it works

### 1. You bring in your files
Open VentaCore and add your song and your videos. This is instant, because nothing heavy happens yet. If a video has a part you never want used, such as a logo at the start, you can mark it. You can also mark favourite moments you would like to see in the final video.

### 2. It listens to the song
VentaCore finds the beats of the song, how they group into bars, and where the music gets louder, quieter or changes. Within seconds you can see the beats drawn on screen. If it mishears the tempo, you can correct it by hand.

### 3. It studies your footage
At the same time it looks through your videos to find scene changes, dark moments and action. It does this in the background, so you never stare at a frozen screen.

### 4. It builds the first cut
Clips start dropping onto the timeline, one after another, each placed to match the music. Busy, loud parts of the song get faster cuts. Calm parts get longer ones. Clips look faded while the program might still improve them, and become solid as it finishes studying the footage. You can press play at any time, using a light preview that runs well on slower computers.

### 5. You stay in charge
Pin any clip you love and the program will never change it. Swap a clip for another suggestion, or regenerate the rest for a different result. When you are happy, you can export right away.

### 6. Fine-tuning in the editor
If you want more control, the editor lets you move, trim, split and replace clips, adjust the sound, and add transitions, effects and text. Everything you do can be undone.

### 7. Saving the video
Export your finished video as a standard video file that plays almost anywhere. If you export again after small changes, only the changed parts are prepared again, so it is quicker.

## Your files stay yours

Everything happens on your own computer. There is no account, no subscription, no watermark and no internet connection needed. Your original videos and song are never changed. Your project is saved as one file, and your work is protected against crashes.

---

## Workspace Architecture

VentaCore is organised into eight crates across strictly ordered architectural layers:

| Layer | Crate | Directory | Responsibility |
|---|---|---|---|
| Layer 1 | `ventacore-core` | `crates/core` | Project memory: timeline model, integer `Tick` arithmetic, edit commands, snapshot history, JSON persistence. |
| Layer 2 | `ventacore-audio` | `crates/audio` | Song listening: multiband spectral flux onsets, tempo autocorrelation, beat grid, energy envelopes. |
| Layer 2 | `ventacore-platform` | `crates/platform` | System boundary: in-process FFmpeg video/audio decoding and encoding, graphics adapter probing. |
| Layer 3 | `ventacore-media` | `crates/media` | Footage study: content fingerprinting, thumbnails, staged scene analysis, motion estimation, cache. |
| Layer 3 | `ventacore-render` | `crates/render` | Graphics core: wgpu WGSL shader composition, resolution tiers, CPU reference oracle, export pipeline. |
| Layer 4 | `ventacore-engine` | `crates/engine` | Conductor: deterministic cut planner, background job pipeline, live cut building. |
| Layer 5 | `ventacore-app` | `crates/app` | Desktop GUI: four-tab native interface (Media, Cut, Edit, Export) built on egui/eframe. |
| Layer 5 | `ventacore-cli` | `crates/cli` | Command-line interface: headless engine automation and verification. |

### Architectural Rules (The Charter)
1. **Local files only**: No accounts, uploads, or online media services.
2. **Whole-number time**: Integer `Tick` values at 254,016,000,000 ticks/sec.
3. **Every change is a command**: Copy, apply, validate, commit.
4. **Codec boundary**: Codecs and FFmpeg exist exclusively within `ventacore-platform`.
5. **Graphics first, never graphics only**: Every GPU path has a working software fallback.
6. **Strict downward dependency**: A crate may only depend on crates in lower layers.

---

## Project Documentation

Detailed design documents and development guides are maintained in the [`docs/`](docs/) directory:
- [Product Plan & AI Build Guide](docs/PLAN.md) — The complete specification, charter rules, open decisions, and stage definitions.
- [Implementation Roadmap](docs/ROADMAP.md) — Execution plan, instructions for future agents, and stage exit gates.
- [Development Status](docs/DEVELOPMENT_STATUS.md) — Live snapshot of verified capabilities, test passes, and current checkpoint.

