// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[tauri::command]
fn toggle_playback() -> bool {
    println!("[Tauri IPC] toggle_playback invoked");
    true
}

#[tauri::command]
fn probe_hardware() -> String {
    let report = ventacore_platform::probe_hardware_capabilities();
    format!("{:?}", report)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![toggle_playback, probe_hardware])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
