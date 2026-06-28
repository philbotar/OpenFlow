#!/usr/bin/env bash
# Run Miri (undefined-behavior interpreter) on engine and/or orchestration lib tests.
# See https://github.com/rust-lang/miri and https://nexte.st/docs/integrations/miri/
#
# Usage:
#   ./scripts/miri.sh                 # both crates (local / verify --deep)
#   ./scripts/miri.sh engine          # one crate (CI matrix leg)
#   ./scripts/miri.sh orchestration
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MIRI_TOOLCHAIN="${MIRI_TOOLCHAIN:-nightly}"

if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
	export CARGO_TARGET_DIR="$ROOT/target/miri"
fi

# UB-relevant orchestration modules only; pure edit/patch/store logic stays on test-fast/clippy.
ORCH_MIRI_FILTER='test(/run::execution::/) | test(/coordinator/) | test(/tool::runner/) | test(/tool::blocking_ops/) | test(/tool::retry/) | test(/schedule::/) | test(/adapters::infrastructure::/)'

preflight_nextest() {
	if ! cargo nextest --version >/dev/null 2>&1; then
		cargo install cargo-nextest --locked
	fi
}

preflight_miri() {
	if ! command -v rustup >/dev/null 2>&1; then
		echo "error: rustup is required for Miri (https://rustup.rs)" >&2
		exit 1
	fi
	if ! rustup "+$MIRI_TOOLCHAIN" component list --installed 2>&1 | grep -q '^miri'; then
		rustup toolchain install "$MIRI_TOOLCHAIN" --component miri
	fi
	cargo "+$MIRI_TOOLCHAIN" miri setup
	preflight_nextest
}

run_engine() {
	local -a engine_args=(--lib)
	if [[ -z "${MIRI_ENGINE_VPROC:-}" && "$(uname -s)" == "Darwin" ]]; then
		MIRI_ENGINE_VPROC="x86_64-unknown-linux-gnu"
	fi
	if [[ -n "${MIRI_ENGINE_VPROC:-}" ]]; then
		engine_args+=(--target "$MIRI_ENGINE_VPROC")
	fi
	# Tokio/subprocess/integration tests carry #[cfg_attr(miri, ignore)].
	echo "Miri: engine (lib, isolated)"
	local -a nextest_cmd=(
		cargo "+$MIRI_TOOLCHAIN" miri nextest run -p engine "${engine_args[@]}"
		--profile default-miri
	)
	if [[ -n "${MIRI_JOBS:-}" ]]; then
		nextest_cmd+=(-j "$MIRI_JOBS")
	fi
	MIRIFLAGS="${MIRIFLAGS:--Zmiri-ignore-leaks}" \
		"${nextest_cmd[@]}" "$@"
}

run_orchestration() {
	echo "Miri: orchestration (lib, UB-relevant allowlist, real temp files)"
	local -a nextest_cmd=(
		cargo "+$MIRI_TOOLCHAIN" miri nextest run -p orchestration --lib
		--profile default-miri
		-E "$ORCH_MIRI_FILTER"
	)
	if [[ -n "${MIRI_JOBS:-}" ]]; then
		nextest_cmd+=(-j "$MIRI_JOBS")
	fi
	MIRIFLAGS="${MIRIFLAGS:--Zmiri-disable-isolation -Zmiri-ignore-leaks}" \
		"${nextest_cmd[@]}" "$@"
}

preflight_miri
cd "$ROOT"

crate="${1:-all}"
shift || true

case "$crate" in
all)
	run_engine "$@"
	run_orchestration "$@"
	;;
engine) run_engine "$@" ;;
orchestration) run_orchestration "$@" ;;
*)
	echo "error: unknown crate '$crate' (expected engine, orchestration, or all)" >&2
	exit 1
	;;
esac
