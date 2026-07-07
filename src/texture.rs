use std::fs;
use std::path::{Path, PathBuf};

/// Find a texture file by its raw B3D path.
///
/// Search strategies (first match wins):
/// 1. `game_dir / raw_path` — preserves directory structure from B3D
/// 2. `game_dir / filename` — filename only, no directory
/// 3. `game_dir / lowercase_filename`
/// 4. Legacy Stranded II paths (`mods/Stranded II/gfx/` and `gfx/`)
pub fn find_texture(raw_path: &str, game_dir: &Path) -> Option<PathBuf> {
    let clean = raw_path.trim_start_matches(".\\").trim_start_matches("./");
    let tex_path = Path::new(clean);

    // Strategy 1: game_dir / full original path (preserves directory structure)
    let full = game_dir.join(tex_path);
    for ext in &["bmp", "jpg", "jpeg", "png", "tga"] {
        let p = full.with_extension(ext);
        if p.exists() { return Some(p); }
    }
    if full.exists() { return Some(full); }

    // Strategy 2: game_dir / filename only
    if let Some(file_name) = tex_path.file_name().and_then(|s| s.to_str()) {
        let base = game_dir.join(file_name);
        for ext in &["bmp", "jpg", "jpeg", "png", "tga"] {
            let p = base.with_extension(ext);
            if p.exists() { return Some(p); }
        }
        if base.exists() { return Some(base); }
    }

    // Strategy 3: lowercase filename in game_dir
    if let Some(stem) = tex_path.file_stem().and_then(|s| s.to_str()) {
        let lower = stem.to_lowercase();
        for ext in &["bmp", "jpg", "jpeg", "png", "tga"] {
            let p = game_dir.join(format!("{lower}.{ext}"));
            if p.exists() { return Some(p); }
        }
    }

    // Strategy 4: legacy Stranded II paths
    if let Some(stem) = tex_path.file_stem().and_then(|s| s.to_str()) {
        for dir in &[game_dir.join("mods/Stranded II/gfx"), game_dir.join("gfx")] {
            for ext in &["bmp", "jpg", "jpeg", "png", "tga"] {
                for fname in &[stem, &stem.to_lowercase()] {
                    let p = dir.join(format!("{fname}.{ext}"));
                    if p.exists() { return Some(p); }
                }
            }
        }
    }

    None
}

/// Load a B3D texture from its raw B3D path, convert to PNG, cache to disk.
///
/// The cache key is the file stem (name without extension or directory).
/// Returns `None` if the texture cannot be found or decoded.
pub fn load_texture(raw_path: &str, game_dir: &Path, tex_cache: &Path) -> Option<Vec<u8>> {
    let stem = texture_stem(raw_path);
    let png_path = tex_cache.join(format!("{stem}.png"));

    // Return cached version if it exists.
    if png_path.exists() {
        return fs::read(&png_path).ok();
    }

    // Find and convert.
    let src = find_texture(raw_path, game_dir)?;
    let img = image::open(&src).ok()?;
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).ok()?;
    let bytes = buf.into_inner();

    // Cache to disk for subsequent runs.
    if let Err(e) = fs::write(&png_path, &bytes) {
        eprintln!("warning: failed to cache texture '{stem}': {e}");
    }
    Some(bytes)
}

/// Extract the file stem (name without extension or directory) from a raw
/// B3D texture path.  E.g. `"gfx\\monkeyskin.bmp"` → `"monkeyskin"`.
pub fn texture_stem(raw: &str) -> &str {
    Path::new(raw.trim_start_matches(".\\").trim_start_matches("./"))
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
}
