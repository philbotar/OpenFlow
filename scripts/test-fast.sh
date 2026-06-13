#!/usr/bin/env bash
# Fast local test lane. Skips desktop/Tauri by default because that compile path dominates loop time.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUN_EXECUTION=0
RUN_DESKTOP=0

usage() {
	cat <<'EOF'
Usage: ./scripts/test-fast.sh [--execution] [--desktop]

Default lane:
  - cargo test -p engine
  - cargo test -p providers
  - cargo test -p orchestration --lib
  - cargo test -p workspace-checks
  - npm --prefix crates/ui run typecheck

Options:
  --execution  Add deterministic workflow acceptance coverage.
  --desktop    Add desktop/Tauri command coverage.
  -h, --help   Show this help text.
EOF
}

run_step() {
	local label="$1"
	shift
	printf '\n== %s ==\n' "$label"
	(
		cd "$ROOT"
		"$@"
	)
}

while [[ $# -gt 0 ]]; do
	case "$1" in
	--execution)
		RUN_EXECUTION=1
		shift
		;;
	--desktop)
		RUN_DESKTOP=1
		shift
		;;
	-h | --help)
		usage
		exit 0
		;;
	*)
		echo "error: unknown arg '$1'" >&2
		usage >&2
		exit 1
		;;
	esac
done

run_step "engine" cargo test -p engine --quiet
run_step "providers" cargo test -p providers --quiet
run_step "orchestration lib" cargo test -p orchestration --lib --quiet
run_step "workspace checks" cargo test -p workspace-checks --quiet
run_step "ui typecheck" npm --prefix crates/ui run typecheck

if [[ "$RUN_EXECUTION" == "1" ]]; then
	run_step \
		"workflow acceptance" \
		cargo test -p orchestration --test workflow_acceptance -- --nocapture
fi

if [[ "$RUN_DESKTOP" == "1" ]]; then
	run_step "desktop" cargo test -p desktop --quiet
fi
