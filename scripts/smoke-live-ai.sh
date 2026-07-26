#!/usr/bin/env bash
# Manual live-AI smoke using the provider selected in saved OpenFlow settings.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=verify/_lib.sh
. "$ROOT/scripts/verify/_lib.sh"

preflight_nextest
cd "$ROOT"

export OPENFLOW_LIVE_AI_SMOKE=1
exec cargo nextest run \
	-p orchestration \
	--test live_ai_smoke \
	--run-ignored ignored-only \
	--no-capture
