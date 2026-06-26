#!/usr/bin/env bash
# Build OpenFlow and open the macOS drag-to-Applications installer when available.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DESKTOP="$ROOT/crates/desktop"
BUNDLE_ROOT="$DESKTOP/target/release/bundle"

"$ROOT/scripts/setup.sh"

if [[ "$(uname -s)" == "Darwin" ]]; then
	echo "==> Building OpenFlow installer (.dmg)"
	npm --prefix "$DESKTOP" run build -- --bundles dmg
	DMG="$(find "$BUNDLE_ROOT/dmg" -maxdepth 1 -name '*.dmg' -print -quit)"
	if [[ -z "$DMG" ]]; then
		echo "error: DMG not found under $BUNDLE_ROOT/dmg" >&2
		exit 1
	fi
	echo "==> Opening installer — drag OpenFlow to Applications"
	open "$DMG"
else
	echo "==> Building OpenFlow release bundle"
	npm --prefix "$DESKTOP" run build
	echo
	echo "Built app bundle: $BUNDLE_ROOT"
fi
