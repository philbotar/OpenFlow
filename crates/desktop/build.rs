use std::path::{Path, PathBuf};
use std::{env, fs};

const SEARCH_CLI_VERSION: &str = "0.8.0";
const SEARCH_CLI_SHA256SUMS: &[(&str, &str)] = &[
    (
        "search-aarch64-apple-darwin.tar.gz",
        "d395541471078b433db5a1f4ca2ecf32457c6d3632e6a1d4ee314446af4d9907",
    ),
    (
        "search-x86_64-apple-darwin.tar.gz",
        "3d584bdd6b8e40e4ba5779a2166397200f8f8568ffc36d830b06de7db96f3469",
    ),
    (
        "search-x86_64-unknown-linux-gnu.tar.gz",
        "4ec3a24532d06189b0a5d0a4a44301cc82287008841bf1f4af1c9c82c19a2619",
    ),
];

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

    let ext = if target.contains("windows") {
        ".exe"
    } else {
        ""
    };
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
    verify_archive_sha256(&archive, asset);
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

fn expected_sha256(asset: &str) -> &'static str {
    SEARCH_CLI_SHA256SUMS
        .iter()
        .find_map(|(name, sha)| (*name == asset).then_some(*sha))
        .unwrap_or_else(|| panic!("missing pinned SHA-256 for search-cli asset {asset}"))
}

fn verify_archive_sha256(archive: &Path, asset: &str) {
    let actual = command_stdout(
        "shasum",
        &["-a", "256", path_str(archive)],
        "verify search-cli release checksum",
    )
    .or_else(|| {
        command_stdout(
            "sha256sum",
            &[path_str(archive)],
            "verify search-cli release checksum",
        )
    })
    .unwrap_or_else(|| {
        panic!(
            "verify search-cli release checksum: install shasum/sha256sum or prepopulate {}",
            archive.display()
        )
    });
    let actual = actual
        .split_whitespace()
        .next()
        .unwrap_or_else(|| panic!("empty SHA-256 output for {}", archive.display()));
    assert_eq!(
        actual,
        expected_sha256(asset),
        "search-cli release checksum mismatch for {asset}"
    );
}

fn command_stdout(program: &str, args: &[&str], action: &str) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8(output.stdout).unwrap_or_else(|error| {
            panic!("{action} via {program} emitted non-utf8 output: {error}")
        }),
    )
}

fn run_command(program: &str, args: &[&str], action: &str) {
    let status = std::process::Command::new(program)
        .args(args)
        .status()
        .unwrap_or_else(|error| panic!("spawn {program} to {action}: {error}"));
    assert!(status.success(), "{action} failed via {program}");
}
