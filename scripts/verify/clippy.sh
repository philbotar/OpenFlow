#!/usr/bin/env bash
# clippy — granular verify step. Run directly for full-output debug.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=verify/_lib.sh
. "$ROOT/scripts/verify/_lib.sh"
preflight_toolchain
preflight_tauri_deps
cd "$ROOT"
exec cargo clippy --workspace --all-targets --quiet --message-format=short -- \
	-D warnings -D clippy::pedantic -D clippy::nursery -D clippy::cargo
