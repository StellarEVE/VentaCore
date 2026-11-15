//! CLI command: Purge all disposable media and render cache directories.

use std::path::PathBuf;
use ventacore_media::MediaCache;
use ventacore_render::RenderCache;

pub fn execute_purge_cache(custom_path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let media_dir = custom_path
        .clone()
        .unwrap_or_else(MediaCache::default_dir);
    let media_cache = MediaCache::new(&media_dir)?;
    let freed_bytes = media_cache.purge()?;

    let render_dir = custom_path
        .map(|p| p.join("rendered"))
        .unwrap_or_else(RenderCache::default_dir);
    let render_cache = RenderCache::new(&render_dir)?;
    let freed_render_files = render_cache.purge()?;

    println!("============================================================");
    println!(" VentaCore Cache Purged (Charter Rule 9)");
    println!("============================================================");
    println!("Media Cache Freed:  {:.2} MB ({})", freed_bytes as f64 / (1024.0 * 1024.0), media_dir.display());
    println!("Render Cache Purged: {} cached frames ({})", freed_render_files, render_dir.display());
    println!("============================================================");

    Ok(())
}
