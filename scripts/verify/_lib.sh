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

ensure_cargo_tool() {
	local bin="$1"
	local crate="${2:-$1}"
	if command -v "$bin" >/dev/null 2>&1; then
		return 0
	fi
	echo "Installing $crate ..."
	cargo install "$crate" --locked
}

ensure_nightly_toolchain() {
	if ! command -v rustup >/dev/null 2>&1; then
		echo "error: rustup is required for nightly toolchain (https://rustup.rs)" >&2
		exit 1
	fi
	if ! rustup toolchain list | grep -q '^nightly'; then
		echo "Installing nightly toolchain ..."
		rustup toolchain install nightly
	fi
}

preflight_cargo_tools() {
	preflight_toolchain
	ensure_cargo_tool cargo-deny cargo-deny
	ensure_cargo_tool cargo-machete cargo-machete
	ensure_cargo_tool typos typos-cli
	ensure_cargo_tool cargo-public-api cargo-public-api
	ensure_nightly_toolchain
}

preflight_tauri_deps() {
	if [[ "$(uname -s)" != "Linux" ]]; then
		return 0
	fi
	if ! command -v pkg-config >/dev/null 2>&1; then
		echo "error: pkg-config is required for desktop/Tauri workspace builds on Linux" >&2
		exit 1
	fi
	if pkg-config --exists gdk-3.0 2>/dev/null; then
		return 0
	fi
	if [[ "${VERIFY_SKIP_TAURI_DEPS:-0}" == "1" ]]; then
		echo "error: gdk-3.0 dev libs missing — install Tauri Linux deps or unset VERIFY_SKIP_TAURI_DEPS to auto-install" >&2
		exit 1
	fi
	if ! command -v apt-get >/dev/null 2>&1; then
		echo "error: gdk-3.0 dev libs missing — install Tauri Linux deps (see .github/actions/install-tauri-deps/action.yml)" >&2
		exit 1
	fi
	echo "Installing Tauri Linux dev libs (gdk-3.0 missing) ..."
	sudo apt-get update -qq
	sudo apt-get install -y -qq \
		libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
	if ! pkg-config --exists gdk-3.0 2>/dev/null; then
		echo "error: gdk-3.0 still missing after installing Tauri Linux deps" >&2
		exit 1
	fi
}
