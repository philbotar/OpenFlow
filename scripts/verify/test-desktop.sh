#!/usr/bin/env bash
# test-desktop — granular test-fast leg. Run directly for full-output debug.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=verify/_lib.sh
. "$ROOT/scripts/verify/_lib.sh"
preflight_toolchain
preflight_nextest
preflight_tauri_deps
cd "$ROOT"
exec cargo nextest run -p desktop --cargo-quiet
