#!/bin/bash
# Assemble a plugin's zip as Volumio's plugin manager takes it: the plugin's
# files at the root, its program under bin/, and the components it ships
# from their own releases (scripts/fetch-components.sh). Output:
# dist/<store name>-<version>.zip.
#
# The zip carries no node modules: `volumio plugin install` installs a
# plugin's dependencies on the player, and the plugin store refuses a
# package that brings its own.
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
  COMPONENTS=
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
  rm -rf "$stage/test" "$stage/build.conf" "$stage/CHANGELOG.md" "$stage/node_modules" "$stage/package-lock.json"
  install -D -m 755 "$bin" "$stage/bin/$CRATE"

  # The components the plugin ships but this repository does not build,
  # each fetched from its own release and verified first.
  shipped=
  for component in $COMPONENTS; do
    scripts/fetch-components.sh "$component"
    NAME='' VERSION='' FILES=''
    . "components/$component.lock"
    for entry in $FILES; do
      from=${entry%%:*}
      rest=${entry#*:}
      install -D -m "${rest##*:}" "target/components/$component/files/$from" "$stage/${rest%%:*}"
    done
    shipped="$shipped${shipped:+, }\"$NAME\": \"$VERSION\""
  done

  # The plugin's version follows its program's.
  sed -i "s/\"version\": \"[^\"]*\"/\"version\": \"$version\"/" "$stage/package.json"

  # The build the zip carries: the commit, the time, and the components'
  # versions.
  printf '{ "commit": "%s", "built": "%s", "components": { %s } }\n' \
    "$(git rev-parse --short HEAD 2>/dev/null || echo unknown)" \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$shipped" > "$stage/build.json"

  zip="dist/$name-$version.zip"
  rm -f "$zip"
  ( cd "$stage" && zip -qr "$ROOT/$zip" . )
  rm -rf dist/stage
  if unzip -Z1 "$zip" | grep -q -e '^node_modules/' -e '/node_modules/' -e 'package-lock\.json$'; then
    echo "package: $zip carries node modules or a lock file; the store refuses it" >&2
    exit 1
  fi
  ls -l "$zip"
done
