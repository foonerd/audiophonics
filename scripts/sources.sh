#!/bin/bash
# Print a digest of the sources the plugins' programs are built from: the
# workspace's manifests, its toolchain and linker settings, and every file
# under crates/ and bins/ that git tracks or would track.
#
# scripts/ship.sh keeps the digest beside each program it builds, and
# scripts/package.sh puts a program in a zip only where the sources are
# still the ones it was built from: a zip names the commit it was made at,
# and must not carry a program built from another.
set -euo pipefail
ROOT=$(CDPATH='' cd -- "$(dirname "$0")/.." && pwd)
cd "$ROOT"
export LC_ALL=C

paths=(Cargo.toml Cargo.lock rust-toolchain.toml .cargo crates bins)

# The files, each once: as git knows them, or as they lie where the tree is
# not a checkout.
list() {
  if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    git ls-files -z --cached --others --exclude-standard -- "${paths[@]}"
  else
    find "${paths[@]}" -type f -print0
  fi
}

list | sort -zu | while IFS= read -r -d '' file; do
  # A file git still lists and the tree no longer has is not a source.
  if [ -f "$file" ]; then
    sha256sum "$file"
  fi
done | sha256sum | cut -d' ' -f1
