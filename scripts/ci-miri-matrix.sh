#!/usr/bin/env bash
# Emit the CI Miri matrix for engine/orchestration when those crates changed.
#
# Local: VERSION_CHECK_BASE=main ./scripts/ci-miri-matrix.sh
#        CI_CHANGED_FILES=$'crates/engine/src/lib.rs\n' ./scripts/ci-miri-matrix.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

BASE="${VERSION_CHECK_BASE:-}"
if [[ -z "$BASE" ]]; then
	BASE="$(git merge-base HEAD origin/main 2>/dev/null || git merge-base HEAD main)"
fi
HEAD="${VERSION_CHECK_HEAD:-HEAD}"

changed=""
add() { case " $changed " in *" $1 "*) ;; *) changed="$changed $1" ;; esac; }
has() { [[ " $changed " == *" $1 "* ]]; }

force_all=0
while IFS= read -r file; do
	[[ -n "$file" ]] || continue
	case "$file" in
	Cargo.lock | Cargo.toml | rust-toolchain.toml) force_all=1 ;;
	.github/workflows/ci.yml) force_all=1 ;;
	scripts/miri.sh | scripts/ci-miri-matrix.sh) force_all=1 ;;
	crates/engine/*) add engine ;;
	crates/orchestration/*) add orchestration ;;
	esac
done < <(if [[ -n "${CI_CHANGED_FILES:-}" ]]; then printf '%s\n' "$CI_CHANGED_FILES"; else git diff --name-only --diff-filter=ACDMRT "$BASE..$HEAD"; fi)

if ((force_all)); then
	changed="engine orchestration"
fi

crates=()
has engine && crates+=('"engine"')
has orchestration && crates+=('"orchestration"')

if ((${#crates[@]})); then
	matrix="[$(
		IFS=,
		echo "${crates[*]}"
	)]"
else
	matrix="[]"
fi

echo "matrix=$matrix"
if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
	echo "matrix=$matrix" >>"$GITHUB_OUTPUT"
fi
