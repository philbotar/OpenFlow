#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DEBUG="$ROOT/target/debug"
CONFIRMED=0

usage() {
	cat <<'EOF'
Usage: ./scripts/clean-rust-cache.sh --yes

Delete this checkout's rebuildable target/debug cache. Source files, Git state,
release artifacts, target/miri, and cross-compiled targets remain untouched.
EOF
}

while [[ $# -gt 0 ]]; do
	case "$1" in
	--yes)
		CONFIRMED=1
		shift
		;;
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

if [[ "$CONFIRMED" != "1" ]]; then
	echo "error: pass --yes to delete $TARGET_DEBUG" >&2
	exit 1
fi
if [[ "$ROOT" == "/" || ! -f "$ROOT/Cargo.toml" || "$TARGET_DEBUG" != "$ROOT/target/debug" ]]; then
	echo "error: refusing unsafe cache target: $TARGET_DEBUG" >&2
	exit 1
fi
if [[ ! -e "$TARGET_DEBUG" ]]; then
	echo "Rust debug cache already absent: $TARGET_DEBUG"
	exit 0
fi

echo "Deleting rebuildable Rust debug cache: $TARGET_DEBUG"
/bin/rm -rf "$TARGET_DEBUG"
echo "Deleted. Next Rust build will be cold."
