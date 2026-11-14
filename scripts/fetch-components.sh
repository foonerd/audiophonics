#!/bin/bash
# The components a plugin ships but this repository does not build, each from
# its own GitHub release at the version its lock file names
# (components/<name>.lock). Nothing is unpacked before it is verified: the
# release's SHA256SUMS against its Ed25519 signature with the evo release key
# (keys/release-signing.pub), then the archive against the sums; and once
# unpacked, the archive's own MANIFEST against its signature and its files.
# Unpacked under target/components/<name>/files/, where package.sh looks.
#
#   scripts/fetch-components.sh              every component
#   scripts/fetch-components.sh keepalive    the ones named
#
# Needs curl, openssl and unzip.
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$ROOT"
PUB="$ROOT/keys/release-signing.pub"

# One download: tried again when the connection fails or stalls, and under
# its name only once it is whole.
fetch() {
  local url=$1 file=$2
  curl -fL --no-progress-meter --retry 5 --retry-delay 3 --retry-all-errors \
    --connect-timeout 20 --speed-limit 1024 --speed-time 30 -o "$file.part" "$url"
  mv "$file.part" "$file"
}

# A file against its Ed25519 signature.
signed() {
  openssl pkeyutl -verify -pubin -inkey "$PUB" -rawin -in "$1" -sigfile "$2" >/dev/null 2>&1
}

if [ $# -gt 0 ]; then
  components=("$@")
else
  components=()
  for lock in components/*.lock; do
    components+=("$(basename "$lock" .lock)")
  done
fi

for component in "${components[@]}"; do
  lock="components/$component.lock"
  [ -f "$lock" ] || { echo "fetch-components: no $lock" >&2; exit 1; }
  NAME='' VERSION='' REPOSITORY=''
  . "$lock"
  dir="target/components/$component"
  if [ -f "$dir/verified-$VERSION" ]; then
    echo "fetch-components: $component $VERSION is here"
    continue
  fi

  rm -rf "$dir"
  mkdir -p "$dir/download" "$dir/files"
  archive="$NAME-$VERSION.zip"
  base="https://github.com/$REPOSITORY/releases/download/v$VERSION"
  for asset in "$archive" SHA256SUMS SHA256SUMS.sig; do
    fetch "$base/$asset" "$dir/download/$asset"
  done

  signed "$dir/download/SHA256SUMS" "$dir/download/SHA256SUMS.sig" \
    || { echo "fetch-components: $component $VERSION: SHA256SUMS is not signed with the release key" >&2; exit 1; }
  ( cd "$dir/download" && sha256sum --check --quiet --ignore-missing SHA256SUMS ) \
    || { echo "fetch-components: $component $VERSION: $archive is not the file the release names" >&2; exit 1; }
  grep -q " [.*/]*$archive\$" "$dir/download/SHA256SUMS" \
    || { echo "fetch-components: $component $VERSION: SHA256SUMS does not name $archive" >&2; exit 1; }

  unzip -q "$dir/download/$archive" -d "$dir/files"
  signed "$dir/files/MANIFEST" "$dir/files/MANIFEST.sig" \
    || { echo "fetch-components: $component $VERSION: the archive's MANIFEST is not signed with the release key" >&2; exit 1; }
  ( cd "$dir/files" && sha256sum --check --quiet MANIFEST ) \
    || { echo "fetch-components: $component $VERSION: a file in the archive is not the one its MANIFEST names" >&2; exit 1; }

  : > "$dir/verified-$VERSION"
  echo "fetch-components: $component $VERSION fetched and verified"
done
