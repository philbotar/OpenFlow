#!/usr/bin/env bash
# Copy existing ~/.tauri/openflow.key.pub into tauri.conf.json (does not regenerate keys).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KEY="${TAURI_KEY_PATH:-$HOME/.tauri/openflow.key}"
TAURI_CONF="$ROOT/crates/desktop/tauri.conf.json"

[[ -f "${KEY}.pub" ]] || {
	echo "error: ${KEY}.pub not found" >&2
	exit 1
}

PUBKEY="$(tr -d '\n' < "${KEY}.pub")"
jq --arg pk "$PUBKEY" '.plugins.updater.pubkey = $pk' "$TAURI_CONF" >"${TAURI_CONF}.tmp"
mv "${TAURI_CONF}.tmp" "$TAURI_CONF"

echo "Updated plugins.updater.pubkey from ${KEY}.pub"
echo
echo "Next:"
echo "  ./scripts/validate-tauri-signing.sh"
echo "  git add crates/desktop/tauri.conf.json"
echo "  git commit -m 'Sync Tauri updater public key'"
echo "  git push"
