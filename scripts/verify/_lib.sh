#!/usr/bin/env bash
# Shared preflight for scripts/verify/*.sh granular steps.
ROOT="${ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"

preflight_toolchain() {
	if ! command -v cargo >/dev/null 2>&1; then
		# shellcheck disable=SC1090
		source "$HOME/.cargo/env"
	fi
	if ! command -v cargo >/dev/null 2>&1; then
		echo "error: cargo not found — install Rust via https://rustup.rs" >&2
		exit 1
	fi
}

preflight_npm() {
	if ! command -v npm >/dev/null 2>&1; then
		echo "error: npm is required for UI steps (crates/ui)" >&2
		return 1
	fi
	if [[ ! -d "$ROOT/crates/ui/node_modules" ]]; then
		npm --prefix "$ROOT/crates/ui" ci
	fi
}

require_tool() {
	local bin="$1"
	local install_hint="$2"
	if ! command -v "$bin" >/dev/null 2>&1; then
		echo "error: $bin is required ($install_hint)" >&2
		exit 1
	fi
}
