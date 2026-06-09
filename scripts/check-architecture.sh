#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(dirname "$SCRIPT_DIR")"

cd "$WORKSPACE_DIR"

exec python3 - "$WORKSPACE_DIR" <<'PY'
import fnmatch
import re
import sys
import tomllib
from pathlib import Path

WORKSPACE_DIR = Path(sys.argv[1])
RULES_PATH = WORKSPACE_DIR / "docs" / "architecture" / "arch-check-rules.toml"
ERRORS = 0


def error(message: str) -> None:
    global ERRORS
    print(f"error: {message}")
    ERRORS += 1


with RULES_PATH.open("rb") as f:
    rules = tomllib.load(f)

manifests: dict[str, str] = rules["manifests"]
workspace_deps: dict[str, list[str]] = rules["workspace_deps"]
engine_forbidden: list[str] = rules["engine_forbidden_deps"]["names"]
legacy_aliases: list[str] = rules["legacy_crate_aliases"]["names"]
forbidden_use: dict[str, list[str]] = {
    entry["crate"]: entry["ban"] for entry in rules["forbidden_use"]
}

# ── Tier 1: Cargo.toml workspace-member edges ───────────────────
for crate, rel_manifest in manifests.items():
    manifest = WORKSPACE_DIR / rel_manifest
    if not manifest.is_file():
        error(f"missing manifest for crate '{crate}': {rel_manifest}")
        continue

    with manifest.open("rb") as f:
        data = tomllib.load(f)

    allowed = set(workspace_deps.get(crate, []))
    for name, spec in data.get("dependencies", {}).items():
        if not isinstance(spec, dict) or "path" not in spec:
            continue
        target = spec.get("package", name)
        if target in manifests and target not in allowed:
            error(
                f"{crate} depends on workspace member '{target}' "
                f"({rel_manifest}), but allowed: {sorted(allowed)}"
            )

# ── Engine forbidden external dependencies ──────────────────────
engine_manifest = WORKSPACE_DIR / manifests["engine"]
if engine_manifest.is_file():
    with engine_manifest.open("rb") as f:
        engine_data = tomllib.load(f)
    engine_deps = engine_data.get("dependencies", {})
    for dep in engine_forbidden:
        if dep in engine_deps:
            error(
                f"engine must not depend on '{dep}' "
                f"(transport/GUI concern; see engine_forbidden_deps)"
            )

# ── Collect Rust sources per workspace crate ────────────────────
def rust_sources(crate: str, *, include_tests: bool = True) -> list[Path]:
    crate_root = WORKSPACE_DIR / Path(manifests[crate]).parent
    paths: list[Path] = []
    src = crate_root / "src"
    if src.is_dir():
        paths.extend(sorted(src.rglob("*.rs")))
    if include_tests:
        tests = crate_root / "tests"
        if tests.is_dir():
            paths.extend(sorted(tests.rglob("*.rs")))
    return paths


USE_LINE = re.compile(r"^\s*(pub\s+)?use\s+")
EXTERN_LINE = re.compile(r"^\s*(pub\s+)?extern\s+crate\s+")


def banned_roots_on_line(line: str, banned: list[str]) -> list[str]:
    stripped = line.strip()
    if not (USE_LINE.match(stripped) or EXTERN_LINE.match(stripped)):
        return []
    hits: list[str] = []
    for name in banned:
        if (
            re.search(rf"^use\s+{re.escape(name)}(?:\s*;|::)", stripped)
            or re.search(rf"\bextern\s+crate\s+{re.escape(name)}\b", stripped)
            or re.search(rf"\b{re.escape(name)}::", stripped)
        ):
            hits.append(name)
    return hits


def legacy_roots_on_line(line: str, legacy: list[str]) -> list[str]:
    """Match legacy crate roots only (not submodules named `domain`)."""
    stripped = line.strip()
    if not (USE_LINE.match(stripped) or EXTERN_LINE.match(stripped)):
        return []
    hits: list[str] = []
    for name in legacy:
        if (
            re.search(rf"^use\s+{re.escape(name)}::", stripped)
            or re.search(rf"^use\s+{re.escape(name)}\s*;", stripped)
            or re.search(rf"\bextern\s+crate\s+{re.escape(name)}\b", stripped)
            or re.search(rf"(?:^use\s+.*\{{|,)\s*{re.escape(name)}::", stripped)
        ):
            hits.append(name)
    return hits


# ── Tier 2: forbidden cross-crate `use` per crate ─────────────
for crate, banned in forbidden_use.items():
    for file in rust_sources(crate):
        text = file.read_text(encoding="utf-8")
        for lineno, line in enumerate(text.splitlines(), start=1):
            hits = banned_roots_on_line(line, banned)
            hits.extend(legacy_roots_on_line(line, legacy_aliases))
            for name in dict.fromkeys(hits):
                rel = file.relative_to(WORKSPACE_DIR)
                error(
                    f"{crate}: forbidden import '{name}'\n"
                    f"  {rel}:{lineno}: {line.strip()}"
                )

# ── Tier 3 (Phase B) ────────────────────────────────────────────

def rel_posix(path: Path) -> str:
    return path.relative_to(WORKSPACE_DIR).as_posix()


