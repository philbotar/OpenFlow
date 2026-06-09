#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1; then
	# Support shells where rustup env isn't preloaded.
	# shellcheck disable=SC1090
	source "$HOME/.cargo/env"
fi

cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo test --workspace

if ! command -v npm >/dev/null 2>&1; then
	echo "error: npm is required to run UI tests (crates/ui)" >&2
	exit 1
fi
if [[ ! -d crates/ui/node_modules ]]; then
	npm --prefix crates/ui ci
fi
npm --prefix crates/ui run test

cargo deny check
./scripts/check-architecture.sh

