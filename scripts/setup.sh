#!/usr/bin/env bash
# Install OpenFlow dev dependencies and optionally launch or build the app.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
UI="$ROOT/crates/ui"
DESKTOP="$ROOT/crates/desktop"
RUN_DEV=0
RUN_BUILD=0

usage() {
	cat <<'EOF'
Usage: ./scripts/setup.sh [options]

Install npm and Rust dependencies for local development.

Options:
  --dev     After setup, launch the desktop app in dev mode
  --build   After setup, build a release app bundle
  -h, --help
EOF
}

while [[ $# -gt 0 ]]; do
	case "$1" in
	--dev) RUN_DEV=1; shift ;;
	--build) RUN_BUILD=1; shift ;;
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

if [[ "$RUN_DEV" -eq 1 && "$RUN_BUILD" -eq 1 ]]; then
	echo "error: pass only one of --dev or --build" >&2
	exit 1
fi

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

echo "==> Checking prerequisites"
echo "    rustc  $(rustc --version)"
echo "    node   $(node --version)"
echo "    npm    $(npm --version)"

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
	cargo fetch --workspace --quiet
)

echo
echo "Setup complete."
echo
echo "Next steps:"
echo "  Dev app:     npm --prefix crates/desktop run start -- dev"
echo "  Frontend:    npm --prefix crates/ui run dev"
echo "  Verify:      ./scripts/verify.sh"
echo "  Release:     npm --prefix crates/desktop run build"
echo
echo "Tauri platform deps: https://v2.tauri.app/start/prerequisites/"

if [[ "$RUN_DEV" -eq 1 ]]; then
	echo "==> Launching OpenFlow (dev)"
	exec npm --prefix "$DESKTOP" run start -- dev
fi

if [[ "$RUN_BUILD" -eq 1 ]]; then
	echo "==> Building OpenFlow (release)"
	exec npm --prefix "$DESKTOP" run build
fi
