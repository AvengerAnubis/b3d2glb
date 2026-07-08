---
name: b3d-conversion
description: Use when converting B3D models to glTF/GLB, debugging skinned mesh rendering, matrix conventions, or IBM transpose issues.
---

# B3D to glTF conversion

## Root cause: IBM transposition (the "black/stretched mesh" bug)

The most critical bug in B3D-to-GLB converters: **IBMs written row-major but
read as column-major** by the glTF loader.

**The fix:** write column-by-column:

```rust
// CORRECT — glTF expects column-major
for col in 0..4 {
    ibm_data.extend_from_slice(&inv[0][col].to_le_bytes());
    ibm_data.extend_from_slice(&inv[1][col].to_le_bytes());
    ibm_data.extend_from_slice(&inv[2][col].to_le_bytes());
    ibm_data.extend_from_slice(&inv[3][col].to_le_bytes());
}
```

**The wrong way** (produces transposed IBMs → garbage joint matrices):

```rust
// WRONG — writes row-major data that from_cols_array_2d reads as columns
for row in 0..4 {
    ibm_data.extend_from_slice(&inv[row][0..4]);  // No!
}
```

**Why it breaks:** The glTF loader reads raw floats in order, then constructs
`Mat4::from_cols_array_2d(&[[f32; 4]; 4])`. If the buffer has `row0[col0,
col1, col2, col3]`, it becomes `col0 = row0`, which is a transpose.

## Coordinate systems

|            | B3D (Blitz3D) | glTF      |
|------------|---------------|-----------|
| Handedness | Left-handed Y-up | Right-handed Y-up |
| Position   | `[x, y, z]`  | `[x, y, -z]` |
| Normal     | `[x, y, z]`  | `[x, y, -z]` |
| Quaternion | `[w, x, y, z]` (left-handed) | `[x, y, z, w]` (right-handed, negate Z axis) |
| UV         | `[u, v]`     | `[u, 1-v]` |

### Quaternion conversion

B3D quat `[w, x, y, z]` → negate rotation-axis Z → reorder to `[x, y, z, w]`:

```rust
fn swap_yz_quat(q: [f32; 4]) -> [f32; 4] {
    [q[0], q[1], q[2], -q[3]]  // negate Z component of axis
}
// glTF node rotation: [q[1], q[2], q[3], q[0]]  = [x, y, z, w]
```

## Matrix conventions

### Row-major (m[row][col]) — used throughout

`b3d_to_mat4` produces a **transposed** matrix relative to the standard
column-major convention. Translation is in `m[3][0..2]` (last row) instead of
`m[0..2][3]` (last column).

The `mat4_mul(a, b)` computes `a * b` in row-major, which equals `(b^T *
a^T)^T` in column-major — the multiplication semantics are correct for the
chosen convention.

### `compute_world_matrix`

Recursively computes `world = parent_world * local`. Because local is
row-major (transposed), this produces the correct world transform for the B3D
hierarchy.

## B3D skinning specifics

- B3D stores **at most 1 bone per vertex** (weight always 1.0).
- Unskinned vertices get joint=0, weight=0 in the 4-wide glTF attributes.
- Joint hierarchy is the B3D node tree — the mesh lives on node 0, bones
  reference vertices by index.

## Vertex winding

B3D triangles are CW (clockwise); glTF uses CCW. The converter flips:
```rust
indices.push(tri[0]);
indices.push(tri[2]);  // swapped
indices.push(tri[1]);
```

## Testing checklist for skinning

1. Convert monkey.b3d with `--glb`
2. Check vertex attributes: `monkey.glb` must have `JOINTS_0` and `WEIGHTS_0`
3. At bind pose (no animation), skinned mesh must match non-skinned mesh
   vertex-for-vertex
4. `joint_matrix[i] = GT(joint[i]) * ibm[i]` must equal `spawn_transform`
5. Any stretched/black render means IBMs are wrong

## Texture alpha transparency

`alphaMode` is determined in this order:

1. **B3D texture flags**: `flags & 2` (alpha channel), `flags & 4` (color key),
   `blend == 1` (alpha blend).
2. **Pixel fallback**: if flags don't indicate alpha, the PNG bytes are
   scanned for any non-opaque pixel (`png_has_alpha()` in `texture.rs`).
