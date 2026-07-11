# b3d2glb

Convert Blitz3D `.b3d` models to glTF 2.0 (`.glb` or `.gltf` + `.bin` + textures).

## Features

- **Mesh** — vertices, normals, UVs, triangle strips → indexed geometry
- **Normals** — auto-computed from triangles when B3D doesn't store them
- **Textures** — automatic lookup/conversion (BMP/JPG/PNG/TGA → PNG) with disk cache
- **Alpha** — detected from B3D flags or actual pixel data (fallback)
- **Materials** — per-face brush materials with diffuse colour + texture
- **Metallic/Roughness** — override via CLI or library (default: 0.0 / 1.0)
- **Base color** — override for textureless materials via CLI or library
- **Skinning** — B3D BONE chunks → glTF skin with inverse bind matrices (column-major IBM)
- **Skeletal animation** — B3D KEYS chunks → glTF animation channels with LINEAR interpolation
  - Absolute keyframe rotations (right-handed Y-up, `[x,y,z,w]` for glTF)
  - Position/rotation/scale channels per `key_flags` bitmask
  - Named clips from B3D SEQS chunks, or fallback "default" clip over all frames
  - FPS defaults to 30.0 when the file stores 0

## Coordinate conversion

B3D is left-handed Y-up; glTF is right-handed Y-up. The converter:
- Swaps Y and Z on positions and normals
- Negates Z axis on quaternions, reorders from `[w,x,y,z]` to `[x,y,z,w]`
- Leaves UVs and scale unchanged

## CLI usage

```text
b3d2glb [OPTIONS] input...

ARGS:
  input...   One or more .b3d files or directories containing .b3d files.

OPTIONS:
  -b, --glb              Write binary .glb (default: separate .gltf + .bin + textures)
  -o, --out DIR          Output directory (default: current directory)
  -c, --context DIR      Context / game root directory (texture lookup root)
  -m, --material VAL     Metallic/roughness (e.g. "0.0m0.9r" or "0.0,0.9")
  -C, --color R,G,B[,A]  Fallback base color for textureless materials
  -h, --help             Display this help and exit
```

### Examples

```bash
# Convert a single file to .glb
b3d2glb -b -o ./out -c /path/to/game model.b3d

# Convert all .b3d in a directory to .gltf + .bin + textures
b3d2glb -o ./out -c /path/to/game /path/to/game/gfx/

# Override metallic/roughness and fallback color
b3d2glb -b -o ./out -c /path/to/game -m 0.0m0.9r -C 0.8,0.8,0.8 model.b3d
```

## Library API

Add to your `Cargo.toml`:

```toml
[dependencies]
b3d2glb = { git = "https://github.com/AvengerAnubis/b3d2glb" }
```

### Quick start — B3D → GLB in memory

```rust
use b3d2glb::writer::Converter;

let b3d_data = std::fs::read("model.b3d")?;
let glb: Vec<u8> = Converter::new("model", "/path/to/game")
    .convert_bytes(&b3d_data)?;
std::fs::write("model.glb", &glb)?;
```

### Builder options

```rust
let glb = Converter::new("model", "/path/to/game")
    .glb(true)                       // output .glb (default: true)
    .material(0.0, 0.9)              // metallic, roughness
    .color_override(1.0, 0.0, 0.0, 0.5)  // fallback base color (RGBA)
    .tex_cache(&custom_cache)        // texture PNG cache directory
    .convert_bytes(&b3d_data)?;
```

### Convert to file

```rust
Converter::new("model", "/path/to/game")
    .convert_to_file(input_path, output_path)?;
```

### Low-level access

```rust
let (gltf_json, bin_buffer, images) = Converter::new("model", "/path/to/game")
    .build(&b3d_data)?;
```

Or use the individual modules directly:

```rust
use b3d2glb::b3d::{self, B3D};
use b3d2glb::writer;

let b3d = B3D::read(&bytes)?;
let mesh = b3d::collect_mesh(&b3d);
// ... see writer::build_gltf_inner docs for full API
```

## Dependencies

- Rust 2024 edition
- serde, serde_json, image, walkdir, byteorder, thiserror

## License

**GNU GPL v3** — see `LICENSE`.

Portions derived from the [`b3d`](https://github.com/DotWith/b3d/) crate by DotWith (MIT OR Apache-2.0) — see `NOTICE`.
