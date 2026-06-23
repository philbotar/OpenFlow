#!/usr/bin/env bash
# Move a flat UI module file into a folder with index.ts barrel.
# Usage: ./scripts/ui-move-module.sh <parent-dir> <ModuleName>
# Example: ./scripts/ui-move-module.sh crates/ui/src/components AppHeader
set -euo pipefail

parent="${1:?parent directory required}"
name="${2:?module name required}"

dir="${parent}/${name}"
src="${parent}/${name}.tsx"
test_src="${parent}/${name}.test.tsx"

if [[ -e "${dir}" ]]; then
  echo "error: ${dir} already exists" >&2
  exit 1
fi

if [[ ! -f "${src}" ]]; then
  echo "error: ${src} not found" >&2
  exit 1
fi

mkdir -p "${dir}"
git mv "${src}" "${dir}/${name}.tsx"

if [[ -f "${test_src}" ]]; then
  git mv "${test_src}" "${dir}/${name}.test.tsx"
fi

cat >"${dir}/index.ts" <<EOF
export * from "./${name}";
EOF

echo "moved ${name} -> ${dir}/"
