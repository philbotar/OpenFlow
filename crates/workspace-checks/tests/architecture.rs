use std::path::PathBuf;
use std::process::Command;

#[test]
fn architecture_boundaries() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("resolve workspace root");
    let script = root.join("scripts/check-architecture.sh");
    let status = Command::new(&script)
        .current_dir(&root)
        .status()
        .expect("run check-architecture.sh");
    assert!(status.success(), "check-architecture.sh failed");
}
