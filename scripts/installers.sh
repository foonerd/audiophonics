#!/bin/bash
# Run each plugin's install.sh and uninstall.sh for real, as root, in a
# throwaway Debian bookworm container laid out like a player: a fresh
# install, an install over the plugin's previous version, a second install,
# and an uninstall. What the scripts ask of systemd and udev is recorded
# rather than done; what they write is looked at.
#
#   scripts/installers.sh              every plugin that has a sandbox
#   scripts/installers.sh audiophonics_evosabre    the ones named
#
# A plugin's sandbox is plugins/<plugin>/test/sandbox.sh, which runs inside
# the container with the plugin's files at /src.
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$ROOT"

if ! command -v docker >/dev/null 2>&1; then
  echo "installers: docker not found; the installers have not been run" >&2
  exit 0
fi

if [ $# -gt 0 ]; then
  plugins=("$@")
else
  plugins=()
  for sandbox in plugins/*/test/sandbox.sh; do
    [ -f "$sandbox" ] && plugins+=("$(basename "$(dirname "$(dirname "$sandbox")")")")
  done
fi

for plugin in "${plugins[@]}"; do
  echo "installers: $plugin"
  docker run --rm -v "$ROOT/plugins/$plugin:/src:ro" debian:bookworm-slim sh /src/test/sandbox.sh
done
