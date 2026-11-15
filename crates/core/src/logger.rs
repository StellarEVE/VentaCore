//! Panic-safe line-at-a-time logger helper for VentaCore.
//!
//! Conforms to Stage 0 requirements:
//! - Appends one line at a time.
//! - Opens and closes the file for every line.
//! - Flushes and syncs data to disk immediately.
//! - Guaranteed to NEVER panic, even on invalid paths or I/O errors.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

/// Append a single line of text to a log file.
///
/// This function opens the file in append mode (creating it if necessary),
/// writes the line followed by a newline, flushes, syncs to disk, and closes
/// the file when it goes out of scope.
///
/// It will NEVER panic under any circumstance.
pub fn append_log_line<P: AsRef<Path>>(path: P, message: &str) {
    let path = path.as_ref();
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{}", message);
            let _ = file.flush();
            let _ = file.sync_all();
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_logger_never_panics_on_invalid_path() {
        // Writing to an invalid path must not panic
        append_log_line("///::invalid::path::???/log.txt", "test line");
    }

    #[test]
    fn test_logger_writes_1000_lines_and_persists() {
        let temp_dir = std::env::temp_dir();
        let log_path = temp_dir.join(format!("ventacore_test_log_{}.txt", std::process::id()));

        // Ensure clean state
        if log_path.exists() {
            let _ = fs::remove_file(&log_path);
        }

        // Write 1,000 lines
        for i in 0..1000 {
            append_log_line(&log_path, &format!("Step {:04}: VentaCore log line event", i));
        }

        // Simulate abrupt stop by directly reading what is physically on disk
        let content = fs::read_to_string(&log_path).expect("Log file must exist and be readable");
        let lines: Vec<&str> = content.lines().collect();

        assert_eq!(
            lines.len(),
            1000,
            "All 1,000 log lines must be present and accounted for"
        );
        assert_eq!(lines[0], "Step 0000: VentaCore log line event");
        assert_eq!(lines[999], "Step 0999: VentaCore log line event");

        // Clean up
        let _ = fs::remove_file(&log_path);
    }
}
