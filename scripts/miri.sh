#!/usr/bin/env bash
# Run Miri (undefined-behavior interpreter) on engine + orchestration lib tests.
# See https://github.com/rust-lang/miri
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
	export CARGO_TARGET_DIR="$ROOT/target/miri"
fi

preflight_miri() {
	if ! command -v rustup >/dev/null 2>&1; then
		echo "error: rustup is required for Miri (https://rustup.rs)" >&2
		exit 1
	fi
	if ! rustup +nightly component list --installed 2>&1 | grep -q '^miri'; then
		rustup toolchain install nightly --component miri
	fi
	cargo +nightly miri setup
}

preflight_miri
cd "$ROOT"

ENGINE_ARGS=(--lib)
if [[ -z "${MIRI_ENGINE_VPROC:-}" && "$(uname -s)" == "Darwin" ]]; then
	MIRI_ENGINE_VPROC="x86_64-unknown-linux-gnu"
fi
if [[ -n "${MIRI_ENGINE_VPROC:-}" ]]; then
	ENGINE_ARGS+=(--target "$MIRI_ENGINE_VPROC")
fi

# Tokio/subprocess/integration tests carry #[cfg_attr(miri, ignore)].
echo "Miri: engine (lib, isolated)"
MIRIFLAGS="${MIRIFLAGS:--Zmiri-ignore-leaks}" \
	cargo +nightly miri test -p engine "${ENGINE_ARGS[@]}" "$@"

echo "Miri: orchestration (lib, real temp files)"
MIRIFLAGS="${MIRIFLAGS:--Zmiri-disable-isolation -Zmiri-ignore-leaks}" \
	cargo +nightly miri test -p orchestration --lib "$@"
