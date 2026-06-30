#!/usr/bin/env bash
# doc — granular verify step. Run directly for full-output debug.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=verify/_lib.sh
. "$ROOT/scripts/verify/_lib.sh"
preflight_toolchain
preflight_tauri_deps
cd "$ROOT"
exec env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --quiet
