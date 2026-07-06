use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::b3d::{AnimClip, JointInfo, MeshData, compute_world_matrix};
use crate::math::{mat4_inverse, neg_z_pos, neg_z_quat};
use crate::texture::{load_texture, texture_stem};

use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Write a binary .glb file with all data embedded.
pub fn write_glb(
    mesh: &MeshData,
    joints: &[JointInfo],
    clips: &[AnimClip],
    textures: &[b3d::Texture],
    brushes: &[b3d::Brush],
    model_name: &str,
    game_dir: &Path,
    tex_cache: &Path,
    out_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let (root, bin, _) = build_gltf_inner(mesh, joints, clips, textures, brushes, model_name, game_dir, tex_cache, true)?;

    let json_str = serde_json::to_string(&root)?;
    let json_padded = pad_to_4(json_str.as_bytes());

    const HEADER_SIZE: u32 = 12;
    let total_len = HEADER_SIZE + 8 + json_padded.len() as u32 + 8 + bin.len() as u32;

    let mut glb = Vec::with_capacity(total_len as usize);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&total_len.to_le_bytes());
    glb.extend_from_slice(&(json_padded.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&json_padded);
    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"BIN\0");
    glb.extend_from_slice(&bin);

    fs::write(out_path, glb)?;
    Ok(())
}

