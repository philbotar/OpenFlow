#!/usr/bin/env bash
# Generate Tauri updater signing keys, wire pubkey into tauri.conf.json, set GitHub secret.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KEY="${TAURI_KEY_PATH:-$HOME/.tauri/openflow.key}"
TAURI_CONF="$ROOT/crates/desktop/tauri.conf.json"
REPO="${GITHUB_REPOSITORY:-philbotar/OpenFlow}"

cd "$ROOT/crates/desktop"

if [[ ! -x ../ui/node_modules/.bin/tauri ]]; then
	echo "Installing UI deps (tauri CLI)..."
	npm ci --prefix ../ui
fi

echo "Generating signing keypair at $KEY"
echo "warning: this overwrites existing keys. Skip if TAURI_SIGNING_PRIVATE_KEY is already on GitHub."
echo "         Use ./scripts/sync-tauri-pubkey.sh instead to match an existing keypair."
CI=1 ../ui/node_modules/.bin/tauri signer generate --write-keys "$KEY" --force --ci

PUBKEY="$(tr -d '\n' < "${KEY}.pub")"
jq --arg pk "$PUBKEY" '.plugins.updater.pubkey = $pk' "$TAURI_CONF" >"${TAURI_CONF}.tmp"
mv "${TAURI_CONF}.tmp" "$TAURI_CONF"

echo "Updated plugins.updater.pubkey in crates/desktop/tauri.conf.json"

if ! command -v gh >/dev/null; then
	echo "Install GitHub CLI (gh), then run:" >&2
	echo "  gh secret set TAURI_SIGNING_PRIVATE_KEY --repo $REPO < $KEY" >&2
	exit 1
fi

gh secret set TAURI_SIGNING_PRIVATE_KEY --repo "$REPO" <"$KEY"
echo "Set GitHub secret TAURI_SIGNING_PRIVATE_KEY on $REPO"
echo
echo "Next: commit the pubkey change and push:"
echo "  git add crates/desktop/tauri.conf.json"
echo "  git commit -m 'Update Tauri updater public key'"
echo
echo "Private key file (keep local, never commit): $KEY"
