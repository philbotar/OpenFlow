#!/usr/bin/env bash
# Print wall-clock time per Rust test leg (warm ./target assumed).
# Usage: ./scripts/bench-test-legs.sh [--with-opt-in]
#   --with-opt-in  also time desktop + full workspace (slow)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WITH_OPT_IN=0
LOG_DIR="$(mktemp -d)"
trap 'rm -rf "$LOG_DIR"' EXIT

usage() {
	cat <<'EOF'
Usage: ./scripts/bench-test-legs.sh [--with-opt-in]

Times each test-fast leg against ./target. Pass --with-opt-in to also
time desktop and full workspace (includes Tauri).

Avoid running while rust-analyzer / another cargo holds target/.cargo-lock.
EOF
}

while [[ $# -gt 0 ]]; do
	case "$1" in
	--with-opt-in)
		WITH_OPT_IN=1
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

LEGS=(
	test-engine
	test-providers
	test-orchestration-lib
	test-workspace-checks
	test-execution
)

if [[ "$WITH_OPT_IN" == "1" ]]; then
	LEGS+=(test-desktop test)
fi

printf '== test leg wall times (cwd=%s) ==\n' "$ROOT"
printf '%-28s %8s  %s\n' LEG SECONDS STATUS
printf '%-28s %8s  %s\n' --- ------- ------

for leg in "${LEGS[@]}"; do
	script="$ROOT/scripts/verify/${leg}.sh"
	log="$LOG_DIR/${leg}.log"
	if [[ ! -f "$script" ]]; then
		printf '%-28s %8s  %s\n' "$leg" "-" "MISSING"
		continue
	fi
	printf '… %s\n' "$leg" >&2
	start=$(date +%s)
	# ponytail: avoid zsh readonly `status`
	leg_status=ok
	if ! "$script" >"$log" 2>&1; then
		leg_status=FAIL
	fi
	end=$(date +%s)
	dur=$((end - start))
	printf '%-28s %7ss  %s\n' "$leg" "$dur" "$leg_status"
	if [[ "$leg_status" == "FAIL" ]]; then
		printf '  (last 15 lines of %s)\n' "$log" >&2
		tail -n 15 "$log" | sed 's/^/  /' >&2
	fi
done
