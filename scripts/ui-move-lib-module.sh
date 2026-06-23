#!/usr/bin/env bash
# Move a flat lib module into lib/<name>/index.ts (+ optional test).
# Usage: ./scripts/ui-move-lib-module.sh <ModuleName>
set -euo pipefail

name="${1:?module name required}"
root="crates/ui/src/lib"
src="${root}/${name}.ts"
test_src="${root}/${name}.test.ts"
dir="${root}/${name}"

if [[ -e "${dir}" ]]; then
  echo "error: ${dir} already exists" >&2
  exit 1
fi

if [[ ! -f "${src}" ]]; then
  echo "error: ${src} not found" >&2
  exit 1
fi

mkdir -p "${dir}"
git mv "${src}" "${dir}/index.ts"

if [[ -f "${test_src}" ]]; then
  git mv "${test_src}" "${dir}/${name}.test.ts"
fi

echo "moved lib/${name} -> lib/${name}/"
