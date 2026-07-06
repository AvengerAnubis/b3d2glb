use std::path::PathBuf;

/// Parsed command-line arguments.
#[derive(Debug)]
pub struct Args {
    /// Input paths (files or directories).
    pub inputs: Vec<PathBuf>,
    /// Output directory (default: current dir).
    pub out_dir: PathBuf,
    /// Context / game directory for texture lookups.
    pub context_dir: Option<PathBuf>,
    /// Whether to write a single .glb (otherwise .gltf + .bin + textures).
    pub glb: bool,
}

const USAGE: &str = "\
b3d2glb — convert Blitz3D .b3d models to glTF 2.0

USAGE:
  b3d2glb [OPTIONS] input...

ARGS:
  input...   One or more .b3d files or directories containing .b3d files.

OPTIONS:
  -o, --out DIR      Output directory (default: current directory)
  -c, --context DIR  Context / game root directory (texture lookup root)
  -b, --glb          Write binary .glb instead of separate .gltf + .bin + textures
  -h, --help         Display this help and exit

EXAMPLES:
  b3d2glb -o ./out -c /path/to/game model.b3d
  b3d2glb --glb -o ./out /path/to/game/gfx
  b3d2glb -b model.b3d
";

/// Parse command-line arguments or print help and exit.
pub fn parse_args() -> Result<Args, String> {
    let raw: Vec<String> = std::env::args().collect();
    let mut args = Args {
        inputs: Vec::new(),
        out_dir: PathBuf::from("."),
        context_dir: None,
        glb: false,
    };

    let mut i = 1;
    while i < raw.len() {
        match raw[i].as_str() {
            "-h" | "--help" | "-?" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            "-o" | "--out" => {
                i += 1;
                if i >= raw.len() {
                    return Err("-o/--out requires a value".into());
                }
                args.out_dir = PathBuf::from(&raw[i]);
            }
            "-c" | "--context" => {
                i += 1;
                if i >= raw.len() {
                    return Err("-c/--context requires a value".into());
                }
                args.context_dir = Some(PathBuf::from(&raw[i]));
            }
            "-b" | "--glb" => {
                args.glb = true;
            }
            s if s.starts_with('-') => {
                return Err(format!("unknown option: {s}"));
            }
            _ => {
                args.inputs.push(PathBuf::from(&raw[i]));
            }
        }
        i += 1;
    }

    if args.inputs.is_empty() {
        return Err("no input files or directories specified".into());
    }

    Ok(args)
}
