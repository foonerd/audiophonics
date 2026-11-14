#!/bin/bash
# Assemble a plugin's zip as Volumio's plugin manager takes it: the plugin's
# files at the root, its program under bin/, its node modules. Output:
# dist/<store name>-<version>.zip.
#
#   scripts/package.sh              every plugin
#   scripts/package.sh evo_sabre    the ones named
#
# Run scripts/ship.sh first: it builds the program this puts in the zip.
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$ROOT"

if [ $# -gt 0 ]; then
  plugins=("$@")
else
  plugins=()
  for conf in plugins/*/build.conf; do
    plugins+=("$(basename "$(dirname "$conf")")")
  done
fi

mkdir -p dist
for plugin in "${plugins[@]}"; do
  [ -f "plugins/$plugin/build.conf" ] || { echo "package: no plugin $plugin under plugins/" >&2; exit 1; }
  CRATE=
  . "plugins/$plugin/build.conf"
  version=$(sed -n 's/^version = "\(.*\)"/\1/p' "bins/$CRATE/Cargo.toml" | head -n1)
  name=$(sed -n 's/^ *"name": "\(.*\)",*$/\1/p' "plugins/$plugin/package.json" | head -n1)
  bin="target/ship/$plugin/bin/$CRATE"
  [ -x "$bin" ] || { echo "package: $bin is not built; run scripts/ship.sh $plugin first" >&2; exit 1; }

  stage="dist/stage/$name"
  rm -rf dist/stage
  mkdir -p "$stage"
  cp -r "plugins/$plugin/." "$stage/"
  # What the repository needs and the player does not.
  rm -rf "$stage/test" "$stage/build.conf" "$stage/CHANGELOG.md" "$stage/node_modules"
  install -D -m 755 "$bin" "$stage/bin/$CRATE"

  # The plugin's version follows its program's.
  sed -i "s/\"version\": \"[^\"]*\"/\"version\": \"$version\"/" "$stage/package.json"

  # The build the zip carries: the commit and the time.
  printf '{ "commit": "%s", "built": "%s" }\n' \
    "$(git rev-parse --short HEAD 2>/dev/null || echo unknown)" \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$stage/build.json"

  # Node modules: with npm on this machine, directly; otherwise in a container.
  if command -v npm >/dev/null 2>&1; then
    ( cd "$stage" && npm install --omit=dev --no-audit --no-fund --loglevel=error >/dev/null )
  else
    docker run --rm -v "$ROOT/$stage:/plugin" -w /plugin node:20-slim sh -c "npm install --omit=dev --no-audit --no-fund --loglevel=error >/dev/null && chown -R $(id -u):$(id -g) /plugin/node_modules /plugin/package-lock.json 2>/dev/null || true"
  fi
  rm -f "$stage/package-lock.json"

  zip="dist/$name-$version.zip"
  rm -f "$zip"
  ( cd "$stage" && zip -qr "$ROOT/$zip" . )
  rm -rf dist/stage
  ls -l "$zip"
done
