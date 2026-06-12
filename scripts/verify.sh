#!/usr/bin/env bash
# Verification gate for agents and CI. Runs all steps by default; surfaces every failure.
set -uo pipefail

VERIFY_MAX_LINES="${VERIFY_MAX_LINES:-150}"
VERIFY_FAIL_FAST="${VERIFY_FAIL_FAST:-0}"
DEEP=0
REQUESTED_STEPS=()

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="$(mktemp -d)"
trap 'rm -rf "$LOG_DIR"' EXIT

export CARGO_TERM_COLOR=never
export CARGO_TERM_PROGRESS_WHEN=never
export NO_COLOR=1
export CI=true
export FORCE_COLOR=0

ALL_STEPS=(
	fmt
	clippy
	doc
	test
	public-api
	machete
	typos
	ui-typecheck
	ui-test
	deny
	arch
)

RAN_STEPS=()
STEP_STATUS=()
FAILED_COUNT=0

step_repro() {
	case "$1" in
	fmt) printf '%s' 'cargo fmt --all --check' ;;
	clippy)
		printf '%s' 'cargo clippy --workspace --all-targets --quiet --message-format=short -- -D warnings -D clippy::pedantic -D clippy::nursery -D clippy::cargo'
		;;
	doc) printf '%s' 'RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --quiet' ;;
	test) printf '%s' 'cargo test --workspace --quiet' ;;
	public-api) printf '%s' './scripts/check-engine-public-api.sh' ;;
	machete) printf '%s' 'cargo machete' ;;
	typos) printf '%s' 'typos' ;;
	ui-typecheck) printf '%s' 'npm --prefix crates/ui run typecheck' ;;
	ui-test) printf '%s' 'npm --prefix crates/ui run test' ;;
	deny) printf '%s' 'cargo deny check' ;;
	arch) printf '%s' './scripts/check-architecture.sh' ;;
	mutants) printf '%s' 'cargo mutants --no-shuffle' ;;
	*) printf '%s' "$1" ;;
	esac
}

is_valid_step() {
	case "$1" in
	fmt | clippy | doc | test | public-api | machete | typos | ui-typecheck | ui-test | deny | arch | mutants)
		return 0
		;;
	*) return 1 ;;
	esac
}

usage_steps() {
	printf 'Valid steps: %s\n' "${ALL_STEPS[*]} mutants (--deep only)"
}

preflight_toolchain() {
	if ! command -v cargo >/dev/null 2>&1; then
		# shellcheck disable=SC1090
		source "$HOME/.cargo/env"
	fi
	if ! command -v cargo >/dev/null 2>&1; then
		echo "error: cargo not found — install Rust via https://rustup.rs" >&2
		exit 1
	fi
}

preflight_npm() {
	if ! command -v npm >/dev/null 2>&1; then
		echo "error: npm is required for UI steps (crates/ui)" >&2
		return 1
	fi
	if [[ ! -d "$ROOT/crates/ui/node_modules" ]]; then
		npm --prefix "$ROOT/crates/ui" ci
	fi
}

record_step_result() {
	local name="$1"
	local status="$2"
	local duration="$3"
	RAN_STEPS+=("$name")
	STEP_STATUS+=("$status")
	if [[ "$status" == "FAIL" ]]; then
		FAILED_COUNT=$((FAILED_COUNT + 1))
		printf 'FAIL  %s  (%ss)\n' "$name" "$duration"
	else
		printf 'PASS  %s  (%ss)\n' "$name" "$duration"
	fi
}

fail_step() {
	local name="$1"
	local message="$2"
	local log="$LOG_DIR/${name}.log"
	printf '%s\n' "$message" >"$log"
	record_step_result "$name" "FAIL" "0"
	printf '%s\n' '--- output ---'
	printf '%s\n' "$message"
	if [[ "$VERIFY_FAIL_FAST" == "1" ]]; then
		print_summary
		exit 1
	fi
}

run_step() {
	local name="$1"
	shift
	local log="$LOG_DIR/${name}.log"
	local start end duration exit_code line_count

	start=$(date +%s)
	set +o pipefail
	( cd "$ROOT" && "$@" ) >"$log" 2>&1
	exit_code=$?
	set -o pipefail
	end=$(date +%s)
	duration=$((end - start))

	if [[ $exit_code -eq 0 ]]; then
		record_step_result "$name" "PASS" "$duration"
	else
		record_step_result "$name" "FAIL" "$duration"
		line_count=$(wc -l <"$log" | tr -d ' ')
		if [[ "$line_count" -gt "$VERIFY_MAX_LINES" ]]; then
			printf '%s\n' "--- last $VERIFY_MAX_LINES lines ($line_count total; full log: $log) ---"
		else
			printf '%s\n' '--- output ---'
		fi
		# Use cat (not printf) so diff/cargo lines starting with `-` are not parsed as flags.
		tail -n "$VERIFY_MAX_LINES" "$log" | cat
		if [[ "$VERIFY_FAIL_FAST" == "1" ]]; then
			print_summary
			exit 1
		fi
	fi
}

