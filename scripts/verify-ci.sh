#!/usr/bin/env bash
# Workspace lint gate run by CI's `lint` job. Per-crate clippy/test now run in
# parallel in the `check` matrix (see scripts/ci-crate-check.sh), so this covers
# only the workspace-global steps. Run full ./scripts/verify.sh locally before handoff.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec "$ROOT/scripts/verify.sh" fmt arch deny ui-typecheck ui-test