/// Write a .gltf file plus a separate .bin and texture files.
pub fn write_gltf_separate(
    mesh: &MeshData,
    joints: &[JointInfo],
    clips: &[AnimClip],
    textures: &[b3d::Texture],
    brushes: &[b3d::Brush],
    model_name: &str,
    game_dir: &Path,
    tex_cache: &Path,
    out_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let (mut root, bin, image_infos) = build_gltf_inner(mesh, joints, clips, textures, brushes, model_name, game_dir, tex_cache, false)?;

    // Write binary buffer.
    let bin_path = out_path.with_extension("bin");
    fs::write(&bin_path, &bin)?;

    // Point the buffer to the external .bin file.
    let bin_name = bin_path.file_name().unwrap().to_str().unwrap().to_string();
    if let Some(bufs) = root.get_mut("buffers").and_then(|v| v.as_array_mut()) {
        if let Some(buf) = bufs.get_mut(0).and_then(|v| v.as_object_mut()) {
            buf.insert("uri".into(), json!(bin_name));
        }
    }

    // Build separate image files + URI-based JSON.
    if !image_infos.is_empty() {
        let tex_dir = out_path.parent().unwrap_or(Path::new(".")).join("textures");
        let (images, gltf_textures) = build_image_uris(&image_infos, &tex_dir, model_name);
        root["images"] = json!(images);
        root["textures"] = json!(gltf_textures);
    }

    let json_str = serde_json::to_string_pretty(&root)?;
    fs::write(out_path, json_str)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Internal: build the glTF JSON root + binary buffer
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn build_gltf_inner(
    mesh: &MeshData,
    joints: &[JointInfo],
    clips: &[AnimClip],
    b3d_textures: &[b3d::Texture],
    brushes: &[b3d::Brush],
    model_name: &str,
    game_dir: &Path,
    tex_cache: &Path,
    _embed_images: bool,
) -> Result<(Value, Vec<u8>, Vec<ImageInfo>), Box<dyn std::error::Error>> {
    let vc = mesh.positions.len();
    let has_skin = !joints.is_empty() && mesh.skin.iter().any(|s| s.is_some());

    // --- materials ---------------------------------------------------------
    let brush_to_mat = build_brush_map(mesh);
    let (materials, image_infos, fallback_mat) = build_materials(
        &brush_to_mat, b3d_textures, brushes, model_name, game_dir, tex_cache,
    )?;

    // --- vertex buffer -----------------------------------------------------
    let mut bin = Vec::new();

    let pos_off = push_positions(&mut bin, &mesh.positions);
    let norm_off = push_normals(&mut bin, &mesh.normals);
    let uv_off = push_uvs(&mut bin, &mesh.uvs);

    let (joints_off, weights_off) = if has_skin {
        let jo = push_joints(&mut bin, &mesh.skin);
        let wo = push_weights(&mut bin, &mesh.skin);
        (Some(jo), Some(wo))
    } else {
        (None, None)
    };

    let idx_off = push_indices(&mut bin, &mesh.tri_groups);
    pad_to_4_in_place(&mut bin);

    let vc_u32 = vc as u32;

    // --- buffer views ------------------------------------------------------
    let mut bvs: Vec<Value> = vec![
        make_bv(0, pos_off, vc_u32 * 12, 12, 34962),
        make_bv(0, norm_off, vc_u32 * 12, 12, 34962),
        make_bv(0, uv_off, vc_u32 * 8, 8, 34962),
    ];

    let joints_bv = has_skin.then(|| {
        let i = bvs.len() as u32;
        bvs.push(make_bv(0, joints_off.unwrap(), vc_u32 * 8, 0, 34962));
        i
    });
    let weights_bv = has_skin.then(|| {
        let i = bvs.len() as u32;
        bvs.push(make_bv(0, weights_off.unwrap(), vc_u32 * 16, 0, 34962));
        i
    });

    let total_indices: u32 = mesh.tri_groups.iter().map(|t| t.indices.len() as u32).sum();
    let idx_bv = bvs.len() as u32;
    bvs.push(make_bv(0, idx_off, total_indices * 4, 0, 34963));

    // --- accessors ---------------------------------------------------------
    let mut accs: Vec<Value> = Vec::new();

    let (pos_min, pos_max) = calc_bounds(&mesh.positions);
    accs.push(json!({
        "bufferView": 0, "componentType": 5126, "count": vc_u32, "type": "VEC3",
        "min": [pos_min[0], pos_min[1], pos_min[2]],
        "max": [pos_max[0], pos_max[1], pos_max[2]],
    }));
    accs.push(json!({"bufferView": 1, "componentType": 5126, "count": vc_u32, "type": "VEC3"}));
    accs.push(json!({"bufferView": 2, "componentType": 5126, "count": vc_u32, "type": "VEC2"}));

    let joints_acc = has_skin.then(|| {
        let i = accs.len() as u32;
        accs.push(json!({"bufferView": joints_bv.unwrap(), "componentType": 5123, "count": vc_u32, "type": "VEC4"}));
        i
    });
    let weights_acc = has_skin.then(|| {
        let i = accs.len() as u32;
        accs.push(json!({"bufferView": weights_bv.unwrap(), "componentType": 5126, "count": vc_u32, "type": "VEC4"}));
        i
    });

    let base_idx_acc = accs.len() as u32;
    for (i, tg) in mesh.tri_groups.iter().enumerate() {
        let byte_start: u32 = mesh.tri_groups.iter().take(i).map(|t| t.indices.len() as u32 * 4).sum();
        accs.push(json!({
            "bufferView": idx_bv, "byteOffset": byte_start,
            "componentType": 5125, "count": tg.indices.len() as u32, "type": "SCALAR",
        }));
    }

    // --- nodes & scene -----------------------------------------------------
    let (gltf_nodes, scene_nodes) = build_node_hierarchy(joints, has_skin);

    // --- skin (IBM) --------------------------------------------------------
    let skins: Vec<Value> = if has_skin {
        let ibm_off = bin.len();
        for j_idx in 0..joints.len() {
            let world = compute_world_matrix(joints, j_idx);
            let inv = mat4_inverse(&world);
            for col in 0..4 {
                bin.extend_from_slice(&inv[0][col].to_le_bytes());
                bin.extend_from_slice(&inv[1][col].to_le_bytes());
                bin.extend_from_slice(&inv[2][col].to_le_bytes());
                bin.extend_from_slice(&inv[3][col].to_le_bytes());
            }
        }
        pad_to_4_in_place(&mut bin);

        let ibm_bv = bvs.len() as u32;
        bvs.push(make_bv(0, ibm_off, (joints.len() * 64) as u32, 0, 34962));

        let ibm_acc = accs.len() as u32;
        accs.push(json!({
            "bufferView": ibm_bv, "componentType": 5126,
            "count": joints.len() as u32, "type": "MAT4",
        }));

        let joint_ids: Vec<u32> = (0..joints.len() as u32).collect();
        vec![json!({
            "inverseBindMatrices": ibm_acc,
            "joints": joint_ids,
            "skeleton": 0,
        })]
    } else {
        vec![]
    };

    // --- primitives --------------------------------------------------------
    let mut primitives = Vec::new();
    for (i, tg) in mesh.tri_groups.iter().enumerate() {
        let mat = brush_to_mat.get(&tg.brush_id).copied().unwrap_or(fallback_mat);
        let mut prim = json!({
            "attributes": {"POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2},
            "indices": base_idx_acc + i as u32,
            "material": mat,
        });

        if let (Some(ja), Some(wa)) = (joints_acc, weights_acc) {
            if let Some(attrs) = prim.pointer_mut("/attributes").and_then(|v| v.as_object_mut()) {
                attrs.insert("JOINTS_0".into(), json!(ja));
                attrs.insert("WEIGHTS_0".into(), json!(wa));
            }
        }

        primitives.push(prim);
    }

    let meshes = vec![json!({"primitives": primitives})];

    // --- animations --------------------------------------------------------
    let anim_acc_offset = accs.len() as u32;
    let animations = build_animations(clips, joints, anim_acc_offset, &mut bvs, &mut accs, &mut bin);

    // --- textures (embedded) -----------------------------------------------
    let (images, gltf_textures) = if !image_infos.is_empty() {
        build_image_json(&image_infos, &mut bvs, &mut bin)
    } else {
        (vec![], vec![])
    };

    // --- assemble root -----------------------------------------------------
    let mut root = json!({
        "asset": {"version": "2.0", "generator": "b3d2glb"},
        "scene": 0,
        "scenes": [{"nodes": scene_nodes}],
        "nodes": gltf_nodes,
        "meshes": meshes,
        "accessors": accs,
        "bufferViews": bvs,
        "buffers": [{"byteLength": bin.len() as u32}],
        "materials": materials,
    });

    if !skins.is_empty() { root["skins"] = json!(skins); }
    if !animations.is_empty() { root["animations"] = json!(animations); }
    if !images.is_empty() { root["images"] = json!(images); }
    if !gltf_textures.is_empty() { root["textures"] = json!(gltf_textures); }

    Ok((root, bin, image_infos))
}

// ---------------------------------------------------------------------------
// Buffer helpers
// ---------------------------------------------------------------------------

fn make_bv(buffer: u32, offset: usize, length: u32, stride: u32, target: u32) -> Value {
    let mut o = json!({
        "buffer": buffer,
        "byteOffset": offset,
        "byteLength": length,
        "target": target,
    });
    if stride > 0 {
        o["byteStride"] = json!(stride);
    }
    o
}

fn pad_to_4(data: &[u8]) -> Vec<u8> {
    let mut v = data.to_vec();
    while v.len() % 4 != 0 { v.push(0x20); }
    v
}

fn pad_to_4_in_place(data: &mut Vec<u8>) {
    while data.len() % 4 != 0 { data.push(0); }
}

// ---------------------------------------------------------------------------
// Vertex data writers
// ---------------------------------------------------------------------------

fn push_positions(bin: &mut Vec<u8>, positions: &[[f32; 3]]) -> usize {
    let off = bin.len();
    for p in positions {
        let c = neg_z_pos(*p);
        bin.extend_from_slice(&c[0].to_le_bytes());
        bin.extend_from_slice(&c[1].to_le_bytes());
        bin.extend_from_slice(&c[2].to_le_bytes());
    }
    off
}

fn push_normals(bin: &mut Vec<u8>, normals: &[[f32; 3]]) -> usize {
    let off = bin.len();
    for n in normals {
        let c = neg_z_pos(*n);
        bin.extend_from_slice(&c[0].to_le_bytes());
        bin.extend_from_slice(&c[1].to_le_bytes());
        bin.extend_from_slice(&c[2].to_le_bytes());
    }
    off
}

fn push_uvs(bin: &mut Vec<u8>, uvs: &[[f32; 2]]) -> usize {
    let off = bin.len();
    for uv in uvs {
        bin.extend_from_slice(&uv[0].to_le_bytes());
        bin.extend_from_slice(&uv[1].to_le_bytes());
    }
    off
}

fn push_joints(bin: &mut Vec<u8>, skin: &[Option<crate::b3d::BoneWeight>]) -> usize {
    let off = bin.len();
    for s in skin {
        let j = s.as_ref().map(|b| b.joint_idx as u16).unwrap_or(0);
        bin.extend_from_slice(&j.to_le_bytes());
        bin.extend_from_slice(&0u16.to_le_bytes());
        bin.extend_from_slice(&0u16.to_le_bytes());
        bin.extend_from_slice(&0u16.to_le_bytes());
    }
    off
}

fn push_weights(bin: &mut Vec<u8>, skin: &[Option<crate::b3d::BoneWeight>]) -> usize {
    let off = bin.len();
    for s in skin {
        let w = s.as_ref().map(|b| b.weight).unwrap_or(0.0);
        bin.extend_from_slice(&w.to_le_bytes());
        bin.extend_from_slice(&0.0f32.to_le_bytes());
        bin.extend_from_slice(&0.0f32.to_le_bytes());
        bin.extend_from_slice(&0.0f32.to_le_bytes());
    }
    off
}

fn push_indices(bin: &mut Vec<u8>, tri_groups: &[crate::b3d::TriGroup]) -> usize {
    let off = bin.len();
    for tg in tri_groups {
        for &i in &tg.indices {
            bin.extend_from_slice(&i.to_le_bytes());
        }
    }
    off
}

// ---------------------------------------------------------------------------
// Materials
// ---------------------------------------------------------------------------

fn build_brush_map(mesh: &MeshData) -> HashMap<u32, usize> {
    let mut map = HashMap::new();
    map.insert(u32::MAX, 0);
    let mut sorted: Vec<u32> = mesh.tri_groups.iter().map(|t| t.brush_id).collect();
    sorted.sort();
    sorted.dedup();
    for (idx, bid) in sorted.iter().enumerate() {
        map.insert(*bid, idx);
    }
    map
}

struct ImageInfo {
    mime: String,
    data: Vec<u8>,
}

#[allow(clippy::too_many_arguments)]
fn build_materials(
    brush_to_mat: &HashMap<u32, usize>,
    b3d_textures: &[b3d::Texture],
    brushes: &[b3d::Brush],
    model_name: &str,
    game_dir: &Path,
    tex_cache: &Path,
) -> Result<(Vec<Value>, Vec<ImageInfo>, usize), Box<dyn std::error::Error>> {
    let mut materials: Vec<Value> = Vec::new();
    let mut image_infos: Vec<ImageInfo> = Vec::new();
    let fallback_mat = 0usize;

    // Determine sorted brush IDs from the map.
    let mut sorted: Vec<(u32, usize)> = brush_to_mat.iter()
        .filter(|&(&k, _)| k != u32::MAX)
        .map(|(&k, &v)| (k, v))
        .collect();
    sorted.sort_by_key(|&(k, _)| k);

    for &(brush_id, mat_idx) in &sorted {
        if (brush_id as usize) >= brushes.len() {
            continue;
        }
        let brush = &brushes[brush_id as usize];

        // Ensure the materials vec has room.
        while materials.len() <= mat_idx {
            materials.push(Value::Null);
        }

        let tex_ref = brush.texture_id.first().and_then(|&tid| {
            let tid = tid as usize;
            (tid < b3d_textures.len()).then(|| &b3d_textures[tid])
        });

        let color = brush.color;

        let mat_val = if let Some(tex) = tex_ref {
            let raw = tex.file.trim_start_matches(".\\").trim_start_matches("./");
            let tex_name = texture_stem(raw);
            let png_bytes = load_texture(tex_name, game_dir, tex_cache);

            if let Some(bytes) = png_bytes {
                let tex_idx = image_infos.len();
                image_infos.push(ImageInfo { mime: "image/png".into(), data: bytes });

                json!({
                    "pbrMetallicRoughness": {
                        "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                        "baseColorTexture": { "index": tex_idx },
                        "metallicFactor": 0.0,
                        "roughnessFactor": 0.9,
                    },
                    "doubleSided": true,
                })
            } else {
                json!({
                    "pbrMetallicRoughness": {
                        "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                        "metallicFactor": 0.0,
                        "roughnessFactor": 0.9,
                    },
                    "doubleSided": true,
                })
            }
        } else {
            json!({
                "pbrMetallicRoughness": {
                    "baseColorFactor": [color[0], color[1], color[2], color[3]],
                    "metallicFactor": 0.0,
                    "roughnessFactor": 0.9,
                },
                "doubleSided": true,
            })
        };

        materials[mat_idx] = mat_val;
    }

    // Fallback: try a texture named after the model.
    if image_infos.is_empty() && !materials.is_empty() {
        if let Some(bytes) = load_texture(model_name, game_dir, tex_cache) {
            let tex_idx = image_infos.len() as u32;
            image_infos.push(ImageInfo { mime: "image/png".into(), data: bytes });

            for mat in &mut materials {
                if let Some(obj) = mat.as_object_mut() {
                    if let Some(pbr) = obj.get_mut("pbrMetallicRoughness").and_then(|v| v.as_object_mut()) {
                        pbr.insert("baseColorFactor".into(), json!([1.0, 1.0, 1.0, 1.0]));
                        pbr.insert("baseColorTexture".into(), json!({"index": tex_idx}));
                    }
                }
            }
        }
    }

    // Ensure at least one material exists.
    if materials.is_empty() || materials.iter().all(|v| v.is_null()) {
        materials.clear();
        materials.push(json!({
            "pbrMetallicRoughness": {
                "baseColorFactor": [0.8, 0.8, 0.8, 1.0],
                "metallicFactor": 0.0,
                "roughnessFactor": 0.9,
            },
            "doubleSided": true,
        }));
    }

    Ok((materials, image_infos, fallback_mat))
}

// ---------------------------------------------------------------------------
// Nodes & scene
// ---------------------------------------------------------------------------

fn build_node_hierarchy(joints: &[JointInfo], has_skin: bool) -> (Vec<Value>, Vec<u32>) {
    if joints.is_empty() {
        return (vec![json!({"mesh": 0, "name": "root"})], vec![0]);
    }

    let mut gltf_nodes: Vec<Value> = Vec::new();
    for (i, j) in joints.iter().enumerate() {
        let pos = neg_z_pos(j.position);
        let rot = neg_z_quat(j.rotation);
        let scl = j.scale;

        let mut node = json!({
            "name": j.name,
            "translation": [pos[0], pos[1], pos[2]],
            "rotation": [rot[1], rot[2], rot[3], rot[0]],
            "scale": [scl[0], scl[1], scl[2]],
        });

        if i == 0 {
            node["mesh"] = json!(0);
            if has_skin {
                node["skin"] = json!(0);
            }
        }

        let children: Vec<u32> = (0..joints.len())
            .filter(|&c| joints[c].parent == Some(i))
            .map(|c| c as u32)
            .collect();
        if !children.is_empty() {
            node["children"] = json!(children);
        }

        gltf_nodes.push(node);
    }

    let scene_nodes: Vec<u32> = (0..joints.len())
        .filter(|&i| joints[i].parent.is_none())
        .map(|i| i as u32)
        .collect();

    (gltf_nodes, scene_nodes)
}

// ---------------------------------------------------------------------------
// Animations
// ---------------------------------------------------------------------------

fn build_animations(
    clips: &[AnimClip],
    joints: &[JointInfo],
    acc_start: u32,
    bvs: &mut Vec<Value>,
    accs: &mut Vec<Value>,
    bin: &mut Vec<u8>,
) -> Vec<Value> {
    let mut gltf_anims: Vec<Value> = Vec::new();
    let mut acc_counter = acc_start;

    for clip in clips {
        let fps = if clip.fps > 0.0 { clip.fps } else { 30.0 };
        let mut channels: Vec<Value> = Vec::new();
        let mut samplers: Vec<Value> = Vec::new();

        for (ji, joint) in joints.iter().enumerate() {
            if joint.keys.is_empty() { continue; }

            let filtered: Vec<&(u32, [f32; 3], [f32; 3], [f32; 4])> = joint.keys.iter()
                .filter(|(frame, _, _, _)| *frame >= clip.first_frame && *frame <= clip.last_frame)
                .collect();
            if filtered.is_empty() { continue; }

            let kc = filtered.len();
            let flags = joint.key_flags;

            // Time accessor.
            let times_off = bin.len();
            for (frame, _, _, _) in &filtered {
                let t = (*frame - clip.first_frame) as f32 / fps;
                bin.extend_from_slice(&t.to_le_bytes());
            }
            let times_bv = bvs.len() as u32;
            bvs.push(make_bv(0, times_off, (kc * 4) as u32, 0, 34962));

            let times_acc = acc_counter;
            accs.push(json!({"bufferView": times_bv, "componentType": 5126, "count": kc as u32, "type": "SCALAR"}));
            acc_counter += 1;

            // Position channel.
            if flags & 1 != 0 {
                let off = bin.len();
                for (_, p, _, _) in &filtered {
                    let cp = neg_z_pos(*p);
                    bin.extend_from_slice(&cp[0].to_le_bytes());
                    bin.extend_from_slice(&cp[1].to_le_bytes());
                    bin.extend_from_slice(&cp[2].to_le_bytes());
                }
                let bv_idx = bvs.len() as u32;
                bvs.push(make_bv(0, off, (kc * 12) as u32, 12, 34962));
                let val_acc = acc_counter;
                accs.push(json!({"bufferView": bv_idx, "componentType": 5126, "count": kc as u32, "type": "VEC3"}));
                acc_counter += 1;

                let si = samplers.len() as u32;
                samplers.push(json!({"input": times_acc, "output": val_acc, "interpolation": "LINEAR"}));
                channels.push(json!({"sampler": si, "target": {"node": ji as u32, "path": "translation"}}));
            }

            // Scale channel.
            if flags & 2 != 0 {
                let off = bin.len();
                for (_, _, s, _) in &filtered {
                    bin.extend_from_slice(&s[0].to_le_bytes());
                    bin.extend_from_slice(&s[1].to_le_bytes());
                    bin.extend_from_slice(&s[2].to_le_bytes());
                }
                let bv_idx = bvs.len() as u32;
                bvs.push(make_bv(0, off, (kc * 12) as u32, 12, 34962));
                let val_acc = acc_counter;
                accs.push(json!({"bufferView": bv_idx, "componentType": 5126, "count": kc as u32, "type": "VEC3"}));
                acc_counter += 1;

                let si = samplers.len() as u32;
                samplers.push(json!({"input": times_acc, "output": val_acc, "interpolation": "LINEAR"}));
                channels.push(json!({"sampler": si, "target": {"node": ji as u32, "path": "scale"}}));
            }

            // Rotation channel.
            if flags & 4 != 0 {
                let off = bin.len();
                for (_, _, _, r) in &filtered {
                    let q = neg_z_quat(*r);
                    bin.extend_from_slice(&q[1].to_le_bytes());
                    bin.extend_from_slice(&q[2].to_le_bytes());
                    bin.extend_from_slice(&q[3].to_le_bytes());
                    bin.extend_from_slice(&q[0].to_le_bytes());
                }
                let bv_idx = bvs.len() as u32;
                bvs.push(make_bv(0, off, (kc * 16) as u32, 16, 34962));
                let val_acc = acc_counter;
                accs.push(json!({"bufferView": bv_idx, "componentType": 5126, "count": kc as u32, "type": "VEC4"}));
                acc_counter += 1;

                let si = samplers.len() as u32;
                samplers.push(json!({"input": times_acc, "output": val_acc, "interpolation": "LINEAR"}));
                channels.push(json!({"sampler": si, "target": {"node": ji as u32, "path": "rotation"}}));
            }
        }

        pad_to_4_in_place(bin);

        if !channels.is_empty() {
            gltf_anims.push(json!({
                "name": clip.name,
                "channels": channels,
                "samplers": samplers,
            }));
        }
    }

    gltf_anims
}

// ---------------------------------------------------------------------------
// Image / texture JSON (embedded or external)
// ---------------------------------------------------------------------------

fn build_image_json(
    image_infos: &[ImageInfo],
    bvs: &mut Vec<Value>,
    bin: &mut Vec<u8>,
) -> (Vec<Value>, Vec<Value>) {
    pad_to_4_in_place(bin);

    let mut images = Vec::new();
    let mut textures = Vec::new();

    for info in image_infos {
        let img_off = bin.len();
        bin.extend_from_slice(&info.data);
        pad_to_4_in_place(bin);

        let bv_idx = bvs.len() as u32;
        bvs.push(json!({
            "buffer": 0,
            "byteOffset": img_off,
            "byteLength": info.data.len() as u32,
        }));

        images.push(json!({
            "mimeType": info.mime,
            "bufferView": bv_idx,
        }));
        textures.push(json!({"source": textures.len() as u32}));
    }

    (images, textures)
}

fn build_image_uris(
    image_infos: &[ImageInfo],
    tex_out_dir: &Path,
    model_name: &str,
) -> (Vec<Value>, Vec<Value>) {
    let mut images = Vec::new();
    let mut textures = Vec::new();

    for (i, info) in image_infos.iter().enumerate() {
        let fname = format!("{model_name}_tex{i}.png");
        let fpath = tex_out_dir.join(&fname);

        // Write the PNG file.
        let _ = std::fs::write(&fpath, &info.data);

        images.push(json!({
            "mimeType": info.mime,
            "uri": format!("textures/{fname}"),
        }));
        textures.push(json!({"source": textures.len() as u32}));
    }

    (images, textures)
}

// ---------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------

fn calc_bounds(positions: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX, f32::MAX, f32::MAX];
    let mut max = [f32::MIN, f32::MIN, f32::MIN];
    for p in positions {
        let pz = -p[2];
        if p[0] < min[0] { min[0] = p[0]; }
        if p[1] < min[1] { min[1] = p[1]; }
        if pz < min[2] { min[2] = pz; }
        if p[0] > max[0] { max[0] = p[0]; }
        if p[1] > max[1] { max[1] = p[1]; }
        if pz > max[2] { max[2] = pz; }
    }
    (min, max)
}
