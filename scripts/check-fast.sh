#!/usr/bin/env bash
# Crate-scoped cargo check / clippy for the edit loop (skips workspace + desktop).
# Usage:
#   ./scripts/check-fast.sh                 # check engine providers orchestration
#   ./scripts/check-fast.sh engine          # one crate
#   ./scripts/check-fast.sh --clippy engine
#   ./scripts/check-fast.sh -p orchestration --clippy
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=verify/_lib.sh
. "$ROOT/scripts/verify/_lib.sh"

RUN_CLIPPY=0
PACKAGES=()

usage() {
	cat <<'EOF'
Usage: ./scripts/check-fast.sh [--clippy] [crate ...]

Default crates: engine providers orchestration
Options:
  --clippy       Run clippy -p <crate> --all-targets -D warnings (not workspace)
  -p <crate>     Same as passing <crate> positionally
  -h, --help     Show help

Examples:
  ./scripts/check-fast.sh
  ./scripts/check-fast.sh engine
  ./scripts/check-fast.sh --clippy -p orchestration
EOF
}

while [[ $# -gt 0 ]]; do
	case "$1" in
	--clippy)
		RUN_CLIPPY=1
		shift
		;;
	-p)
		PACKAGES+=("$2")
		shift 2
		;;
	-h | --help)
		usage
		exit 0
		;;
	-*)
		echo "error: unknown option: $1" >&2
		usage >&2
		exit 1
		;;
	*)
		PACKAGES+=("$1")
		shift
		;;
	esac
done

if [[ ${#PACKAGES[@]} -eq 0 ]]; then
	PACKAGES=(engine providers orchestration)
fi

preflight_toolchain
cd "$ROOT"

for pkg in "${PACKAGES[@]}"; do
	case "$pkg" in
	engine | providers | orchestration | desktop | workspace-checks) ;;
	*)
		echo "error: unknown crate '$pkg'" >&2
		exit 1
		;;
	esac
	if [[ "$RUN_CLIPPY" == "1" ]]; then
		printf '== clippy -p %s ==\n' "$pkg"
		cargo clippy -p "$pkg" --all-targets --quiet --message-format=short -- -D warnings
	else
		printf '== check -p %s ==\n' "$pkg"
		cargo check -p "$pkg" --quiet --message-format=short
	fi
done
