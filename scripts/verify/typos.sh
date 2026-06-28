#!/usr/bin/env bash
# typos — granular verify step. Run directly for full-output debug.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=verify/_lib.sh
. "$ROOT/scripts/verify/_lib.sh"
require_tool typos "cargo install typos-cli"
cd "$ROOT"
exec typos
