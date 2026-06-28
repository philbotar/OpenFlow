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
  - ./scripts/verify/test-engine.sh
  - ./scripts/verify/test-providers.sh
  - ./scripts/verify/test-orchestration-lib.sh
  - ./scripts/verify/test-workspace-checks.sh
  - ./scripts/verify/ui-typecheck.sh

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
	"$@"
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

run_step "engine" "$ROOT/scripts/verify/test-engine.sh"
run_step "providers" "$ROOT/scripts/verify/test-providers.sh"
run_step "orchestration lib" "$ROOT/scripts/verify/test-orchestration-lib.sh"
run_step "workspace checks" "$ROOT/scripts/verify/test-workspace-checks.sh"
run_step "ui typecheck" "$ROOT/scripts/verify/ui-typecheck.sh"

if [[ "$RUN_EXECUTION" == "1" ]]; then
	run_step "workflow acceptance" "$ROOT/scripts/verify/test-execution.sh"
fi

if [[ "$RUN_DESKTOP" == "1" ]]; then
	run_step "desktop" "$ROOT/scripts/verify/test-desktop.sh"
fi
