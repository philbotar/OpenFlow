#!/usr/bin/env bash
# Shared preflight for scripts/verify/*.sh granular steps.
ROOT="${ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"

# Put rust-lld (and friends) on PATH so .cargo/config.toml linker = "rust-lld" resolves.
ensure_rust_host_bin_path() {
	if ! command -v rustc >/dev/null 2>&1; then
		return 0
	fi
	local host sysroot host_bin
	host="$(rustc -vV 2>/dev/null | awk '/^host:/{print $2}')"
	sysroot="$(rustc --print sysroot 2>/dev/null)"
	host_bin="${sysroot}/lib/rustlib/${host}/bin"
	if [[ -n "$host" && -d "$host_bin" && ":$PATH:" != *":$host_bin:"* ]]; then
		export PATH="${host_bin}:${PATH}"
	fi
}

# Use sccache when installed and not already wrapped.
ensure_sccache_wrapper() {
	if [[ -n "${RUSTC_WRAPPER:-}" ]]; then
		return 0
	fi
	if command -v sccache >/dev/null 2>&1; then
		export RUSTC_WRAPPER=sccache
	fi
}

preflight_build_space() {
	local min_gib max_debug_gib available_kib available_gib
	local target_root target_debug debug_kib debug_gib
	target_root="${CARGO_TARGET_DIR:-$ROOT/target}"
	if [[ "$target_root" != /* ]]; then
		target_root="$ROOT/$target_root"
	fi
	target_debug="$target_root/debug"

	max_debug_gib="${OPENFLOW_MAX_DEBUG_CACHE_GIB:-64}"
	if [[ ! "$max_debug_gib" =~ ^[0-9]+$ ]]; then
		echo "error: OPENFLOW_MAX_DEBUG_CACHE_GIB must be a non-negative integer" >&2
		return 1
	fi
	if [[ "$max_debug_gib" != "0" ]]; then
		if [[ -n "${OPENFLOW_DEBUG_CACHE_SIZE_KIB:-}" ]]; then
			debug_kib="$OPENFLOW_DEBUG_CACHE_SIZE_KIB"
		elif [[ -d "$target_debug" ]]; then
			debug_kib="$(du -sk "$target_debug" | awk '{print $1}')"
		else
			debug_kib=0
		fi
		if [[ ! "$debug_kib" =~ ^[0-9]+$ ]]; then
			echo "error: could not determine debug cache size for $target_debug" >&2
			return 1
		fi
		if ((debug_kib > max_debug_gib * 1024 * 1024)); then
			debug_gib=$((debug_kib / 1024 / 1024))
			echo "error: refusing to start Cargo with a ${debug_gib} GiB debug cache; ${max_debug_gib} GiB maximum" >&2
			echo "rebuildable cache: $target_debug" >&2
			if [[ "$target_debug" == "$ROOT/target/debug" ]]; then
				echo "cleanup: ./scripts/clean-rust-cache.sh --yes" >&2
			else
				echo "cleanup: cargo clean --target-dir '$target_root'" >&2
			fi
			echo "override: OPENFLOW_MAX_DEBUG_CACHE_GIB=0 (unsafe; disables cache ceiling)" >&2
			return 1
		fi
	fi

	if [[ -n "${OPENFLOW_MIN_BUILD_SPACE_GIB:-}" ]]; then
		min_gib="$OPENFLOW_MIN_BUILD_SPACE_GIB"
	elif [[ "${GITHUB_ACTIONS:-false}" == "true" ]]; then
		min_gib=8
	else
		min_gib=24
	fi
	if [[ ! "$min_gib" =~ ^[0-9]+$ ]]; then
		echo "error: OPENFLOW_MIN_BUILD_SPACE_GIB must be a non-negative integer" >&2
		return 1
	fi
	if [[ "$min_gib" == "0" ]]; then
		return 0
	fi

	if [[ -n "${OPENFLOW_BUILD_SPACE_AVAILABLE_KIB:-}" ]]; then
		available_kib="$OPENFLOW_BUILD_SPACE_AVAILABLE_KIB"
	else
		available_kib="$(df -Pk "$ROOT" | awk 'END {print $4}')"
	fi
	if [[ ! "$available_kib" =~ ^[0-9]+$ ]]; then
		echo "error: could not determine free disk space for $ROOT" >&2
		return 1
	fi

	if ((available_kib >= min_gib * 1024 * 1024)); then
		return 0
	fi

	available_gib=$((available_kib / 1024 / 1024))
	echo "error: refusing to start Cargo with only ${available_gib} GiB free; ${min_gib} GiB required" >&2
	echo "rebuildable cache: $target_debug" >&2
	if [[ "$target_debug" == "$ROOT/target/debug" ]]; then
		echo "cleanup: ./scripts/clean-rust-cache.sh --yes" >&2
	else
		echo "cleanup: cargo clean --target-dir '$target_root'" >&2
	fi
	echo "override: OPENFLOW_MIN_BUILD_SPACE_GIB=0 (unsafe; disables free-space floor)" >&2
	return 1
}

preflight_toolchain() {
	if ! command -v cargo >/dev/null 2>&1; then
		# shellcheck disable=SC1090
		source "$HOME/.cargo/env"
	fi
	if ! command -v cargo >/dev/null 2>&1; then
		echo "error: cargo not found — install Rust via https://rustup.rs" >&2
		exit 1
	fi
	ensure_rust_host_bin_path
	ensure_sccache_wrapper
	preflight_build_space
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

preflight_nextest() {
	preflight_toolchain
	if cargo nextest --version >/dev/null 2>&1; then
		return 0
	fi
	echo "Installing cargo-nextest ..."
	cargo install cargo-nextest --locked
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
