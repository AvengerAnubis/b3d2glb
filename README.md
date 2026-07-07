# b3d2glb

Convert Blitz3D `.b3d` models to glTF 2.0 (`.glb` or `.gltf` + `.bin` + textures).

## Features

- **Mesh** — vertices, normals, UVs, triangle strips → indexed geometry
- **Textures** — automatic lookup/conversion (BMP/JPG/PNG/TGA → PNG)
- **Materials** — per-face brush materials with diffuse colour + texture
- **Skinning** — B3D BONE chunks → glTF skin with inverse bind matrices (column-major IBM)
- **Skeletal animation** — B3D KEYS chunks → glTF animation channels with LINEAR interpolation
  - Absolute keyframe rotations (right-handed Y-up, `[x,y,z,w]` for glTF)
  - Position/rotation/scale channels per `key_flags` bitmask
  - Named clips from B3D SEQS chunks, or fallback "default" clip over all frames
  - FPS defaults to 30.0 when the file stores 0

## Coordinate conversion

B3D is left-handed Y-up; glTF is right-handed Y-up. The converter:
- Swaps Y and Z on positions and normals
- Swaps the Y and Z components of quaternion rotation axes (`swap_yz_quat`)
- Leaves scale unchanged

## Usage

```text
b3d2glb [OPTIONS] input...

ARGS:
  input...   One or more .b3d files or directories containing .b3d files.

OPTIONS:
  -o, --out DIR      Output directory (default: current directory)
  -c, --context DIR  Context / game root directory (texture lookup root)
  -b, --glb          Write binary .glb instead of separate .gltf + .bin + textures
  -h, --help         Display this help and exit
```

## Examples

```bash
# Convert a single file to .glb
b3d2glb -b -o ./out -c /path/to/game model.b3d

# Convert all .b3d in a directory to .gltf + .bin + textures
b3d2glb -o ./out -c /path/to/game /path/to/game/gfx/
```

## Dependencies

- Rust 2024 edition
- [b3d](https://crates.io/crates/b3d) crate for B3D parsing
- serde, serde_json, image, walkdir

## License

GPL-2.0
