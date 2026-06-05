#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(dirname "$SCRIPT_DIR")"

cd "$WORKSPACE_DIR"

exec python3 - "$WORKSPACE_DIR" <<'PY'
import sys
import tomllib
from pathlib import Path

WORKSPACE_DIR = Path(sys.argv[1])
ERRORS = 0

ALLOWED = {
    "workflow-core": [],
    "openai-client": ["workflow-core"],
    "agent-workflow-app": ["workflow-core", "openai-client"],
    "agent-workflow-desktop": ["workflow-core", "agent-workflow-app"],
}

CRATE_MANIFESTS = {
    "workflow-core": WORKSPACE_DIR / "crates" / "workflow-core" / "Cargo.toml",
    "openai-client": WORKSPACE_DIR / "crates" / "openai-client" / "Cargo.toml",
    "agent-workflow-app": WORKSPACE_DIR / "crates" / "agent-workflow-app" / "Cargo.toml",
    "agent-workflow-desktop": WORKSPACE_DIR / "crates" / "agent-workflow-desktop" / "src-tauri" / "Cargo.toml",
}

# Verify workspace dependency direction
for crate, manifest in CRATE_MANIFESTS.items():
    with manifest.open("rb") as f:
        data = tomllib.load(f)
    deps = data.get("dependencies", {})
    allowed = set(ALLOWED[crate])
    for name, spec in deps.items():
        if isinstance(spec, dict) and "path" in spec:
            if name not in allowed:
                print(f"error: {crate} depends on workspace member '{name}', but only allowed: {sorted(allowed)}")
                ERRORS += 1

# Verify workflow-core has no forbidden external deps
FORBIDDEN = {"reqwest", "tauri", "eframe", "egui", "egui-phosphor"}
workflow_core_manifest = CRATE_MANIFESTS["workflow-core"]
with workflow_core_manifest.open("rb") as f:
    data = tomllib.load(f)
deps = data.get("dependencies", {})
for dep in FORBIDDEN:
    if dep in deps:
        print(f"error: workflow-core must not depend on '{dep}' (GUI/framework concern)")
        ERRORS += 1

# Advisory note about known architectural drift
app_manifest = CRATE_MANIFESTS["agent-workflow-app"]
with app_manifest.open("rb") as f:
    data = tomllib.load(f)
deps = data.get("dependencies", {})
for gui_dep in {"eframe", "egui"}:
    if gui_dep in deps:
        print(f"note: agent-workflow-app contains GUI dependency '{gui_dep}'. Per AGENTS.md, visual UI should be in agent-workflow-desktop. This is deferred — see architecture plan.")

if ERRORS:
    print(f"Architecture check failed with {ERRORS} error(s).")
    sys.exit(1)

print("Architecture check passed.")
PY
