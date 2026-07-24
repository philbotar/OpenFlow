#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=verify/_lib.sh
. "$ROOT/scripts/verify/_lib.sh"

TEST_TMP="$(mktemp -d)"
trap 'rm -rf "$TEST_TMP"' EXIT

fail() {
	echo "FAIL: $*" >&2
	exit 1
}

python3 - "$ROOT/.cargo/config.toml" <<'PY' \
	|| fail "Cargo incremental builds are not disabled"
import pathlib
import sys
import tomllib

config = tomllib.loads(pathlib.Path(sys.argv[1]).read_text())
if config.get("build", {}).get("incremental") is not False:
    raise SystemExit("build.incremental must be false")
PY

OPENFLOW_MIN_BUILD_SPACE_GIB=24
OPENFLOW_MAX_DEBUG_CACHE_GIB=64
OPENFLOW_DEBUG_CACHE_SIZE_KIB=$((30 * 1024 * 1024))
OPENFLOW_BUILD_SPACE_AVAILABLE_KIB=$((30 * 1024 * 1024))
preflight_build_space || fail "guard rejected sufficient free space"

OPENFLOW_DEBUG_CACHE_SIZE_KIB=$((70 * 1024 * 1024))
if preflight_build_space 2>"$TEST_TMP/large-cache.log"; then
	fail "guard allowed an oversized debug cache"
fi
grep -q "70 GiB debug cache; 64 GiB maximum" "$TEST_TMP/large-cache.log" \
	|| fail "large-cache error lacks measured size and ceiling"
grep -q "$ROOT/target/debug" "$TEST_TMP/large-cache.log" \
	|| fail "large-cache error lacks exact target path"
grep -q "./scripts/clean-rust-cache.sh --yes" "$TEST_TMP/large-cache.log" \
	|| fail "large-cache error lacks cleanup command"

OPENFLOW_DEBUG_CACHE_SIZE_KIB=$((30 * 1024 * 1024))
OPENFLOW_BUILD_SPACE_AVAILABLE_KIB=$((5 * 1024 * 1024))
if preflight_build_space 2>"$TEST_TMP/low-space.log"; then
	fail "guard allowed insufficient free space"
fi
grep -q "refusing to start Cargo" "$TEST_TMP/low-space.log" \
	|| fail "low-space error lacks refusal reason"
grep -q "$ROOT/target/debug" "$TEST_TMP/low-space.log" \
	|| fail "low-space error lacks exact target path"
grep -q "./scripts/clean-rust-cache.sh --yes" "$TEST_TMP/low-space.log" \
	|| fail "low-space error lacks cleanup command"

OPENFLOW_MIN_BUILD_SPACE_GIB=0
preflight_build_space || fail "zero threshold did not disable guard"

OPENFLOW_MIN_BUILD_SPACE_GIB=invalid
if preflight_build_space 2>"$TEST_TMP/invalid-threshold.log"; then
	fail "guard allowed an invalid threshold"
fi
grep -q "must be a non-negative integer" "$TEST_TMP/invalid-threshold.log" \
	|| fail "invalid threshold error lacks correction"

OPENFLOW_MIN_BUILD_SPACE_GIB=0
OPENFLOW_MAX_DEBUG_CACHE_GIB=invalid
if preflight_build_space 2>"$TEST_TMP/invalid-ceiling.log"; then
	fail "guard allowed an invalid debug cache ceiling"
fi
grep -q "OPENFLOW_MAX_DEBUG_CACHE_GIB must be a non-negative integer" \
	"$TEST_TMP/invalid-ceiling.log" \
	|| fail "invalid debug cache ceiling error lacks correction"

echo "Build-space guard tests passed."
