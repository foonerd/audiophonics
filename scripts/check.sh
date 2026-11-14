#!/bin/bash
# The workshop pass CI runs: formatting, lints with warnings denied, tests,
# the documentation with warnings denied, every screen drawn, and each
# plugin's files and scripts. Run it before a commit.
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$ROOT"

echo "check: format"
cargo fmt --all -- --check

echo "check: clippy"
cargo clippy --workspace --all-targets --locked -- -D warnings

echo "check: tests"
cargo test --workspace --locked

echo "check: documentation"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked

echo "check: renders"
# Every screen a display draws, from the code that draws the glass.
for conf in plugins/*/build.conf; do
  plugin=$(basename "$(dirname "$conf")")
  CRATE=
  . "$conf"
  cargo run -q --locked -p "$CRATE" -- --render "target/renders/$plugin" --assets "plugins/$plugin/assets" >/dev/null
done

echo "check: shell scripts"
for script in scripts/*.sh; do
  bash -n "$script" || { echo "check: $script does not parse" >&2; exit 1; }
done
# Volumio runs a plugin's install.sh and uninstall.sh with sh, which is dash.
for script in plugins/*/install.sh plugins/*/uninstall.sh plugins/*/test/*.sh; do
  sh -n "$script" || { echo "check: $script does not parse" >&2; exit 1; }
done
if command -v shellcheck >/dev/null 2>&1; then
  # SC1007 is the `CDPATH= cd` idiom; SC1090 a plugin's build.conf, named at run time.
  shellcheck -e SC1007,SC1090 scripts/*.sh
  shellcheck -s dash plugins/*/install.sh plugins/*/uninstall.sh plugins/*/test/*.sh
fi
# A script committed without its executable bit fails on someone else's machine.
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  for script in scripts/*.sh plugins/*/install.sh plugins/*/uninstall.sh; do
    mode=$(git ls-files -s "$script" | cut -d' ' -f1)
    if [ -n "$mode" ] && [ "$mode" != 100755 ]; then
      echo "check: $script is not executable in git (mode $mode)" >&2
      exit 1
    fi
  done
fi

echo "check: installers"
# Each plugin's install.sh and uninstall.sh, run for real in a container.
scripts/installers.sh

echo "check: versions"
# A plugin carries the version of the program it ships.
for conf in plugins/*/build.conf; do
  plugin=$(basename "$(dirname "$conf")")
  CRATE=
  . "$conf"
  crate_version=$(sed -n 's/^version = "\(.*\)"/\1/p' "bins/$CRATE/Cargo.toml" | head -n1)
  plugin_version=$(sed -n 's/^ *"version": "\(.*\)",*$/\1/p' "plugins/$plugin/package.json" | head -n1)
  if [ -z "$crate_version" ] || [ "$crate_version" != "$plugin_version" ]; then
    echo "check: plugins/$plugin is $plugin_version but bins/$CRATE is $crate_version" >&2
    exit 1
  fi
  if ! grep -q "^## \[$crate_version\]" "plugins/$plugin/CHANGELOG.md"; then
    echo "check: plugins/$plugin/CHANGELOG.md has no section for $crate_version" >&2
    exit 1
  fi
  echo "$plugin $plugin_version"
done

# The languages a release to the store carries beside English: the set the
# house's other plugins ship.
STORE_LANGUAGES="de es fr it ja nl pl pt ru zh"

# Node runs the checks of the plugins' files: here where it is installed,
# else in the node image.
if command -v node >/dev/null 2>&1; then
  with_node() { node "$@"; }
elif command -v docker >/dev/null 2>&1; then
  with_node() { docker run -i --rm -v "$ROOT:/work" -w /work node:20-slim node "$@"; }
else
  echo "check: neither node nor docker found; the plugins' checks are skipped" >&2
  exit 0
fi

