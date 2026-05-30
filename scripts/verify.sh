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

