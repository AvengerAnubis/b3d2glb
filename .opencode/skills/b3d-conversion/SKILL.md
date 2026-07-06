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
fn neg_z_quat(q: [f32; 4]) -> [f32; 4] {
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

## Original game location (Stranded II)

```
/home/admen/Games/umu/umu-default/drive_c/Games/StrandedII/
```

Models are under `<game>/mods/Stranded II/gfx/`, textures in the same dir,
named as `.bmp` files referenced by B3D brush texture IDs.
