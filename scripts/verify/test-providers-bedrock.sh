#!/usr/bin/env bash
# providers + AWS Bedrock feature — slower; CI / Bedrock work only.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=verify/_lib.sh
. "$ROOT/scripts/verify/_lib.sh"
preflight_toolchain
preflight_nextest
cd "$ROOT"
exec cargo nextest run -p providers --features bedrock --cargo-quiet