3. If either check passes → `alphaMode: "MASK"` + `alphaCutoff: 0.5`.

**Common pitfall**: many B3D files from Stranded II don't set transparency
flags. The pixel fallback is essential.

## Missing normals (black model in Bevy)

B3D `Verts.flags & 1 == 0` means per-vertex normals are not stored. The
converter computes them from triangle faces in `compute_normals()` (b3d.rs):

1. Iterate triangles, compute face normal via cross product of two edges
2. Accumulate face normal into all three vertices (area-weighted)
3. Normalize all vertex normals at the end

Computation happens **after** coordinate conversion (Z negated, CW→CCW
winding flipped) so the result is correct for glTF.

## CLI material/color overrides

```bash
# Set metallic=0.0, roughness=0.9 (dot-syntax):
b3d2glb -m 0.0m0.9r -o out model.b3d

# Same with comma syntax:
b3d2glb -m 0.0,0.9 -o out model.b3d

# Override fallback base color (RGBA):
b3d2glb -C 0.5,0.5,0.5,1.0 -o out model.b3d

# Override fallback base color (RGB, alpha defaults to 1.0):
b3d2glb -C 1.0,0.0,0.0 -o out model.b3d
```

## Texture search strategy

`find_texture()` in `texture.rs` tries paths in order:

1. `context_dir / raw_path` (preserves B3D directory structure)
2. `context_dir / filename` (filename only from B3D path)
3. `context_dir / lowercase_filename`
4. Legacy: `context_dir / mods/Stranded II/gfx/` and `context_dir / gfx/`

Each strategy tries extensions: `.bmp`, `.jpg`, `.jpeg`, `.png`, `.tga`.

## Library API (`Converter` builder)

The `Converter` struct in `writer.rs` is the high-level library API:

```rust
use b3d2glb::writer::Converter;

// Minimal: B3D bytes → GLB bytes
let glb: Vec<u8> = Converter::new("model_name", "/path/to/game")
    .convert_bytes(&b3d_bytes)?;

// Builder options:
let glb = Converter::new("model", "/path/to/game")
    .glb(true)                          // output .glb (default: true)
    .material(0.0, 0.9)                 // metallic, roughness
    .color_override(1.0, 0.0, 0.0, 0.5) // fallback base colour
    .tex_cache(&"/tmp/cache")           // texture PNG cache
    .convert_bytes(&b3d_data)?;

// Write directly to file:
Converter::new("model", "/path/to/game")
    .convert_to_file(input_path, output_path)?;

// Low-level access (JSON, buffer, images):
let (json, bin, images) = Converter::new("model", "/path/to/game")
    .build(&b3d_data)?;
```

### Public module functions

| Module | Function | Purpose |
|--------|----------|---------|
| `writer` | `build_gltf_inner(...)` | Parsed B3D → `(JSON, Buffer, Images)` |
| `writer` | `pad_to_4(data)` | 4-byte alignment |
| `writer` | `pad_to_4_in_place(data)` | 4-byte alignment (in-place) |
| `b3d` | `B3D::read(&bytes)` | Parse B3D file |
| `b3d` | `collect_mesh(&b3d)` | Extract mesh data |
| `b3d` | `collect_joints(&b3d)` | Extract bone data |
| `b3d` | `collect_anims(&b3d)` | Extract animation data |

### Testing the library API

```bash
# Run all tests including API tests
cargo test

# Specific API tests
cargo test api_
```

There are 8 integration tests in `tests/e2e.rs` covering the `Converter` API:
`api_convert_bytes_valid_glb`, `api_convert_bytes_has_skin`,
`api_convert_to_file_writes_glb`, `api_build_returns_gltf_data`,
`api_convert_bytes_with_material_override`,
`api_convert_bytes_with_color_override`,
`api_convert_empty_data_returns_error`,
`api_convert_invalid_data_returns_error`.

## Original game location (Stranded II)

```
/home/admen/Games/umu/umu-default/drive_c/Games/StrandedII/
```

Models are under `<game>/mods/Stranded II/gfx/`, textures in the same dir,
named as `.bmp` files referenced by B3D brush texture IDs.
