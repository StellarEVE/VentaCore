//! Layer 5: CLI crate for VentaCore.
//!
//! Command line automation, song analysis, montage planning,
//! render export, and headless verification tool.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

use ventacore_cli::commands::analyze_song::execute_analyze_song;
use ventacore_cli::commands::capabilities::execute_capabilities;
use ventacore_cli::commands::export::execute_export;
use ventacore_cli::commands::frame::execute_frame;
use ventacore_cli::commands::measure_hardware::execute_measure_hardware;
use ventacore_cli::commands::plan::execute_plan;
use ventacore_cli::commands::purge_cache::execute_purge_cache;
use ventacore_cli::commands::run::execute_run;

#[derive(Parser, Debug)]
#[command(
    name = "ventacore",
    version,
    about = "VentaCore: Local-first beat-synchronized music video editor CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Probe and display graphics adapter, decoder, and encoder hardware capabilities.
    Capabilities,

    /// Analyze a song file for tempo, beat grid, bar grid, energy, and change points.
    AnalyzeSong {
        /// Path to audio song file (MP3, WAV, FLAC, AAC, etc.)
        song: PathBuf,

        /// Output results as JSON
        #[arg(long)]
        json: bool,
    },

    /// Generate a deterministic beat-synchronized montage plan.
    Plan {
        /// Path to the audio song file
        #[arg(short, long)]
        song: PathBuf,

        /// Paths to one or more video footage files
        #[arg(short, long, num_args = 1..)]
        footage: Vec<PathBuf>,

        /// Random generator seed for repeatable results
        #[arg(long)]
        seed: Option<u64>,

        /// Preset name (Balanced, FastCuts, Cinematic, NarrativeWalk, MontageShuffle)
        #[arg(long)]
        preset: Option<String>,

        /// Output montage plan as JSON
        #[arg(long)]
        json: bool,
    },

    /// Export an existing project timeline to full-quality MP4.
    Export {
        /// Path to the .ventacore project file
        #[arg(short, long)]
        project: PathBuf,

        /// Output destination path for the rendered MP4
        #[arg(short, long)]
        output: PathBuf,

        /// Output video width in pixels (default: 1920)
        #[arg(long)]
        width: Option<u32>,

        /// Output video height in pixels (default: 1080)
        #[arg(long)]
        height: Option<u32>,

        /// Output frame rate in frames per second (default: 30)
        #[arg(long)]
        fps: Option<u32>,
    },

    /// Run complete end-to-end beat-synced video creation and export.
    Run {
        /// Path to audio song file
        #[arg(short, long)]
        song: PathBuf,

        /// Paths to video footage files
        #[arg(short, long, num_args = 1..)]
        footage: Vec<PathBuf>,

        /// Output destination path for the rendered MP4
        #[arg(short, long)]
        output: PathBuf,

        /// Output video width in pixels (default: 1920)
        #[arg(long)]
        width: Option<u32>,

        /// Output video height in pixels (default: 1080)
        #[arg(long)]
        height: Option<u32>,

        /// Random seed
        #[arg(long)]
        seed: Option<u64>,

        /// Preset name
        #[arg(long)]
        preset: Option<String>,
    },

    /// Purge all disposable media and render cache directories (Charter Rule 9).
    PurgeCache {
        /// Optional custom cache directory root
        #[arg(long)]
        path: Option<PathBuf>,
    },

    /// Run the guided hardware proof and performance measurement kit (Stage 19).
    MeasureHardware,

    /// Extract a single frame from a video file to PNG at a given timestamp in seconds.
    Frame {
        /// Path to the video file
        video: PathBuf,

        /// Timestamp in seconds (e.g. 1.5)
        #[arg(long)]
        at: f64,

        /// Output destination path for the PNG frame
        #[arg(long)]
        out: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result: Result<(), Box<dyn std::error::Error>> = match cli.command {
        Commands::Capabilities => {
            execute_capabilities();
            Ok(())
        }
        Commands::AnalyzeSong { song, json } => execute_analyze_song(song, json),
        Commands::Plan {
            song,
            footage,
            seed,
            preset,
            json,
        } => execute_plan(song, &footage, seed, preset.as_deref(), json),
        Commands::Export {
            project,
            output,
            width,
            height,
            fps,
        } => execute_export(project, output, width, height, fps),
        Commands::Run {
            song,
            footage,
            output,
            width,
            height,
            seed,
            preset,
        } => execute_run(song, &footage, output, width, height, seed, preset.as_deref()),
        Commands::PurgeCache { path } => execute_purge_cache(path),
        Commands::MeasureHardware => execute_measure_hardware().map_err(|e| e.into()),
        Commands::Frame { video, at, out } => execute_frame(video, at, out),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {}", err);
            ExitCode::FAILURE
        }
    }
}
