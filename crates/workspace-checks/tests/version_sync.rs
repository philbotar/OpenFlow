use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[test]
fn version_manifests_stay_in_lockstep() {
    let root = workspace_root();
    let rust_versions = rust_workspace_versions(&root);
    assert_versions_are_consistent("Rust workspace packages", &rust_versions);

    let rust_version = rust_versions
        .values()
        .next()
        .expect("workspace must contain at least one package")
        .clone();

    let mut manifests = BTreeMap::new();
    manifests.insert(
        "crates/desktop/package.json",
        json_string_version(&root.join("crates/desktop/package.json"), |json| {
            json.get("version").and_then(serde_json::Value::as_str)
        }),
    );
    manifests.insert(
        "crates/desktop/tauri.conf.json",
        json_string_version(&root.join("crates/desktop/tauri.conf.json"), |json| {
            json.get("version").and_then(serde_json::Value::as_str)
        }),
    );
    manifests.insert(
        "crates/desktop/e2e/package.json",
        json_string_version(&root.join("crates/desktop/e2e/package.json"), |json| {
            json.get("version").and_then(serde_json::Value::as_str)
        }),
    );
    manifests.insert(
        "crates/desktop/e2e/package-lock.json",
        json_string_version(&root.join("crates/desktop/e2e/package-lock.json"), |json| {
            json.get("version").and_then(serde_json::Value::as_str)
        }),
    );
    manifests.insert(
        "crates/ui/package.json",
        json_string_version(&root.join("crates/ui/package.json"), |json| {
            json.get("version").and_then(serde_json::Value::as_str)
        }),
    );
    manifests.insert(
        "crates/ui/package-lock.json",
        json_string_version(&root.join("crates/ui/package-lock.json"), |json| {
            json.get("version").and_then(serde_json::Value::as_str)
        }),
    );

    assert_versions_match("release manifests", &manifests, &rust_version);
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("resolve workspace root")
}

fn rust_workspace_versions(root: &Path) -> BTreeMap<String, String> {
    let mut versions = BTreeMap::new();
    for manifest in [
        "crates/engine/Cargo.toml",
        "crates/providers/Cargo.toml",
        "crates/orchestration/Cargo.toml",
        "crates/desktop/Cargo.toml",
        "crates/workspace-checks/Cargo.toml",
    ] {
        versions.insert(
            manifest.to_owned(),
            cargo_package_version(&root.join(manifest)),
        );
    }

    versions
}

fn cargo_package_version(path: &Path) -> String {
    let contents = fs::read_to_string(path).unwrap_or_else(|err| {
        panic!("read {}: {err}", path.display());
    });

    let mut in_package_section = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed == "[package]" {
            in_package_section = true;
            continue;
        }
        if in_package_section && trimmed.starts_with('[') {
            break;
        }
        if in_package_section && trimmed.starts_with("version") {
            let (_, value) = trimmed
                .split_once('=')
                .unwrap_or_else(|| panic!("malformed version in {}", path.display()));
            return value.trim().trim_matches('"').to_owned();
        }
    }

    panic!("missing package version in {}", path.display());
}

fn json_string_version<F>(path: &Path, getter: F) -> String
where
    F: FnOnce(&serde_json::Value) -> Option<&str>,
{
    let contents = fs::read_to_string(path).unwrap_or_else(|err| {
        panic!("read {}: {err}", path.display());
    });
    let json: serde_json::Value = serde_json::from_str(&contents).unwrap_or_else(|err| {
        panic!("parse {}: {err}", path.display());
    });
    getter(&json)
        .unwrap_or_else(|| panic!("missing version in {}", path.display()))
        .to_owned()
}

fn assert_versions_are_consistent(label: &str, versions: &BTreeMap<String, String>) {
    let mut iter = versions.iter();
    let (first_name, first_version) = iter.next().expect("at least one versioned entry");
    let mismatches: Vec<_> = iter
        .filter(|(_, version)| *version != first_version)
        .map(|(name, version)| format!("{name}={version}"))
        .collect();

    assert!(
        mismatches.is_empty(),
        "{label} are not in lockstep. baseline {first_name}={first_version}; mismatches: {}",
        mismatches.join(", ")
    );
}

fn assert_versions_match(label: &str, versions: &BTreeMap<&str, String>, expected: &str) {
    let mismatches: Vec<_> = versions
        .iter()
        .filter(|(_, version)| version.as_str() != expected)
        .map(|(name, version)| format!("{name}={version}"))
        .collect();

    assert!(
        mismatches.is_empty(),
        "{label} do not match Rust workspace version {expected}; mismatches: {}",
        mismatches.join(", ")
    );
}
