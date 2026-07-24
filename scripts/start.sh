#!/usr/bin/env bash
# Start OpenFlow in development mode.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DESKTOP="$ROOT/crates/desktop"
UI="$ROOT/crates/ui"

# shellcheck source=verify/_lib.sh
. "$ROOT/scripts/verify/_lib.sh"
preflight_toolchain

if [[ ! -d "$UI/node_modules" ]] || ! command -v cargo >/dev/null 2>&1; then
	echo "==> First-time deps missing — running setup"
	"$ROOT/scripts/setup.sh"
else
	echo "==> Deps present — skipping setup (run ./scripts/setup.sh to refresh)"
fi

echo "==> Starting OpenFlow"
exec npm --prefix "$DESKTOP" run start -- dev
