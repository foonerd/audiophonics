#!/bin/bash
# Build each plugin's program as the players run it: one static musl binary
# for the 32-bit ARM userland Volumio runs on every Raspberry Pi, whatever
# the kernel beneath it. Output: target/ship/<plugin>/bin/<program>.
# Players run these files; they do not run cargo. Beside each program goes a
# digest of the sources it was built from (scripts/sources.sh).
#
#   scripts/ship.sh              every plugin
#   scripts/ship.sh audiophonics_evosabre    the ones named
#
# Needs the cross linker of .cargo/config.toml (gcc-arm-linux-gnueabihf) and
# the pinned toolchain. With qemu-arm at hand, each binary is also run: it
# must say its version, and where it draws screens it must draw the same
# pixels as this machine's own build.
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$ROOT"
unset CARGO_TARGET_DIR

TRIPLE=armv7-unknown-linux-musleabihf

# Source paths in panic messages are remapped to fixed names, so a binary
# carries neither this machine's paths nor its user's home.
cargo_home=${CARGO_HOME:-$HOME/.cargo}
rustup_home=${RUSTUP_HOME:-$HOME/.rustup}
export RUSTFLAGS="--remap-path-prefix=$ROOT=/audiophonics --remap-path-prefix=$cargo_home=/cargo --remap-path-prefix=$rustup_home=/rustup"

if [ $# -gt 0 ]; then
  plugins=("$@")
else
  plugins=()
  for conf in plugins/*/build.conf; do
    plugins+=("$(basename "$(dirname "$conf")")")
  done
fi

qemu=$(command -v qemu-arm || command -v qemu-arm-static || true)

for plugin in "${plugins[@]}"; do
  [ -f "plugins/$plugin/build.conf" ] || { echo "ship: no plugin $plugin under plugins/" >&2; exit 1; }
  CRATE=
  . "plugins/$plugin/build.conf"
  echo "ship: $plugin ($CRATE, $TRIPLE)"
  # The sources as they are when the build starts: scripts/package.sh takes
  # the program only for as long as they stay so.
  sources=$(scripts/sources.sh)
  rm -f "target/ship/$plugin/sources"
  cargo build --release --locked --target "$TRIPLE" -p "$CRATE"
  bin="target/ship/$plugin/bin/$CRATE"
  install -D -m 755 "target/$TRIPLE/release/$CRATE" "$bin"
  echo "$sources" > "target/ship/$plugin/sources"

  # Static, or the player's loader has a say in whether it starts.
  if readelf -lW "$bin" | grep -q INTERP || readelf -dW "$bin" 2>/dev/null | grep -q NEEDED; then
    echo "ship: $bin is not statically linked" >&2
    readelf -dW "$bin" | grep NEEDED >&2 || true
    exit 1
  fi
  if strings -a "$bin" | grep -qF -e "$ROOT" -e "$HOME/"; then
    echo "ship: $bin carries this machine's paths" >&2
    exit 1
  fi

  if [ -n "$qemu" ]; then
    said=$("$qemu" "$bin" --version)
    wanted="$CRATE $(sed -n 's/^version = "\(.*\)"/\1/p' "bins/$CRATE/Cargo.toml" | head -n1)"
    [ "$said" = "$wanted" ] || { echo "ship: $bin says '$said', expected '$wanted'" >&2; exit 1; }
    help=$("$qemu" "$bin" --help)
    if [[ $help == *--render* ]]; then
      rm -rf "target/ship/$plugin/renders" "target/ship/$plugin/renders-host"
      "$qemu" "$bin" --render "target/ship/$plugin/renders" --assets "plugins/$plugin/assets" >/dev/null
      RUSTFLAGS='' cargo run -q --locked -p "$CRATE" -- --render "target/ship/$plugin/renders-host" --assets "plugins/$plugin/assets" >/dev/null
      diff -r "target/ship/$plugin/renders" "target/ship/$plugin/renders-host" >/dev/null \
        || { echo "ship: $bin draws differently from this machine's build" >&2; exit 1; }
      echo "ship: $said, and it draws $(find "target/ship/$plugin/renders" -type f | wc -l) screens as this machine does"
    else
      echo "ship: $said"
    fi
  else
    echo "ship: qemu-arm not found; $bin is built and static, but has not been run"
  fi
  file "$bin"
  ls -l "$bin"
done
