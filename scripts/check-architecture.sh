#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(dirname "$SCRIPT_DIR")"

cd "$WORKSPACE_DIR"

exec python3 - "$WORKSPACE_DIR" <<'PY'
import re
import sys
import tomllib
from pathlib import Path

WORKSPACE_DIR = Path(sys.argv[1])
ERRORS = 0

ALLOWED = {
    "domain": set(),
    "providers": {"domain"},
    "orchestration": {"domain", "providers"},
    "desktop": {"domain", "orchestration"},
}

CRATE_MANIFESTS = {
    "domain": WORKSPACE_DIR / "crates" / "domain" / "Cargo.toml",
    "providers": WORKSPACE_DIR / "crates" / "providers" / "Cargo.toml",
    "orchestration": WORKSPACE_DIR / "crates" / "orchestration" / "Cargo.toml",
    "desktop": WORKSPACE_DIR / "crates" / "desktop" / "src-tauri" / "Cargo.toml",
}

# Verify workspace dependency direction
for crate, manifest in CRATE_MANIFESTS.items():
    with manifest.open("rb") as f:
        data = tomllib.load(f)
    deps = data.get("dependencies", {})
    allowed = ALLOWED[crate]
    for name, spec in deps.items():
        if isinstance(spec, dict) and "path" in spec:
            target = spec.get("package", name)
            if target in CRATE_MANIFESTS and target not in allowed:
                print(
                    f"error: {crate} depends on workspace member '{target}', "
                    f"but only allowed: {sorted(allowed)}"
                )
                ERRORS += 1

# Verify domain has no forbidden external deps
FORBIDDEN = {"reqwest", "tauri"}
domain_manifest = CRATE_MANIFESTS["domain"]
with domain_manifest.open("rb") as f:
    data = tomllib.load(f)
deps = data.get("dependencies", {})
for dep in FORBIDDEN:
    if dep in deps:
        print(f"error: domain must not depend on '{dep}' (GUI/framework concern)")
        ERRORS += 1

# Enforce that migrated UI seam files are not placeholders.
UI_SEAM_FILES = [
    WORKSPACE_DIR / "crates" / "ui" / "src" / "ports" / "inbound.ts",
    WORKSPACE_DIR / "crates" / "ui" / "src" / "ports" / "outbound.ts",
    WORKSPACE_DIR / "crates" / "ui" / "src" / "adapters" / "inbound.ts",
    WORKSPACE_DIR / "crates" / "ui" / "src" / "adapters" / "outbound.ts",
]

RUST_SEAM_FILES = [
    WORKSPACE_DIR / "crates" / "domain" / "src" / "ports" / "inbound.rs",
    WORKSPACE_DIR / "crates" / "domain" / "src" / "ports" / "outbound.rs",
    WORKSPACE_DIR / "crates" / "domain" / "src" / "adapters" / "inbound.rs",
    WORKSPACE_DIR / "crates" / "domain" / "src" / "adapters" / "outbound.rs",
    WORKSPACE_DIR / "crates" / "providers" / "src" / "ports" / "inbound.rs",
    WORKSPACE_DIR / "crates" / "providers" / "src" / "ports" / "outbound.rs",
    WORKSPACE_DIR / "crates" / "providers" / "src" / "adapters" / "inbound.rs",
    WORKSPACE_DIR / "crates" / "providers" / "src" / "adapters" / "outbound.rs",
    WORKSPACE_DIR / "crates" / "orchestration" / "src" / "ports" / "inbound.rs",
    WORKSPACE_DIR / "crates" / "orchestration" / "src" / "ports" / "outbound.rs",
    WORKSPACE_DIR / "crates" / "orchestration" / "src" / "adapters" / "inbound.rs",
    WORKSPACE_DIR / "crates" / "orchestration" / "src" / "adapters" / "outbound.rs",
]

for seam_file in UI_SEAM_FILES:
    text = seam_file.read_text(encoding="utf-8")
    if "Placeholder" in text or re.search(r"=\s*never\s*;", text):
        print(f"error: UI seam file still contains placeholder content: {seam_file}")
        ERRORS += 1
        continue

    meaningful_lines = [
        line.strip()
        for line in text.splitlines()
        if line.strip() and not line.strip().startswith("//")
    ]
    if not meaningful_lines:
        print(f"error: UI seam file is effectively empty: {seam_file}")
        ERRORS += 1
        continue

    has_exported_api = any(
        re.search(r"^export\s+(interface|type|function|const)\s+", line)
        for line in meaningful_lines
    )
    if not has_exported_api:
        print(f"error: UI seam file lacks exported API surface: {seam_file}")
        ERRORS += 1

for seam_file in RUST_SEAM_FILES:
    text = seam_file.read_text(encoding="utf-8")
    if "placeholder" in text.lower():
        print(f"error: Rust seam file still contains placeholder content: {seam_file}")
        ERRORS += 1
        continue

    meaningful_lines = [
        line.strip()
        for line in text.splitlines()
        if line.strip() and not line.strip().startswith("//")
    ]
    if not meaningful_lines:
        print(f"error: Rust seam file is effectively empty: {seam_file}")
        ERRORS += 1
        continue

    has_api_shape = any(
        re.search(r"^(pub\s+)?(async\s+)?(trait|struct|enum|type|fn)\s+", line)
        or re.search(r"^impl\s+", line)
        for line in meaningful_lines
    )
    if not has_api_shape:
        print(f"error: Rust seam file lacks API surface: {seam_file}")
        ERRORS += 1

if ERRORS:
    print(f"Architecture check failed with {ERRORS} error(s).")
    sys.exit(1)

print("Architecture check passed.")
PY