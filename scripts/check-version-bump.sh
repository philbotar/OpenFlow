#!/usr/bin/env bash
# Bump only the crate(s) you changed; no cross-crate lockstep.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

BASE="${VERSION_CHECK_BASE:-}"
if [[ -z "$BASE" ]]; then
	BASE="$(git merge-base HEAD origin/main 2>/dev/null || git merge-base HEAD main)"
fi
HEAD="${VERSION_CHECK_HEAD:-HEAD}"

exempt() {
	case "$1" in
	docs/* | .github/* | examples/* | scripts/* | tools/*) return 0 ;;
	AGENTS.md | CHANGELOG.md | CONTRIBUTING.md | LICENSE | README.md | SECURITY.md | deny.toml | Cargo.toml)
		return 0
		;;
	esac
	return 1
}

crate_of() {
	case "$1" in
	crates/engine/*) echo engine ;;
	crates/providers/*) echo providers ;;
	crates/orchestration/*) echo orchestration ;;
	crates/desktop/*) echo desktop ;;
	crates/ui/*) echo ui ;;
	crates/workspace-checks/*) echo workspace-checks ;;
	esac
}

substantive() {
	case "$1" in
	crates/engine/* | crates/providers/* | crates/orchestration/* | crates/workspace-checks/*) return 0 ;;
	crates/desktop/*) [[ "$(basename "$1")" != package-lock.json ]] ;;
	crates/ui/*) [[ "$(basename "$1")" != package-lock.json ]] ;;
	*) return 1 ;;
	esac
}

manifest_for() {
	case "$1" in
	engine | providers | orchestration | workspace-checks) echo "crates/$1/Cargo.toml" ;;
	desktop) echo crates/desktop/Cargo.toml ;;
	ui) echo crates/ui/package.json ;;
	esac
}

cargo_version() {
	git show "$1:$2" | awk '
		/^\[package\]/ { p=1; next }
		p && /^\[/ { exit }
		p && /^version[[:space:]]*=/ {
			gsub(/.*version[[:space:]]*=[[:space:]]*"|".*/, "")
			print
			exit
		}'
}

json_version() {
	git show "$1:$2" | sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1
}

read_version() {
	local ref=$1 path=$2
	case "$path" in
	*.toml) cargo_version "$ref" "$path" ;;
	*.json) json_version "$ref" "$path" ;;
	esac
}

version_gt() {
	[[ "$1" != "$2" && "$(printf '%s\n' "$1" "$2" | sort -V | head -1)" == "$1" ]]
}

touched=
while IFS= read -r file; do
	[[ -n "$file" ]] || continue
	exempt "$file" && continue
	substantive "$file" || continue
	crate=$(crate_of "$file")
	[[ -n "$crate" ]] && touched="${touched}${crate}"$'\n'
done < <(git diff --name-only --diff-filter=ACMRT "$BASE..$HEAD")

touched=$(printf '%s' "$touched" | sort -u)
if [[ -z "$touched" ]]; then
	echo "Version bump check passed (no substantive crate changes vs ${BASE:0:12})."
	exit 0
fi

errors=0
printf 'base: %s\n\n' "$BASE"
while IFS= read -r crate; do
	[[ -n "$crate" ]] || continue
	path=$(manifest_for "$crate")
	old=$(read_version "$BASE" "$path")
	new=$(read_version "$HEAD" "$path")
	printf '  %s: %s -> %s\n' "$crate" "$old" "$new"
	version_gt "$old" "$new" || {
		echo "error: $crate changed but version is still $old" >&2
		errors=1
	}
done <<<"$touched"

if ((errors)); then
	echo "Bump patch (or minor/major) in that crate's manifest." >&2
	exit 1
fi

echo "Version bump check passed."
