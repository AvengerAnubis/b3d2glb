pub use crate::b3d_parser::B3D;
use crate::b3d_parser::Node;
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
///
/// Set `is_root = true` for the initial call (the mesh node). The root node is
/// NOT added to the joint list – it is the skinned mesh, not a deforming bone.
pub fn collect_joints(
    node: &Node,
    parent: Option<usize>,
    joints: &mut Vec<JointInfo>,
    vertex_joint: &mut Vec<Option<(usize, f32)>>,
    vcount: usize,
    is_root: bool,
) {
    let idx = if is_root { 0 } else { joints.len() };
    if !is_root {
        let keys: Vec<_> = node.keys.iter().map(|k| {
            (k.frame, k.position, k.scale, k.rotation)
        }).collect();

        joints.push(JointInfo {
            name: node.name.clone(),
            position: node.position,
            scale: node.scale,
            rotation: node.rotation,
            parent,
            keys,
        });
    }

    for b in &node.bones {
        let vi = b.vertex_id as usize;
        if vi < vcount {
            // Accumulate weights (a vertex may be assigned to multiple bones).
            // For now we keep only the first (or last) assignment; a proper fix
            // would store all weights and let the writer pick up to 4.
            vertex_joint[vi] = Some((idx, b.weight));
        }
    }

    for child in &node.children {
        // Root's children get parent=None since the mesh isn't a joint.
        let child_parent = if is_root { None } else { Some(idx) };
        collect_joints(child, child_parent, joints, vertex_joint, vcount, false);
    }
}

