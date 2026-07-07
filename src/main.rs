mod cli;
mod math;
mod b3d;
mod texture;
mod writer;

use crate::b3d::B3D;
use std::path::Path;
use std::fs;
use walkdir::WalkDir;

fn main() {
    let args = match cli::parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    // Prepare output directories.
    let tex_cache = args.out_dir.join("textures");
    if let Err(e) = fs::create_dir_all(&tex_cache) {
        eprintln!("error: cannot create textures dir: {e}");
        std::process::exit(1);
    }

    // Gather all input .b3d files.
    let mut b3d_files: Vec<std::path::PathBuf> = Vec::new();
    for input in &args.inputs {
        if input.is_file() {
            if input.extension().and_then(|s| s.to_str()) == Some("b3d") {
                b3d_files.push(input.clone());
            } else {
                eprintln!("warning: skipping non-.b3d file: {input:?}");
            }
        } else if input.is_dir() {
            for entry in WalkDir::new(input).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("b3d") {
                    b3d_files.push(path.to_path_buf());
                }
            }
        } else {
            eprintln!("warning: input does not exist: {input:?}");
        }
    }

    if b3d_files.is_empty() {
        eprintln!("error: no .b3d files found");
        std::process::exit(1);
    }

    // Derive context directory if not provided.
    let context_dir = args.context_dir.as_ref().map(|p| p.as_path())
        .or_else(|| derive_context(&b3d_files[0]))
        .map(|p| p.to_path_buf());

    if context_dir.is_none() && b3d_files.iter().any(|p| needs_textures(p)) {
        eprintln!("error: cannot derive context directory; provide --context");
        std::process::exit(1);
    }

    let ctx = context_dir.as_deref().unwrap_or_else(|| Path::new("."));

    // Process each file.
    let mut count = 0u32;
    let mut errors = 0u32;
    let mut skips = 0u32;

    for path in &b3d_files {
        let stem = path.file_stem().unwrap().to_str().unwrap_or("model");
        let base_name = if args.glb {
            format!("{stem}.glb")
        } else {
            format!("{stem}.gltf")
        };
        let out_path = args.out_dir.join(&base_name);

        eprint!("  {stem} ... ");

        match convert_one(path, &out_path, ctx, &tex_cache, args.glb) {
            Ok(true) => {
                eprintln!("OK");
                count += 1;
            }
            Ok(false) => {
                eprintln!("SKIP (no mesh)");
                skips += 1;
            }
            Err(e) => {
                eprintln!("FAIL: {e}");
                errors += 1;
            }
        }
    }

    let total = count + errors + skips;
    eprintln!("\nDone: {count} converted, {skips} skipped, {errors} errors (from {total} files)");
}

fn convert_one(
    in_path: &Path,
    out_path: &Path,
    game_dir: &Path,
    tex_cache: &Path,
    glb_mode: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    let data = fs::read(in_path)?;
    let b3d_parsed = B3D::read(&data)
        .map_err(|e| format!("parse error: {e}"))?;

    let vcount = b3d_parsed.node.mesh.vertices.vertices.len();
    if vcount == 0 {
        return Ok(false);
    }

    let model_name = in_path.file_stem().unwrap_or_default().to_str().unwrap_or("model");

    let mut joints = Vec::new();
    let mut vertex_joint: Vec<Option<(usize, f32)>> = vec![None; vcount];
    b3d::collect_joints(&b3d_parsed.node, None, &mut joints, &mut vertex_joint, vcount, true);

    let mut mesh = b3d::collect_mesh(&b3d_parsed);
    for (vi, j) in vertex_joint.iter().enumerate() {
        mesh.skin[vi] = j.as_ref().map(|(ji, w)| b3d::BoneWeight {
            joint_idx: *ji as u32,
            weight: *w,
        });
    }

    let clips = b3d::collect_anims(&b3d_parsed.node);

    if glb_mode {
        writer::write_glb(&mesh, &joints, &clips, &b3d_parsed.textures, &b3d_parsed.brushes, model_name, game_dir, tex_cache, out_path)?;
    } else {
        writer::write_gltf_separate(&mesh, &joints, &clips, &b3d_parsed.textures, &b3d_parsed.brushes, model_name, game_dir, tex_cache, out_path)?;
    }

    Ok(true)
}

fn derive_context(path: &Path) -> Option<&Path> {
    if path.is_file() {
        path.parent().and_then(|p| p.parent())
    } else {
        path.parent()
    }
}

fn needs_textures(_path: &Path) -> bool {
    // Conservative: assume any .b3d may reference textures.
    true
}
