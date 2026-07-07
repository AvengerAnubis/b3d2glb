use std::fs;
use std::env;
use b3d::B3D;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 { eprintln!("usage: dump <file.b3d>"); return; }
    let data = fs::read(&args[1]).unwrap();
    let b3d = B3D::read(&data).unwrap();
    let vcount = b3d.node.mesh.vertices.vertices.len();
    dump_node(&b3d.node, 0, vcount);
}

fn dump_node(node: &b3d::Node, depth: usize, vcount: usize) {
    let indent = "  ".repeat(depth);
    let has_mesh = !node.mesh.vertices.vertices.is_empty();
    if has_mesh {
        println!("{}+ MESH \"{}\" bones={} verts={}", indent, node.name, node.bones.len(), node.mesh.vertices.vertices.len());
    } else {
        println!("{}+ NODE \"{}\" p=({:.4},{:.4},{:.4}) bones={} keys={}", indent, node.name,
            node.position[0], node.position[1], node.position[2], node.bones.len(), node.keys.len());
    }
    for (i, b) in node.bones.iter().enumerate().take(8) {
        println!("{}  bone[{}]: v={} w={:.2}", indent, i, b.vertex_id, b.weight);
    }
    if node.bones.len() > 8 { println!("{}  ... ({} more)", indent, node.bones.len() - 8); }
    for child in &node.children { dump_node(child, depth + 1, vcount); }
}