/// Collect named animation clips from the B3D node tree.
pub fn collect_anims(node: &Node) -> Vec<AnimClip> {
    let mut anims = Vec::new();
    let fps = if node.animation.fps > 0.0 { node.animation.fps } else { 30.0 };

    if !node.sequences.is_empty() {
        for seq in &node.sequences {
            anims.push(AnimClip {
                name: seq.name.clone(),
                fps,
                first_frame: seq.first_frame,
                last_frame: seq.last_frame,
            });
        }
    } else if node.animation.frames > 1 {
        anims.push(AnimClip {
            name: "default".into(),
            fps,
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
        // Mesh vertices are in root-node local space, same as root joints.
        // Convert from B3D (left-handed Y-up) to glTF (right-handed Y-up)
        // by negating Z: [x, y, -z]. This matches root_pos for root bones.
        positions.push([v.position[0], v.position[1], -v.position[2]]);
        normals.push([v.normal[0], v.normal[1], -v.normal[2]]);
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

/// Compute the world-space matrix for a single joint (recursive).
///
/// Prefer `compute_world_matrices` for batch computation — it computes all
/// world matrices in a single O(n) pass instead of O(n²) recursion.
pub fn compute_world_matrix(joints: &[JointInfo], idx: usize) -> Mat4 {
    let scale = joints[idx].scale;
    let (pos, rot) = if joints[idx].parent.is_none() {
        (math::root_pos(joints[idx].position), math::root_quat(joints[idx].rotation))
    } else {
        (math::swap_yz_pos(joints[idx].position), math::swap_yz_quat(joints[idx].rotation))
    };
    let local = math::b3d_to_mat4(pos, scale, rot);
    match joints[idx].parent {
        Some(p) => math::mat4_mul(&compute_world_matrix(joints, p), &local),
        None => local,
    }
}

/// Compute all world-space matrices in a single O(n) pass.
///
/// Joints must be ordered parent-before-child (guaranteed by `collect_joints`'s
/// DFS traversal — a child always appears after its parent).
///
/// The IBMs stored in the glTF skin satisfy: at bind time,
/// `world_matrix(joint) × ibm(joint) = I`.
pub fn compute_world_matrices(joints: &[JointInfo]) -> Vec<Mat4> {
    let mut world = Vec::with_capacity(joints.len());
    for (i, joint) in joints.iter().enumerate() {
        let scale = joint.scale;
        let (pos, rot) = if joint.parent.is_none() {
            (math::root_pos(joint.position), math::root_quat(joint.rotation))
        } else {
            (math::swap_yz_pos(joint.position), math::swap_yz_quat(joint.rotation))
        };
        let local = math::b3d_to_mat4(pos, scale, rot);
        world.push(match joint.parent {
            Some(p) => math::mat4_mul(&world[p], &local),
            None => local,
        });
    }
    world
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math;

    const EPS: f32 = 2e-5;

    fn make_joint(name: &str, pos: [f32; 3], rot: [f32; 4], parent: Option<usize>) -> JointInfo {
        JointInfo {
            name: name.to_string(),
            position: pos,
            scale: [1.0, 1.0, 1.0],
            rotation: rot,
            parent,
            keys: vec![],
        }
    }

    #[test]
    fn test_compute_world_matrix_root() {
        let joints = vec![make_joint("root", [0.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0], None)];
        let w = compute_world_matrix(&joints, 0);
        let expected = math::b3d_to_mat4(
            math::root_pos([0.0, 0.0, 0.0]),
            [1.0, 1.0, 1.0],
            math::root_quat([1.0, 0.0, 0.0, 0.0]),
        );
        for i in 0..4 {
            for j in 0..4 {
                assert!((w[i][j] - expected[i][j]).abs() < EPS,
                    "mismatch at [{i}][{j}]: {} vs {}", w[i][j], expected[i][j]);
            }
        }
    }

    #[test]
    fn test_compute_world_matrix_single_child() {
        let joints = vec![
            make_joint("root", [10.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0], None),
            make_joint("child", [5.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0], Some(0)),
        ];
        let w_child = compute_world_matrix(&joints, 1);
        let root_w = compute_world_matrix(&joints, 0);
        let child_local = math::b3d_to_mat4(
            math::swap_yz_pos([5.0, 0.0, 0.0]),
            [1.0, 1.0, 1.0],
            math::swap_yz_quat([1.0, 0.0, 0.0, 0.0]),
        );
        let expected = math::mat4_mul(&root_w, &child_local);
        for i in 0..4 {
            for j in 0..4 {
                assert!((w_child[i][j] - expected[i][j]).abs() < EPS,
                    "mismatch at [{i}][{j}]: {} vs {}", w_child[i][j], expected[i][j]);
            }
        }
    }

    #[test]
    fn test_compute_world_matrix_chain() {
        let joints = vec![
            make_joint("root", [1.0, 2.0, 3.0], [1.0, 0.0, 0.0, 0.0], None),
            make_joint("mid", [4.0, 5.0, 6.0], [1.0, 0.0, 0.0, 0.0], Some(0)),
            make_joint("tip", [7.0, 8.0, 9.0], [1.0, 0.0, 0.0, 0.0], Some(1)),
        ];
        let w_tip = compute_world_matrix(&joints, 2);
        let root_w = compute_world_matrix(&joints, 0);
        let mid_local = math::b3d_to_mat4(
            math::swap_yz_pos([4.0, 5.0, 6.0]),
            [1.0, 1.0, 1.0],
            math::swap_yz_quat([1.0, 0.0, 0.0, 0.0]),
        );
        let mid_w = math::mat4_mul(&root_w, &mid_local);
        let tip_local = math::b3d_to_mat4(
            math::swap_yz_pos([7.0, 8.0, 9.0]),
            [1.0, 1.0, 1.0],
            math::swap_yz_quat([1.0, 0.0, 0.0, 0.0]),
        );
        let expected = math::mat4_mul(&mid_w, &tip_local);
        for i in 0..4 {
            for j in 0..4 {
                assert!((w_tip[i][j] - expected[i][j]).abs() < EPS,
                    "mismatch at [{i}][{j}]: {} vs {}", w_tip[i][j], expected[i][j]);
            }
        }
    }

    #[test]
    fn test_world_times_ibm_is_identity() {
        // Use real monkey.b3d joint data (root + child + grandchild)
        let joints = vec![
            make_joint("root", [0.0333, 17.5667, -6.0212], [1.0, 0.0, 0.0, 0.0], None),
            make_joint("child", [-9.5327, 5.2530, 4.6003], [0.3942, 0.7666, 0.2318, 0.4508], Some(0)),
            make_joint("grandchild", [0.0, 0.0, -13.4415], [-0.0302, 0.0841, -0.3368, 0.9373], Some(1)),
        ];
        for i in 0..joints.len() {
            let w = compute_world_matrix(&joints, i);
            let ibm = math::mat4_inverse(&w);
            let product = math::mat4_mul(&w, &ibm);
            for r in 0..4 {
                for c in 0..4 {
                    let expected = if r == c { 1.0 } else { 0.0 };
                    assert!((product[r][c] - expected).abs() < EPS,
                        "joint[{i}] product[{r}][{c}]: {} vs {} (expected I)",
                        product[r][c], expected);
                }
            }
        }
    }
}
