# Project context for AI agents

## What this project does

`b3d2glb` converts Blitz3D `.b3d` model files to glTF 2.0 format (either
binary `.glb` or separate `.gltf` + `.bin` + texture files).

It is part of a larger **Stranded II remake** project — the original game
files are located at:

```
/home/admen/Games/umu/umu-default/drive_c/Games/StrandedII/
```

## Architecture

```
src/
  main.rs     — entry point, file discovery, dispatch
  cli.rs      — CLI argument parsing (--out, --context, --glb, --help)
  math.rs     — Mat4 type, matrix ops (mul, inverse), coord conversion
  b3d.rs      — B3D data extraction (joints, mesh, animation clips)
  texture.rs  — texture lookup, PNG conversion, disk caching
  writer.rs   — glTF/GLB output generation
```

## Coordinate systems

- B3D: left-handed Y-up. glTF: right-handed Y-up.
- Positions/normals: swap Y and Z (`swap_yz_pos`).
- Quaternions: `[w, x, y, z]` → negate Z component → `[x, y, z, w]` for glTF.
- All matrices are row-major (`m[row][col]`) internally.

## Important technical details

### Matrix convention

`b3d_to_mat4` returns a row-major TRS matrix with translation in `m[3][0..2]`.
This is *transposed* relative to the standard column-major convention
(translation in `m[0..2][3]`).

`compute_world_matrix` multiplies parent × local: `world = parent * local`.
Because `b3d_to_mat4` returns the transpose of the standard matrix, the
multiplication order appears correct when following the B3D hierarchy.

### IBM (inverse bind matrix) serialization

Inverse bind matrices are written to the GLB binary buffer **column-by-column**
for glTF's column-major layout:

```rust
for col in 0..4 {
    ibm_data.extend_from_slice(&inv[0][col].to_le_bytes());
    ibm_data.extend_from_slice(&inv[1][col].to_le_bytes());
    ibm_data.extend_from_slice(&inv[2][col].to_le_bytes());
    ibm_data.extend_from_slice(&inv[3][col].to_le_bytes());
}
```

The old implementation wrote row-by-row, which produced transposed IBMs and
caused stretched/black renders in Bevy.

### B3D vertex-joint mapping

B3D stores at most *one* bone per vertex (weight=1.0). Unskinned vertices
receive joint=0/weight=0 (4-wide JOINT/WEIGHT vectors padded with zeros).

## Development

```bash
# Build
cargo build --release

# Test with monkey model
cargo run --release -- -b -o /tmp/out -c /path/to/StrandedII ./monkey.b3d
```
