#!/usr/bin/env bash
# Keep desktop app version manifests aligned and print post-merge release steps.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DESKTOP_CARGO=crates/desktop/Cargo.toml
DESKTOP_TAURI=crates/desktop/tauri.conf.json
DESKTOP_PACKAGE=crates/desktop/package.json

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

json_version_file() {
	sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$1" | head -1
}

version_gt() {
	[[ "$1" != "$2" && "$(printf '%s\n' "$1" "$2" | sort -V | head -1)" == "$1" ]]
}

cargo_version_git() {
	git show "$1:$2" | awk '
		/^\[package\]/ { p=1; next }
		p && /^\[/ { exit }
		p && /^version[[:space:]]*=/ {
			gsub(/.*version[[:space:]]*=[[:space:]]*"|".*/, "")
			print
			exit
		}'
}

cargo_ver=$(cargo_version_file "$DESKTOP_CARGO")
tauri_ver=$(json_version_file "$DESKTOP_TAURI")
package_ver=$(json_version_file "$DESKTOP_PACKAGE")

if [[ "$cargo_ver" != "$tauri_ver" || "$cargo_ver" != "$package_ver" ]]; then
	echo "error: desktop app version manifests are out of sync" >&2
	echo "  $DESKTOP_CARGO: $cargo_ver" >&2
	echo "  $DESKTOP_TAURI: $tauri_ver" >&2
	echo "  $DESKTOP_PACKAGE: $package_ver" >&2
	echo "Set the same semver in all three files." >&2
	exit 1
fi

BASE="${VERSION_CHECK_BASE:-}"
if [[ -z "$BASE" ]]; then
	BASE="$(git merge-base HEAD origin/main 2>/dev/null || git merge-base HEAD main)"
fi
HEAD="${VERSION_CHECK_HEAD:-HEAD}"

base_ver=$(cargo_version_git "$BASE" "$DESKTOP_CARGO")
head_ver=$(cargo_version_git "$HEAD" "$DESKTOP_CARGO")

if version_gt "$base_ver" "$head_ver"; then
	printf '\nRelease-ready PR: desktop app version %s -> %s\n' "$base_ver" "$head_ver"
	cat <<EOF

After merge (maintainer):
  git checkout main && git pull
  git tag v${head_ver}
  git push origin v${head_ver}
  # wait for Release workflow, then publish the draft GitHub Release

Users on older builds will see the Settings update badge once the release is published.
EOF
fi

echo "Release version check passed (desktop app version: ${cargo_ver})."
