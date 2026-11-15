//! Windows build and runtime environment configuration for in-process FFmpeg and codecs.
//!
//! Inspects:
//! - `FFMPEG_DLL_PATH`: Directory containing FFmpeg shared libraries (avcodec, avformat, avutil, swscale).
//! - `FFMPEG_INCLUDE_DIR`: Directory containing C headers for FFmpeg compilation.
//! - `LIBCLANG_PATH`: Directory containing libclang.dll for dynamic binding generation.

use std::env;
use std::path::{Path, PathBuf};

/// Environment configuration report for codec bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformEnvironmentConfig {
    pub ffmpeg_dll_path: Option<PathBuf>,
    pub ffmpeg_include_dir: Option<PathBuf>,
    pub libclang_path: Option<PathBuf>,
    pub discovered_dlls: Vec<String>,
}

impl PlatformEnvironmentConfig {
    /// Probes environment variables and common Windows discovery directories.
    pub fn probe_from_env() -> Self {
        let dll_path = env::var("FFMPEG_DLL_PATH")
            .ok()
            .map(PathBuf::from)
            .or_else(discover_common_dll_dirs);

        let include_dir = env::var("FFMPEG_INCLUDE_DIR")
            .ok()
            .map(PathBuf::from)
            .or_else(discover_common_include_dirs);

        let libclang_path = env::var("LIBCLANG_PATH").ok().map(PathBuf::from);

        let mut discovered_dlls = Vec::new();
        if let Some(ref dir) = dll_path {
            if dir.exists() && dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.ends_with(".dll") {
                            discovered_dlls.push(name);
                        }
                    }
                }
            }
        }

        Self {
            ffmpeg_dll_path: dll_path,
            ffmpeg_include_dir: include_dir,
            libclang_path,
            discovered_dlls,
        }
    }

    /// Returns true if valid shared library DLLs are present.
    pub fn has_valid_dlls(&self) -> bool {
        let essential = ["avcodec", "avformat", "avutil", "swscale"];
        essential.iter().all(|prefix| {
            self.discovered_dlls
                .iter()
                .any(|dll| dll.starts_with(prefix))
        })
    }
}

/// Discovers common Windows locations for FFmpeg shared DLLs.
fn discover_common_dll_dirs() -> Option<PathBuf> {
    let candidates = [
        "C:\\ffmpeg\\bin",
        "C:\\tools\\ffmpeg\\bin",
        "C:\\Program Files\\ffmpeg\\bin",
    ];

    for path_str in candidates {
        let p = Path::new(path_str);
        if p.exists() && p.is_dir() {
            return Some(p.to_path_buf());
        }
    }

    // Check AppData WinGet package location
    if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        let winget_path = PathBuf::from(local_app_data)
            .join("Microsoft\\WinGet\\Packages");
        if winget_path.exists() {
            if let Ok(entries) = std::fs::read_dir(&winget_path) {
                for entry in entries.flatten() {
                    let folder_name = entry.file_name().to_string_lossy().to_string();
                    if folder_name.contains("Gyan.FFmpeg") {
                        let bin_dir = entry.path().join("bin");
                        if bin_dir.exists() {
                            return Some(bin_dir);
                        }
                    }
                }
            }
        }
    }

    None
}

/// Discovers common include directories for FFmpeg C headers.
fn discover_common_include_dirs() -> Option<PathBuf> {
    let candidates = [
        "C:\\ffmpeg\\include",
        "C:\\tools\\ffmpeg\\include",
        "C:\\vcpkg\\installed\\x64-windows\\include",
    ];

    for path_str in candidates {
        let p = Path::new(path_str);
        if p.exists() && p.is_dir() {
            return Some(p.to_path_buf());
        }
    }

    None
}
