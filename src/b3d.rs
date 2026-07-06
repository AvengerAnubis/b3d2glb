pub use b3d::B3D;
use b3d::Node;
use crate::math::Mat4;
use crate::math;

/// Per-vertex skinning data (B3D stores at most one bone per vertex).
#[derive(Debug, Clone)]
pub struct BoneWeight {
    pub joint_idx: u32,
    pub weight: f32,
}

/// A group of triangle indices sharing a material (brush).
#[derive(Debug, Clone)]
pub struct TriGroup {
    pub brush_id: u32,
    pub indices: Vec<u32>,
}

/// Extracted mesh data from a B3D node.
#[derive(Debug, Clone)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub tri_groups: Vec<TriGroup>,
    pub skin: Vec<Option<BoneWeight>>,
}

/// One joint (bone) in the skeleton hierarchy.
#[derive(Debug, Clone)]
pub struct JointInfo {
    pub name: String,
    /// Local bind-pose translation (B3D coordinates).
    pub position: [f32; 3],
    /// Local bind-pose scale.
    pub scale: [f32; 3],
    /// Local bind-pose rotation quaternion (B3D: [w, x, y, z]).
    pub rotation: [f32; 4],
    /// Index of parent joint in the flattened array, or `None` for root.
    pub parent: Option<usize>,
    /// Key flags bitmask: 1=position, 2=scale, 4=rotation.
    pub key_flags: u32,
    /// Keyframes: `(frame, position, scale, rotation)`.
    pub keys: Vec<(u32, [f32; 3], [f32; 3], [f32; 4])>,
}

/// A named animation clip derived from B3D sequences.
#[derive(Debug, Clone)]
pub struct AnimClip {
    pub name: String,
    pub fps: f32,
    pub first_frame: u32,
    pub last_frame: u32,
}

/// Traverse the B3D node tree, collecting joints and vertex-to-joint mapping.
pub fn collect_joints(
    node: &Node,
    parent: Option<usize>,
    joints: &mut Vec<JointInfo>,
    vertex_joint: &mut Vec<Option<usize>>,
    vcount: usize,
) {
    let idx = joints.len();
    let keys: Vec<_> = node.keys.iter().map(|k| {
        (k.frame, k.position, k.scale, k.rotation)
    }).collect();

    joints.push(JointInfo {
        name: node.name.clone(),
        position: node.position,
        scale: node.scale,
        rotation: node.rotation,
        parent,
        key_flags: node.key_flags,
        keys,
    });

    for b in &node.bones {
        let vi = b.vertex_id as usize;
        if vi < vcount {
            vertex_joint[vi] = Some(idx);
        }
    }

    for child in &node.children {
        collect_joints(child, Some(idx), joints, vertex_joint, vcount);
    }
}

/// Collect named animation clips from the B3D node tree.
pub fn collect_anims(node: &Node) -> Vec<AnimClip> {
    let mut anims = Vec::new();

    if !node.sequences.is_empty() {
        for seq in &node.sequences {
            anims.push(AnimClip {
                name: seq.name.clone(),
                fps: node.animation.fps,
                first_frame: seq.first_frame,
                last_frame: seq.last_frame,
            });
        }
    } else if node.animation.frames > 1 {
        anims.push(AnimClip {
            name: "default".into(),
            fps: node.animation.fps,
            first_frame: 0,
            last_frame: node.animation.frames.saturating_sub(1),
        });
    }

    anims
}

/// Extract mesh geometry from a parsed B3D file.
pub fn collect_mesh(b3d: &B3D) -> MeshData {
    let verts = &b3d.node.mesh.vertices;
    let vc = verts.vertices.len();

    let mut positions = Vec::with_capacity(vc);
    let mut normals = Vec::with_capacity(vc);
    let mut uvs = Vec::with_capacity(vc);

    for v in &verts.vertices {
        positions.push(v.position);
        normals.push(v.normal);
        uvs.push([v.tex_coords[0], v.tex_coords[1]]);
    }

    let mut tri_groups = Vec::new();
    for tris in &b3d.node.mesh.triangles {
        let mut indices = Vec::with_capacity(tris.indices.len() * 3);
        for tri in &tris.indices {
            indices.push(tri[0]);
            indices.push(tri[2]);
            indices.push(tri[1]);
        }
        tri_groups.push(TriGroup { brush_id: tris.brush_id, indices });
    }

    let skin = (0..vc).map(|_| None).collect();
    MeshData { positions, normals, uvs, tri_groups, skin }
}

/// Compute the world-space matrix for a joint (right-handed Y-up).
pub fn compute_world_matrix(joints: &[JointInfo], idx: usize) -> Mat4 {
    let pos = math::neg_z_pos(joints[idx].position);
    let scale = joints[idx].scale;
    let rot = math::neg_z_quat(joints[idx].rotation);
    let local = math::b3d_to_mat4(pos, scale, rot);
    match joints[idx].parent {
        Some(p) => math::mat4_mul(&compute_world_matrix(joints, p), &local),
        None => local,
    }
}
