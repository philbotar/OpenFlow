#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SNAPSHOT="$ROOT/crates/engine/tests/snapshots/public_api.txt"
ENGINE_CRATE="$ROOT/crates/engine"

if ! command -v cargo-public-api >/dev/null 2>&1; then
	echo "error: cargo-public-api is required (cargo install cargo-public-api)" >&2
	exit 1
fi

if ! rustup toolchain list | grep -q '^nightly'; then
	echo "error: nightly toolchain required for cargo-public-api" >&2
	exit 1
fi

TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT

(cd "$ENGINE_CRATE" && cargo +nightly public-api 2>/dev/null >"$TMP")

if [[ ! -f "$SNAPSHOT" ]]; then
	echo "error: missing snapshot at $SNAPSHOT — run:" >&2
	echo "  (cd crates/engine && cargo +nightly public-api > tests/snapshots/public_api.txt)" >&2
	exit 1
fi

if ! diff -u "$SNAPSHOT" "$TMP"; then
	echo "error: engine public API changed — update crates/engine/tests/snapshots/public_api.txt if intentional" >&2
	exit 1
fi

echo "Engine public API matches snapshot."
