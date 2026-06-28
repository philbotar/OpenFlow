#!/usr/bin/env bash
# ui-typecheck — granular verify step. Run directly for full-output debug.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=verify/_lib.sh
. "$ROOT/scripts/verify/_lib.sh"
preflight_npm
cd "$ROOT"
exec npm --prefix crates/ui run typecheck
