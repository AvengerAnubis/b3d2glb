use std::fs;
use std::path::{Path, PathBuf};

/// Find a texture file by name in common B3D game directories.
pub fn find_texture(name: &str, game_dir: &Path) -> Option<PathBuf> {
    let search_dirs = [
        game_dir.join("mods/Stranded II/gfx"),
        game_dir.join("gfx"),
    ];
    for dir in &search_dirs {
        for ext in &["bmp", "jpg", "jpeg", "png", "tga"] {
            for fname in &[name, &name.to_lowercase()] {
                let p = dir.join(format!("{fname}.{ext}"));
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// Load a B3D texture, convert to PNG in memory, optionally caching to disk.
///
/// Returns `None` if the texture cannot be found or decoded.
pub fn load_texture(name: &str, game_dir: &Path, tex_cache: &Path) -> Option<Vec<u8>> {
    let png_path = tex_cache.join(format!("{name}.png"));

    // Return cached version if it exists.
    if png_path.exists() {
        return fs::read(&png_path).ok();
    }

    // Find and convert.
    let src = find_texture(name, game_dir)?;
    let img = image::open(&src).ok()?;
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).ok()?;
    let bytes = buf.into_inner();

    // Cache to disk for subsequent runs.
    let _ = fs::write(&png_path, &bytes);
    Some(bytes)
}

/// Strip texture path down to its stem.
pub fn texture_stem(raw: &str) -> &str {
    Path::new(raw.trim_start_matches(".\\").trim_start_matches("./"))
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
}
