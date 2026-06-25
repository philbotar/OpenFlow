#!/usr/bin/env bash
# Per-crate CI gate: clippy + nextest (+ doctests for library crates) for one
# matrix leg. Logic lives here (not inline YAML) so it runs identically locally.
#
# Usage: ./scripts/ci-crate-check.sh <engine|providers|orchestration-lib|orchestration-integration|desktop>
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

leg="${1:?usage: ci-crate-check.sh <leg-name>}"

CLIPPY_LINTS=(-D warnings -D clippy::pedantic -D clippy::nursery -D clippy::cargo)

run() {
	printf '\n== %s ==\n' "$*"
	"$@"
}

clippy() { run cargo clippy "$@" --all-targets --quiet --message-format=short -- "${CLIPPY_LINTS[@]}"; }
nextest() { run cargo nextest run "$@"; }
doctest() { run cargo test --doc "$@" --quiet; }

case "$leg" in
engine)
	clippy -p engine
	nextest -p engine
	doctest -p engine
	;;
providers)
	clippy -p providers
	nextest -p providers
	doctest -p providers
	;;
orchestration-lib)
	# clippy --all-targets covers orchestration's integration targets too, so it
	# runs once here (not in the orchestration-integration leg).
	clippy -p orchestration
	nextest -p orchestration --lib
	doctest -p orchestration
	;;
orchestration-integration)
	# Integration test targets only (lib unit tests run in orchestration-lib).
	# live_workflow tests are #[ignore] and stay skipped without credentials.
	nextest -p orchestration \
		--test workflow_acceptance \
		--test workflow_authoring_acceptance \
		--test workflow_e2e \
		--test live_workflow
	;;
desktop)
	clippy -p desktop
	nextest -p desktop
	;;
*)
	echo "error: unknown leg '$leg'" >&2
	exit 1
	;;
esac
