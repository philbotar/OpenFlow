#!/usr/bin/env bash
# mutants — granular verify step (--deep). Run directly for full-output debug.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=verify/_lib.sh
. "$ROOT/scripts/verify/_lib.sh"
preflight_toolchain
require_tool cargo-mutants "cargo install cargo-mutants"
cd "$ROOT"
exec cargo mutants --no-shuffle
