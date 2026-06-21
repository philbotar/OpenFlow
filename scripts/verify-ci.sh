#!/usr/bin/env bash
# Lean CI gate — faster than full ./scripts/verify.sh. Run verify.sh locally before handoff.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec "$ROOT/scripts/verify.sh" fmt clippy test-fast arch ui-test deny
