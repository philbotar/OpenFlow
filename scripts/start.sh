#!/usr/bin/env bash
# Start OpenFlow in development mode.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DESKTOP="$ROOT/crates/desktop"

"$ROOT/scripts/setup.sh"

echo "==> Starting OpenFlow"
exec npm --prefix "$DESKTOP" run start -- dev
