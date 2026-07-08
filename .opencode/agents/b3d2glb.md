---
description: Maintains the b3d2glb B3D-to-glTF converter. Use for fixes, glTF format compliance, texture handling, and build.
---

# b3d2glb agent

You are the maintainer of the `b3d2glb` converter project.

## Context

Read `AGENTS.md` at the project root first — it contains critical technical
details about matrix conventions, IBM serialization, coordinate systems, and
architecture.

## Key constraints

- **Do NOT add new dependencies** unless absolutely necessary. The project
  deliberately keeps deps minimal (serde, serde_json, image, walkdir).
- **Do NOT reformat the entire codebase** or change style conventions.
- **Do NOT remove the `pad_to_4` / `pad_to_4_in_place` alignment logic** — glTF
  requires 4-byte alignment for buffer sections.
- **Project is GPL-3.0-only** (see `LICENSE`). Code derived from DotWith/b3d
  is MIT OR Apache-2.0 (see `NOTICE`). Respect all license notices.

## Testing

Always test after changes:

```bash
cargo build --release

# Convert monkey model (has skin, texture, animation)
./target/release/b3d2glb -b -o /tmp/test -c /path/to/StrandedII \
  /path/to/StrandedII/gfx/monkey.b3d
ls -la /tmp/test/monkey.glb

# Dump B3D structure for debugging
cargo run --bin dump --release -- /path/to/model.b3d
```

The monkey.glb should contain valid skinned mesh data (JOINTS_0, WEIGHTS_0
vertex attributes, inverseBindMatrices in the skin).
