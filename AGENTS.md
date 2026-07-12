# Project Context for AI Agents — b3d2glb

## What is this?

`b3d2glb` converts Blitz3D `.b3d` models to glTF 2.0 (binary `.glb` or separate `.gltf` + `.bin` + textures).

Part of the **OpenStranded** project (Stranded II remake). Original game files:
`/home/admen/Games/umu/umu-default/drive_c/Games/StrandedII/`

## License

GNU GPL v3 (see `LICENSE`). Code in `src/b3d_parser.rs` and `src/b3d_parser/utils.rs`
derived from [DotWith/b3d](https://github.com/DotWith/b3d/) (MIT OR Apache-2.0 — see `NOTICE`).

## Architecture

```
src/
  main.rs              — entry point, file discovery, dispatch
  cli.rs               — CLI argument parsing
  math.rs              — Mat4, multiplication, inversion, coordinate conversion
  b3d.rs               — B3D data extraction: joints, mesh, animation
  b3d_parser.rs        — B3D format parser (derived from DotWith/b3d)
  b3d_parser/utils.rs  — helper types (Vec2, Vec3, Vec4, Chunk)
  texture.rs           — texture lookup, PNG conversion, disk cache
  writer.rs            — glTF/GLB generation
  lib.rs               — module re-exports
  bin/dump.rs          — B3D file dump utility
```

## Key Technical Details

### Coordinate Systems
- B3D: left-handed, Y-up. glTF: right-handed, Y-up.
- Positions/normals: `swap_yz_pos` swaps Y and Z.
- Quaternions: `[w, x, y, z]` → negate Z → `[x, y, z, w]` for glTF.
- Matrices are **row-major** internally but written **column-by-column** to GLB buffer (glTF requires column-major).

### Critical: IBM Serialization
IBMs MUST be written **column-by-column** or skinned meshes render black/stretched:
```rust
for col in 0..4 {
    ibm_data.extend_from_slice(&inv[0][col].to_le_bytes());
    ibm_data.extend_from_slice(&inv[1][col].to_le_bytes());
    ibm_data.extend_from_slice(&inv[2][col].to_le_bytes());
    ibm_data.extend_from_slice(&inv[3][col].to_le_bytes());
}
```

### B3D vertex-joint mapping
- Maximum **1 bone per vertex** (weight=1.0). Unskinned vertices get joint=0/weight=0.

### Texture alpha
Determined by: B3D flags (`flags & 2` alpha, `flags & 4` color key, `blend == 1`) →
pixel fallback (`png_has_alpha()` → `alphaMode: "MASK"`).

## Library API

```rust
use b3d2glb::writer::Converter;

// B3D → GLB in memory
let glb: Vec<u8> = Converter::new("model", "/path/to/game")
    .convert_bytes(&b3d_data)?;

// With options
let glb = Converter::new("model", "/path/to/game")
    .glb(true)
    .material(0.0, 0.9)
    .color_override(1.0, 0.0, 0.0, 0.5)
    .tex_cache(&"/tmp/cache")
    .convert_bytes(&b3d_data)?;

// To file
Converter::new("model", "/path/to/game")
    .convert_to_file(input_path, output_path)?;
```

## Development

```bash
cargo build --release
cargo test

# Test with monkey.b3d (has skin, texture, animation)
cargo run --bin b3d2glb --release -- -b -o /tmp/test \
  -c /path/to/StrandedII /path/to/StrandedII/gfx/monkey.b3d

# Dump B3D structure
cargo run --bin dump --release -- /path/to/model.b3d
```