step_machete() {
	if ! command -v cargo-machete >/dev/null 2>&1; then
		fail_step machete "error: cargo-machete is required (cargo install cargo-machete)"
		return 1
	fi
	run_step machete cargo machete
}

step_typos() {
	if ! command -v typos >/dev/null 2>&1; then
		fail_step typos "error: typos is required (cargo install typos-cli)"
		return 1
	fi
	run_step typos typos
}

step_mutants() {
	if ! command -v cargo-mutants >/dev/null 2>&1; then
		fail_step mutants "error: cargo-mutants is required (cargo install cargo-mutants)"
		return 1
	fi
	run_step mutants cargo mutants --no-shuffle
	local last_idx=$((${#STEP_STATUS[@]} - 1))
	if [[ $last_idx -ge 0 && "${STEP_STATUS[$last_idx]}" == "FAIL" ]]; then
		printf 'note: missed mutants mean injected bugs survived the test suite — backlog signal for untested behavior, not necessarily a release blocker\n'
	fi
}

step_ui_typecheck() {
	preflight_npm || {
		fail_step ui-typecheck "error: npm is required for UI steps (crates/ui)"
		return 1
	}
	run_step ui-typecheck npm --prefix crates/ui run typecheck
}

step_ui_test() {
	preflight_npm || {
		fail_step ui-test "error: npm is required for UI steps (crates/ui)"
		return 1
	}
	run_step ui-test npm --prefix crates/ui run test
}

run_named_step() {
	local name="$1"
	case "$name" in
	fmt) run_step fmt cargo fmt --all --check ;;
	clippy)
		run_step clippy cargo clippy --workspace --all-targets --quiet --message-format=short -- \
			-D warnings -D clippy::pedantic -D clippy::nursery -D clippy::cargo
		;;
	doc) run_step doc env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --quiet ;;
	test) run_step test cargo test --workspace --quiet ;;
	public-api) run_step public-api "$ROOT/scripts/check-engine-public-api.sh" ;;
	machete) step_machete ;;
	typos) step_typos ;;
	ui-typecheck) step_ui_typecheck ;;
	ui-test) step_ui_test ;;
	deny) run_step deny cargo deny check ;;
	arch) run_step arch "$ROOT/scripts/check-architecture.sh" ;;
	mutants)
		if [[ "$DEEP" -ne 1 ]]; then
			fail_step mutants "error: mutants step requires --deep"
			return 1
		fi
		step_mutants
		;;
	*)
		echo "error: unknown step '$name'" >&2
		usage_steps
		return 1
		;;
	esac
}

print_summary() {
	echo
	echo '== VERIFY SUMMARY =='
	local i name status
	for i in "${!RAN_STEPS[@]}"; do
		name="${RAN_STEPS[$i]}"
		status="${STEP_STATUS[$i]}"
		if [[ "$status" == "FAIL" ]]; then
			printf 'FAIL %s   repro: %s\n' "$name" "$(step_repro "$name")"
		else
			printf 'PASS %s\n' "$name"
		fi
	done
	local total="${#RAN_STEPS[@]}"
	if [[ $FAILED_COUNT -eq 0 ]]; then
		printf 'RESULT: PASS (%s/%s steps passed)\n' "$total" "$total"
	else
		printf 'RESULT: FAIL (%s/%s steps failed)\n' "$FAILED_COUNT" "$total"
	fi
}

# --- parse args ---
while [[ $# -gt 0 ]]; do
	case "$1" in
	--deep)
		DEEP=1
		shift
		;;
	-h | --help)
		echo "Usage: $0 [--deep] [step ...]"
		echo
		echo "Runs verification steps; default runs all. Set VERIFY_FAIL_FAST=1 to stop on first failure."
		echo "Set VERIFY_MAX_LINES to control truncated failure output (default: 150)."
		usage_steps
		exit 0
		;;
	-*)
		echo "error: unknown option '$1'" >&2
		exit 1
		;;
	*)
		REQUESTED_STEPS+=("$1")
		shift
		;;
	esac
done

preflight_toolchain

STEPS_TO_RUN=()
if [[ ${#REQUESTED_STEPS[@]} -gt 0 ]]; then
	STEPS_TO_RUN=("${REQUESTED_STEPS[@]}")
else
	STEPS_TO_RUN=("${ALL_STEPS[@]}")
	if [[ "$DEEP" -eq 1 ]]; then
		STEPS_TO_RUN+=(mutants)
	fi
fi

for step in "${STEPS_TO_RUN[@]}"; do
	if ! is_valid_step "$step"; then
		echo "error: unknown step '$step'" >&2
		usage_steps
		exit 1
	fi
done

for step in "${STEPS_TO_RUN[@]}"; do
	run_named_step "$step"
done

print_summary
if [[ $FAILED_COUNT -gt 0 ]]; then
	exit 1
fi
