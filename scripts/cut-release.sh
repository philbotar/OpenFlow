#!/usr/bin/env bash
# Bump desktop app patch version across canonical manifests (release train cut).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DESKTOP_CARGO=crates/desktop/Cargo.toml
DESKTOP_TAURI=crates/desktop/tauri.conf.json
DESKTOP_PACKAGE=crates/desktop/package.json

DRY_RUN=0
EXPLICIT_VERSION=""

usage() {
	cat <<'EOF'
Usage: ./scripts/cut-release.sh [--dry-run] [VERSION]

Bump the desktop app version in tauri.conf.json, Cargo.toml, and package.json.

Use after merging feature PRs to main when you are ready to ship one release.
Feature PRs should not bump the desktop version; run this script (or open a
release PR with its output) instead.

  --dry-run    Print the planned bump and tag steps without writing files
  VERSION      Set an explicit semver instead of auto patch-bump

After committing the bump on main:

  git tag vX.Y.Z && git push origin vX.Y.Z
EOF
}

while [[ $# -gt 0 ]]; do
	case "$1" in
	--dry-run)
		DRY_RUN=1
		shift
		;;
	-h | --help)
		usage
		exit 0
		;;
	-*)
		echo "error: unknown option $1" >&2
		usage >&2
		exit 1
		;;
	*)
		EXPLICIT_VERSION=$1
		shift
		;;
	esac
done

json_version_file() {
	jq -r '.version' "$1"
}

cargo_version_file() {
	awk '
		/^\[package\]/ { p=1; next }
		p && /^\[/ { exit }
		p && /^version[[:space:]]*=/ {
			gsub(/.*version[[:space:]]*=[[:space:]]*"|".*/, "")
			print
			exit
		}' "$1"
}

bump_patch() {
	local v=$1 major minor patch
	IFS=. read -r major minor patch <<<"$v"
	[[ -n "$major" && -n "$minor" && -n "$patch" ]] || {
		echo "error: expected semver X.Y.Z, got $v" >&2
		exit 1
	}
	echo "${major}.${minor}.$((patch + 1))"
}

current=$(json_version_file "$DESKTOP_TAURI")
cargo_ver=$(cargo_version_file "$DESKTOP_CARGO")
package_ver=$(json_version_file "$DESKTOP_PACKAGE")

if [[ "$current" != "$cargo_ver" || "$current" != "$package_ver" ]]; then
	echo "error: desktop app version manifests are out of sync" >&2
	echo "  $DESKTOP_TAURI: $current" >&2
	echo "  $DESKTOP_CARGO: $cargo_ver" >&2
	echo "  $DESKTOP_PACKAGE: $package_ver" >&2
	exit 1
fi

next=${EXPLICIT_VERSION:-$(bump_patch "$current")}

if [[ "$next" == "$current" ]]; then
	echo "error: new version must differ from $current" >&2
	exit 1
fi

printf 'Desktop app release cut: %s -> %s\n' "$current" "$next"

if ((DRY_RUN)); then
	printf '\n(dry run — files unchanged)\n'
else
	jq --arg v "$next" '.version = $v' "$DESKTOP_TAURI" >"${DESKTOP_TAURI}.tmp"
	mv "${DESKTOP_TAURI}.tmp" "$DESKTOP_TAURI"

	sed -i "s/^version = \".*\"/version = \"${next}\"/" "$DESKTOP_CARGO"
	jq --arg v "$next" '.version = $v' "$DESKTOP_PACKAGE" >"${DESKTOP_PACKAGE}.tmp"
	mv "${DESKTOP_PACKAGE}.tmp" "$DESKTOP_PACKAGE"
fi

cat <<EOF

Next on main:
  git add crates/desktop/tauri.conf.json crates/desktop/Cargo.toml crates/desktop/package.json
  git commit -m "Release desktop app v${next}"
  git push origin main
  git tag v${next}
  git push origin v${next}
  # wait for Release workflow, then publish the draft GitHub Release
EOF
