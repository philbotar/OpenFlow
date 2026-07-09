use std::path::{Path, PathBuf};
use std::{env, fs};

const SEARCH_CLI_VERSION: &str = "0.8.0";

fn main() {
    println!("cargo:rerun-if-changed=icons/icon.icns");
    println!("cargo:rerun-if-changed=icons/icon.png");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=TARGET");
    copy_search_sidecar();
    tauri_build::build();
}

fn asset_for_target(target: &str) -> Option<&'static str> {
    match target {
        "aarch64-apple-darwin" => Some("search-aarch64-apple-darwin.tar.gz"),
        "x86_64-apple-darwin" => Some("search-x86_64-apple-darwin.tar.gz"),
        "x86_64-unknown-linux-gnu" => Some("search-x86_64-unknown-linux-gnu.tar.gz"),
        _ => None,
    }
}

fn copy_search_sidecar() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let target = env::var("TARGET").expect("TARGET");
    let dest_dir = manifest_dir.join("binaries");
    fs::create_dir_all(&dest_dir).expect("create binaries dir");

    let ext = if target.contains("windows") { ".exe" } else { "" };
    let dest = dest_dir.join(format!("search-{target}{ext}"));
    if dest.is_file() {
        return;
    }

    let Some(asset) = asset_for_target(&target) else {
        println!(
            "cargo:warning=search-cli sidecar skipped for target {target} (no prebuilt release asset)"
        );
        return;
    };

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let archive = out_dir.join(asset);
    let extract_dir = out_dir.join("search-extract");
    let _ = fs::remove_dir_all(&extract_dir);
    fs::create_dir_all(&extract_dir).expect("create extract dir");

    let url = format!(
        "https://github.com/paperfoot/search-cli/releases/download/v{SEARCH_CLI_VERSION}/{asset}"
    );
    run_command(
        "curl",
        &["-fsSL", "-o", path_str(&archive), &url],
        "download search-cli release",
    );
    run_command(
        "tar",
        &["-xzf", path_str(&archive), "-C", path_str(&extract_dir)],
        "extract search-cli release",
    );

    let source = extract_dir.join(format!("search{ext}"));
    fs::copy(&source, &dest).unwrap_or_else(|error| {
        panic!(
            "copy search sidecar from {} to {}: {error}",
            source.display(),
            dest.display()
        );
    });
}

fn path_str(path: &Path) -> &str {
    path.to_str()
        .unwrap_or_else(|| panic!("non-utf8 path: {}", path.display()))
}

fn run_command(program: &str, args: &[&str], action: &str) {
    let status = std::process::Command::new(program)
        .args(args)
        .status()
        .unwrap_or_else(|error| panic!("spawn {program} to {action}: {error}"));
    assert!(status.success(), "{action} failed via {program}");
}
