#!/usr/bin/env bash
# Validate Tauri updater signing: local keypair, pubkey in config, optional sign smoke test.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KEY="${TAURI_KEY_PATH:-$HOME/.tauri/openflow.key}"
TAURI_CONF="$ROOT/crates/desktop/tauri.conf.json"
REPO="${GITHUB_REPOSITORY:-philbotar/OpenFlow}"
TAURI_BIN="$ROOT/crates/ui/node_modules/.bin/tauri"

fail() {
	echo "FAIL: $*" >&2
	exit 1
}
pass() {
	echo "OK: $*"
}

[[ -f "$KEY" ]] || fail "private key not found at $KEY (set TAURI_KEY_PATH or run ./scripts/setup-tauri-signing.sh)"

[[ -f "${KEY}.pub" ]] || fail "public key not found at ${KEY}.pub"

[[ -f "$TAURI_CONF" ]] || fail "missing $TAURI_CONF"

KEY_HEAD="$(head -1 "$KEY")"
[[ "$KEY_HEAD" == untrusted\ comment:* ]] || [[ "$KEY_HEAD" == dW50cnVzdGVk* ]] || \
	fail "private key format looks wrong (paste the entire .key file into GitHub secret)"

CONFIG_PUBKEY="$(jq -r '.plugins.updater.pubkey' "$TAURI_CONF")"
FILE_PUBKEY="$(tr -d '\n' < "${KEY}.pub")"

[[ "$CONFIG_PUBKEY" == "$FILE_PUBKEY" ]] || {
	echo "pubkey in tauri.conf.json does not match ${KEY}.pub" >&2
	echo "  config: ${CONFIG_PUBKEY:0:48}..." >&2
	echo "  file:   ${FILE_PUBKEY:0:48}..." >&2
	echo >&2
	echo "The GitHub secret must pair with the pubkey baked into the app." >&2
	echo "Do not run setup-tauri-signing.sh (that generates new keys)." >&2
	echo "Run: ./scripts/sync-tauri-pubkey.sh" >&2
	fail "commit the synced pubkey, then re-run this script"
}
pass "pubkey in tauri.conf.json matches ${KEY}.pub"

if command -v gh >/dev/null; then
	if gh secret list --repo "$REPO" 2>/dev/null | awk '{print $1}' | grep -qx 'TAURI_SIGNING_PRIVATE_KEY'; then
		pass "GitHub secret TAURI_SIGNING_PRIVATE_KEY exists on $REPO"
	else
		fail "GitHub secret TAURI_SIGNING_PRIVATE_KEY not found on $REPO (gh auth?)"
	fi
else
	echo "SKIP: install gh to verify GitHub secret from CLI"
fi

[[ -x "$TAURI_BIN" ]] || {
	echo "Installing UI deps for tauri CLI..."
	npm ci --prefix "$ROOT/crates/ui"
}

TMP="$(mktemp)"
trap 'rm -f "$TMP" "$TMP.sig"' EXIT
echo "openflow-signing-smoke" >"$TMP"

export TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY")"
"$TAURI_BIN" signer sign "$TMP" >/dev/null
[[ -f "${TMP}.sig" ]] || fail "tauri signer sign did not produce ${TMP}.sig"
pass "local sign smoke test passed (${TMP}.sig created)"

cat <<EOF

Signing looks valid locally.

CI validation: push a new release tag after merge (do not retag v0.1.2):
  git tag v0.1.3 && git push origin v0.1.3

In the Release workflow log, confirm:
  - no "failed to decode secret key"
  - build produces .tar.gz and .sig under bundle artifacts
EOF
