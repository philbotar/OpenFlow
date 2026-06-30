#!/usr/bin/env bash
# deny — granular verify step. Run directly for full-output debug.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=verify/_lib.sh
. "$ROOT/scripts/verify/_lib.sh"
preflight_toolchain
preflight_cargo_tools
cd "$ROOT"
exec cargo deny check
