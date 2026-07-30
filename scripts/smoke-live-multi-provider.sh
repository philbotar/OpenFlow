#!/usr/bin/env bash
# Manual live-AI smoke for node override -> shared workflow provider routing.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=verify/_lib.sh
. "$ROOT/scripts/verify/_lib.sh"

preflight_nextest
cd "$ROOT"

export OPENFLOW_LIVE_MULTI_PROVIDER_SMOKE=1
: "${OPENFLOW_LIVE_AI_SECONDARY_PROVIDER:?set OPENFLOW_LIVE_AI_SECONDARY_PROVIDER to a configured provider ID}"
export OPENFLOW_LIVE_AI_SECONDARY_PROVIDER
features=()
if [[ "$OPENFLOW_LIVE_AI_SECONDARY_PROVIDER" == "bedrock" ]]; then
	features=(--features bedrock)
fi
exec cargo nextest run \
	-p orchestration \
	"${features[@]}" \
	--test live_multi_provider_smoke \
	--run-ignored ignored-only \
	--no-capture
