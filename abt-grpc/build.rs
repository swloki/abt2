use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = "E:/work/abt/proto";
    let out_dir = "src/generated";

    if !std::path::Path::new(out_dir).exists() {
        std::fs::create_dir_all(out_dir)?;
    }

    let search_dir = format!("{}/abt/v1", proto_root);
    let mut proto_files = Vec::new();

    for entry in std::fs::read_dir(&search_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "proto") {
            let rel_path = path.strip_prefix(proto_root)?;
            proto_files.push(PathBuf::from(proto_root).join(rel_path));
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    if !proto_files.is_empty() {
        tonic_prost_build::configure()
            .build_server(true)
            .out_dir(out_dir)
            .compile_protos(&proto_files, &[proto_root.into()])?;
    }

    Ok(())
}
