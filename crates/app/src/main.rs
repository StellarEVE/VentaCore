// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod media_server;

use std::sync::{Arc, Mutex};
use commands::*;
use media_server::start_media_server;

fn main() {
    // 1. Start background local media streaming server for video/audio
    let port = start_media_server();
    println!("[VentaCore Studio] Media streaming server active on port {}", port);

    // 2. Initialize shared session state
    let session: SharedSession = Arc::new(Mutex::new(StudioSession::default()));

    // 3. Launch Tauri 2.0 Native Shell
    tauri::Builder::default()
        .manage(session)
        .invoke_handler(tauri::generate_handler![
            probe_hardware,
            toggle_playback,
            select_song,
            import_footage,
            set_in_out,
            generate_beat_cut,
            export_video,
            open_directory,
        ])
        .run(tauri::generate_context!())
        .expect("error while running VentaCore Studio desktop application");
}