# orchestration → providers symbol allowlist (src/ only)
if "orchestration_providers_allowlist" in rules:
    cfg = rules["orchestration_providers_allowlist"]
    allowed_symbols = set(cfg["symbols"])
    ban_symbols = set(cfg.get("ban_symbols", []))
    brace_import = re.compile(r"^\s*use\s+providers::\{([^}]+)\}")
    single_import = re.compile(r"^\s*use\s+providers::(\w+)")

    def providers_symbols_on_line(line: str) -> list[str]:
        stripped = line.strip()
        match = brace_import.match(stripped)
        if match:
            parts = []
            for part in match.group(1).split(","):
                token = part.strip()
                if not token or token == "self":
                    continue
                parts.append(token.split("::")[0])
            return parts
        match = single_import.match(stripped)
        if match:
            return [match.group(1)]
        return []

    for file in rust_sources("orchestration", include_tests=False):
        text = file.read_text(encoding="utf-8")
        for lineno, line in enumerate(text.splitlines(), start=1):
            for symbol in providers_symbols_on_line(line):
                if symbol in ban_symbols:
                    error(
                        "orchestration: banned providers import "
                        f"'{symbol}' (use create_provider)\n"
                        f"  {rel_posix(file)}:{lineno}: {line.strip()}"
                    )
                elif symbol not in allowed_symbols:
                    error(
                        "orchestration: providers import "
                        f"'{symbol}' not in allowlist\n"
                        f"  {rel_posix(file)}:{lineno}: {line.strip()}"
                    )

# Engine construction locality
for rule in rules.get("engine_construction", []):
    crate = rule["crate"]
    pattern = rule["pattern"]
    allowed = [p.rstrip("/") + "/" for p in rule["allowed_path_prefixes"]]
    for file in rust_sources(crate):
        rel = rel_posix(file)
        if not any(rel.startswith(prefix) for prefix in allowed):
            text = file.read_text(encoding="utf-8")
            for lineno, line in enumerate(text.splitlines(), start=1):
                if pattern in line and not line.strip().startswith("//"):
                    error(
                        f"{crate}: '{pattern}' only allowed under "
                        f"{rule['allowed_path_prefixes']}\n"
                        f"  {rel}:{lineno}: {line.strip()}"
                    )

# Orchestration domain folders must not import adapters or flat store modules.
if "orchestration_domain" in rules:
    cfg = rules["orchestration_domain"]
    forbidden_prefixes = cfg.get("forbidden_use_prefixes", [])
    forbidden_modules = cfg.get("forbidden_crate_modules", [])
    orch_src = WORKSPACE_DIR / "crates" / "orchestration" / "src"
    for folder in cfg["folders"]:
        domain_dir = orch_src / folder
        if not domain_dir.is_dir():
            continue
        for file in sorted(domain_dir.rglob("*.rs")):
            text = file.read_text(encoding="utf-8")
            for lineno, line in enumerate(text.splitlines(), start=1):
                stripped = line.strip()
                if not USE_LINE.match(stripped):
                    continue
                for prefix in forbidden_prefixes:
                    if prefix in stripped:
                        error(
                            f"orchestration domain '{folder}': forbidden adapter import\n"
                            f"  {rel_posix(file)}:{lineno}: {stripped}"
                        )
                for module in forbidden_modules:
                    if re.search(
                        rf"^use\s+crate::{re.escape(module)}(?:\s*;|::)",
                        stripped,
                    ) or re.search(
                        rf"(?:^use\s+.*\{{|,)\s*crate::{re.escape(module)}::",
                        stripped,
                    ):
                        error(
                            f"orchestration domain '{folder}': forbidden store import "
                            f"'{module}' (use a port trait; wire adapter in backend/)\n"
                            f"  {rel_posix(file)}:{lineno}: {stripped}"
                        )

# UI @tauri-apps seam
if "ui_tauri_seam" in rules:
    cfg = rules["ui_tauri_seam"]
    allowed_files = set(cfg["allowed_files"])
    allowed_globs = cfg.get("allowed_globs", [])
    ui_src = WORKSPACE_DIR / "crates" / "ui" / "src"
    tauri_import = re.compile(r"""from\s+['"]@tauri-apps/""")
    for file in sorted(ui_src.rglob("*")):
        if file.suffix not in {".ts", ".tsx"}:
            continue
        rel = rel_posix(file)
        if rel in allowed_files:
            continue
        if any(fnmatch.fnmatch(rel, pattern) for pattern in allowed_globs):
            continue
        text = file.read_text(encoding="utf-8")
        for lineno, line in enumerate(text.splitlines(), start=1):
            if tauri_import.search(line):
                error(
                    "ui: @tauri-apps import outside desktop seam "
                    f"(allowed: {sorted(allowed_files)})\n"
                    f"  {rel}:{lineno}: {line.strip()}"
                )

if ERRORS:
    print(f"\nArchitecture check failed with {ERRORS} error(s).")
    print(f"Rules: {RULES_PATH.relative_to(WORKSPACE_DIR)}")
    sys.exit(1)

print("Architecture check passed.")
print(f"Rules: {RULES_PATH.relative_to(WORKSPACE_DIR)}")
PY