for plugin in plugins/*/; do
  plugin=${plugin%/}
  echo "check: $plugin files"
  # Volumio's core reads every plugin's strings at its start: one bad file
  # takes the whole backend down, so every JSON the plugin ships must parse,
  # and a key written twice in one object (the later silently wins) is refused.
  with_node - "$plugin"/package.json "$plugin"/config.json "$plugin"/UIConfig.json "$plugin"/i18n/*.json <<'JS'
const fs = require('fs');
function keysWrittenTwice(text) {
  const twice = new Set();
  const stack = [];
  let i = 0;
  let start = 0;
  const skipString = () => { i++; while (i < text.length && text[i] !== '"') { if (text[i] === '\\') i++; i++; } return text.slice(start + 1, i++); };
  while (i < text.length) {
    const c = text[i];
    if (c === '{') { stack.push({ keys: new Set(), expectKey: true }); i++; }
    else if (c === '}') { stack.pop(); i++; }
    else if (c === '[') { stack.push(null); i++; }
    else if (c === ']') { stack.pop(); i++; }
    else if (c === '"') {
      start = i;
      const s = skipString();
      const top = stack[stack.length - 1];
      if (top && top.expectKey) { if (top.keys.has(s)) twice.add(s); top.keys.add(s); top.expectKey = false; }
    }
    else if (c === ',') { const top = stack[stack.length - 1]; if (top) top.expectKey = true; i++; }
    else i++;
  }
  return [...twice].sort();
}
const files = process.argv.slice(2);
for (const file of files) {
  const text = fs.readFileSync(file, 'utf8');
  try { JSON.parse(text); } catch (e) { console.error(file + ': ' + e.message); process.exit(1); }
  const twice = keysWrittenTwice(text);
  if (twice.length) { console.error(file + ': written twice in one object: ' + twice.join(', ')); process.exit(1); }
  if (!text.endsWith('\n') || text.replace(/\n+$/, '') !== text.trimEnd()) { console.error(file + ': must end with one newline'); process.exit(1); }
}
console.log(files.length + ' plugin files parse');
JS

  echo "check: $plugin UI strings"
  # Every TRANSLATE key the settings page names must be in strings_en.json.
  with_node - "$plugin"/UIConfig.json "$plugin"/i18n/strings_en.json <<'JS'
const fs = require('fs');
const ui = fs.readFileSync(process.argv[2], 'utf8');
const strings = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
const missing = [...new Set([...ui.matchAll(/TRANSLATE\.([A-Z0-9_.]+)/g)].map((m) => m[1]))]
  .filter((key) => key.split('.').reduce((o, k) => (o && typeof o === 'object' ? o[k] : undefined), strings) === undefined);
if (missing.length) { console.error('strings missing: ' + missing.join(', ')); process.exit(1); }
console.log('every UI string is defined');
JS

  echo "check: $plugin languages"
  # A language file is all or nothing: Volumio takes the plugin's block for
  # its toasts and modals from the one file of the player's language, with no
  # fall back to the English file, so a missing key would show as its name.
  # Each file must carry every English key, with the {{placeholders}} the
  # English has.
  with_node - "$plugin"/i18n/strings_en.json "$plugin"/i18n/strings_*.json <<'JS'
const fs = require('fs');
const [enFile, ...files] = process.argv.slice(2);
const en = JSON.parse(fs.readFileSync(enFile, 'utf8'));
const [block] = Object.keys(en);
let bad = 0;
for (const file of files.filter((f) => f !== enFile)) {
  const lang = JSON.parse(fs.readFileSync(file, 'utf8'));
  const problems = [];
  if (Object.keys(lang).join() !== block) problems.push('must have the one block ' + block);
  const strings = lang[block] || {};
  for (const [key, text] of Object.entries(en[block])) {
    const t = strings[key];
    if (typeof t !== 'string' || !t.trim()) { problems.push(key + ' missing'); continue; }
    for (const p of text.match(/\{\{[a-z]+\}\}/g) || []) if (!t.includes(p)) problems.push(key + ' lacks ' + p);
  }
  for (const key of Object.keys(strings)) if (!(key in en[block])) problems.push(key + ' is not an English key');
  if (problems.length) { bad++; console.error(file + ':\n  ' + problems.join('\n  ')); }
}
if (bad) process.exit(1);
console.log((files.length - 1) + ' language files complete');
JS

  # A build for the bench (x.99.z) may carry fewer languages than a release
  # to the store, which carries the set the house's other plugins do.
  version=$(sed -n 's/^ *"version": "\(.*\)",*$/\1/p' "$plugin/package.json" | head -n1)
  case "$version" in
    *.99.*) ;;
    *)
      for lang in $STORE_LANGUAGES; do
        if [ ! -f "$plugin/i18n/strings_$lang.json" ]; then
          echo "check: $plugin $version is a release to the store and has no strings_$lang.json (it carries: $STORE_LANGUAGES)" >&2
          exit 1
        fi
      done
      ;;
  esac

  echo "check: $plugin code"
  for script in "$plugin"/index.js "$plugin"/lib/*.js; do
    with_node --check "$script"
  done
  with_node --test "$plugin"/test/
done

echo "check: clean"
