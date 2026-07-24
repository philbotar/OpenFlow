#!/usr/bin/env bash
# Source into an interactive shell so cargo picks up rust-lld + optional sccache:
#   source ./scripts/dev-env.sh
#
# Works when sourced from bash or zsh.
if [[ -n "${BASH_SOURCE[0]:-}" ]]; then
	_OPENFLOW_DEV_ENV="${BASH_SOURCE[0]}"
elif [[ -n "${ZSH_VERSION:-}" ]]; then
	# zsh: %x is this file when sourced
	# shellcheck disable=SC2296
	_OPENFLOW_DEV_ENV="${(%):-%x}"
else
	_OPENFLOW_DEV_ENV="$0"
fi
ROOT="$(cd "$(dirname "$_OPENFLOW_DEV_ENV")/.." && pwd)"
unset _OPENFLOW_DEV_ENV
# shellcheck source=verify/_lib.sh
. "$ROOT/scripts/verify/_lib.sh"
ensure_rust_host_bin_path
ensure_sccache_wrapper
