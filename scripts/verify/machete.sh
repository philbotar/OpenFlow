#!/usr/bin/env bash
# machete — granular verify step. Run directly for full-output debug.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=verify/_lib.sh
. "$ROOT/scripts/verify/_lib.sh"
preflight_toolchain
require_tool cargo-machete "cargo install cargo-machete"
cd "$ROOT"
exec cargo machete
