#!/usr/bin/env bash
# Install OpenFlow dev dependencies.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
UI="$ROOT/crates/ui"

usage() {
	cat <<'EOF'
Usage: ./scripts/setup.sh

Install npm and Rust dependencies for local development.
Run ./scripts/start.sh to launch the app, or ./scripts/install.sh to build an installer.
EOF
}

while [[ $# -gt 0 ]]; do
	case "$1" in
	-h | --help)
		usage
		exit 0
		;;
	*)
		echo "error: unknown option: $1" >&2
		usage >&2
		exit 1
		;;
	esac
done

need() {
	if ! command -v "$1" >/dev/null 2>&1; then
		echo "error: $1 not found — $2" >&2
		exit 1
	fi
}

need rustc "install Rust via https://rustup.rs"
need cargo "install Rust via https://rustup.rs"
need node "install Node.js 18+ via https://nodejs.org"
need npm "install Node.js 18+ via https://nodejs.org"

# shellcheck source=verify/_lib.sh
. "$ROOT/scripts/verify/_lib.sh"
ensure_rust_host_bin_path
ensure_sccache_wrapper

echo "==> Checking prerequisites"
echo "    rustc  $(rustc --version)"
echo "    node   $(node --version)"
echo "    npm    $(npm --version)"
if command -v rust-lld >/dev/null 2>&1; then
	echo "    linker rust-lld ($(command -v rust-lld))"
else
	echo "warning: rust-lld not on PATH — .cargo/config.toml expects it; open a new shell after rustup install"
fi
if command -v sccache >/dev/null 2>&1; then
	echo "    sccache $(sccache --version 2>/dev/null | head -1)"
else
	echo "    tip: install sccache for faster rebuilds (brew install sccache / cargo install sccache --locked)"
fi

if [[ "$(uname -s)" == "Darwin" ]]; then
	if ! xcode-select -p >/dev/null 2>&1; then
		echo "warning: Xcode Command Line Tools not found — required for Tauri on macOS"
		echo "         run: xcode-select --install"
	fi
fi

echo "==> Installing UI dependencies"
if [[ -f "$UI/package-lock.json" ]]; then
	npm ci --prefix "$UI"
else
	npm install --prefix "$UI"
fi

echo "==> Fetching Rust workspace crates"
(
	cd "$ROOT"
	cargo fetch --quiet
)

echo "==> Ensuring cargo-nextest"
if ! cargo nextest --version >/dev/null 2>&1; then
	cargo install cargo-nextest --locked
fi

echo
echo "Setup complete."
echo
echo "Next steps:"
echo "  Fast check:  ./scripts/check-fast.sh"
echo "  Fast tests:  ./scripts/test-fast.sh"
echo "  Run app:     ./scripts/start.sh"
echo "  Install app: ./scripts/install.sh"
echo "  Verify:      ./scripts/verify.sh"
echo
echo "Note: providers/orchestration omit Bedrock (AWS SDK) by default."
echo "      Desktop enables it. Bedrock tests: cargo nextest run -p providers --features bedrock"
echo
echo "Tauri platform deps: https://v2.tauri.app/start/prerequisites/"
