#!/usr/bin/env bash
# Install Linux system deps for Tauri/desktop builds (CI). No-op on macOS.
set -euo pipefail

if [[ "$(uname -s)" != "Linux" ]]; then
	exit 0
fi

sudo apt-get update
sudo apt-get install -y \
	libwebkit2gtk-4.1-dev \
	build-essential \
	curl \
	wget \
	file \
	libxdo-dev \
	libssl-dev \
	libayatana-appindicator3-dev \
	librsvg2-dev
