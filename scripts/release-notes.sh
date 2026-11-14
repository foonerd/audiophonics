#!/bin/bash
# Print the changelog section of one version of one plugin, for its release
# notes: scripts/release-notes.sh evo_sabre 2.99.0
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
PLUGIN=${1:?a plugin, a folder under plugins/}
VERSION=${2:?a version, such as 2.99.0}
awk -v v="$VERSION" '
  /^## \[/ { if (found) exit; if (index($0, "## [" v "]") == 1) { found = 1; next } }
  found { print }
' "$ROOT/plugins/$PLUGIN/CHANGELOG.md" | sed -e '1{/^$/d}' -e '${/^$/d}'
