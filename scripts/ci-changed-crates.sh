#!/usr/bin/env bash
# Emit the CI test matrix for only the crates affected by this change.
#
# Maps changed files -> owning crate, expands downstream along the dependency
# DAG (engine -> providers -> orchestration -> desktop), and forces every crate
# when a shared/root file changes. Prints `matrix=<json>` and, in CI, appends it
# to $GITHUB_OUTPUT for the `check` job's strategy.matrix.include.
#
# Local use: VERSION_CHECK_BASE=main ./scripts/ci-changed-crates.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Same base resolution as scripts/check-version-bump.sh.
BASE="${VERSION_CHECK_BASE:-}"
if [[ -z "$BASE" ]]; then
	BASE="$(git merge-base HEAD origin/main 2>/dev/null || git merge-base HEAD main)"
fi
HEAD="${VERSION_CHECK_HEAD:-HEAD}"

# bash 3.2 (macOS) compatible set: space-delimited membership.
changed=""
add() { case " $changed " in *" $1 "*) ;; *) changed="$changed $1" ;; esac; }
has() { [[ " $changed " == *" $1 "* ]]; }

force_all=0
while IFS= read -r file; do
	[[ -n "$file" ]] || continue
	case "$file" in
	Cargo.lock | Cargo.toml | rust-toolchain.toml | deny.toml) force_all=1 ;;
	.github/workflows/ci.yml) force_all=1 ;;
	scripts/ci-*.sh | scripts/verify*.sh) force_all=1 ;;
	crates/engine/*) add engine ;;
	crates/providers/*) add providers ;;
	crates/orchestration/*) add orchestration ;;
	crates/desktop/*) add desktop ;;
	esac
	# CI_CHANGED_FILES (newline-separated) overrides git for local testing.
done < <(if [[ -n "${CI_CHANGED_FILES:-}" ]]; then printf '%s\n' "$CI_CHANGED_FILES"; else git diff --name-only --diff-filter=ACDMRT "$BASE..$HEAD"; fi)

if ((force_all)); then
	changed="engine providers orchestration desktop"
fi

# Downstream transitive expansion along engine -> providers -> orchestration -> desktop.
has engine && {
	add providers
	add orchestration
	add desktop
}
has providers && {
	add orchestration
	add desktop
}
has orchestration && add desktop

legs=()
has engine && legs+=('{"name":"engine","node":false,"tauri":false}')
has providers && legs+=('{"name":"providers","node":false,"tauri":false}')
has orchestration && {
	legs+=('{"name":"orchestration-lib","node":false,"tauri":false}')
	legs+=('{"name":"orchestration-integration","node":false,"tauri":false}')
}
has desktop && legs+=('{"name":"desktop","node":true,"tauri":true}')

if ((${#legs[@]})); then
	matrix="[$(
		IFS=,
		echo "${legs[*]}"
	)]"
else
	matrix="[]"
fi

echo "matrix=$matrix"
if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
	echo "matrix=$matrix" >>"$GITHUB_OUTPUT"
fi
