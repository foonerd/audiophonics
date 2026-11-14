#!/bin/sh
# The installer and the uninstaller, run as root in a container laid out
# like a player. See scripts/installers.sh.
set -eu

PLUGIN=/data/plugins/system_hardware/audiophonics_evo_sabre
CALLS=/tmp/calls
USERCONFIG=/boot/userconfig.txt

# What the scripts ask of the system is written down, not done.
mkdir -p /usr/local/sbin
for tool in systemctl udevadm visudo; do
  printf '#!/bin/sh\necho "%s $*" >> %s\n' "$tool" "$CALLS" > "/usr/local/sbin/$tool"
  chmod +x "/usr/local/sbin/$tool"
done
useradd -m volumio

fail() {
  echo "FAILED: $*" >&2
  echo "--- $USERCONFIG" >&2
  cat "$USERCONFIG" >&2 2>/dev/null || true
  echo "--- calls" >&2
  cat "$CALLS" >&2 2>/dev/null || true
  exit 1
}

# A player with the plugin's files unpacked and nothing installed yet.
player() {
  rm -rf /data "$CALLS" /etc/systemd/system/evo*.service \
    /etc/udev/rules.d/70-evosabre-remote.rules /etc/sudoers.d/volumio-user-audiophonics_evo_sabre
  rm -f /boot/config.txt /boot/volumioconfig.txt "$USERCONFIG"
  mkdir -p "$PLUGIN/bin" /boot /etc/systemd/system /etc/udev/rules.d /etc/sudoers.d
  cp -r /src/. "$PLUGIN/"
  printf '#!/bin/sh\necho "evosabre-panel 3.0.0"\n' > "$PLUGIN/bin/evosabre-panel"
  printf 'dtparam=audio=on\n' > /boot/config.txt
  : > "$CALLS"
}

installed() {
  [ -f /etc/systemd/system/evosabre-panel.service ] || fail "$1: the unit is not installed"
  [ -f /etc/udev/rules.d/70-evosabre-remote.rules ] || fail "$1: the udev rule is not installed"
  [ "$(stat -c %a /etc/sudoers.d/volumio-user-audiophonics_evo_sabre)" = 440 ] || fail "$1: the sudoers entry is not 0440"
  [ "$(stat -c %U "$PLUGIN/index.js")" = volumio ] || fail "$1: the plugin folder is not volumio's"
  grep -q "^systemctl daemon-reload" "$CALLS" || fail "$1: systemd was not asked to reload"
  grep -q "^udevadm trigger --action=change --subsystem-match=rc" "$CALLS" || fail "$1: udev was not asked for the decoder"
  grep -q "^visudo -c -f /etc/sudoers.d/volumio-user-audiophonics_evo_sabre" "$CALLS" || fail "$1: the sudoers entry was not checked"
}

# --- A fresh install: the two lines go into the plugin's own block. ----------
player
printf 'dtoverlay=hifiberry-dac\n# mine\n' > "$USERCONFIG"
out=$(sh "$PLUGIN/install.sh")
echo "$out" | grep -q "^plugininstallend$" || fail "fresh: the install did not finish: $out"
installed fresh
expected='dtoverlay=hifiberry-dac
# mine
# >>> audiophonics_evo_sabre
dtparam=spi=on
dtoverlay=gpio-ir,gpio_pin=4
# <<< audiophonics_evo_sabre'
[ "$(cat "$USERCONFIG")" = "$expected" ] || fail "fresh: the boot configuration is not as expected"
echo "$out" | grep -q "Restart the player once" || fail "fresh: no word about the restart"
echo "ok: a fresh install writes its block and asks for a restart"

# --- A second install changes nothing and asks for no restart. ---------------
: > "$CALLS"
out=$(sh "$PLUGIN/install.sh")
[ "$(cat "$USERCONFIG")" = "$expected" ] || fail "second: the boot configuration changed"
echo "$out" | grep -q "Restart the player once" && fail "second: asked for a restart with nothing changed"
installed second
echo "ok: a second install leaves the boot configuration alone"

# --- The uninstall takes back everything and leaves the rest. ----------------
: > "$CALLS"
out=$(sh "$PLUGIN/uninstall.sh")
echo "$out" | grep -q "^pluginuninstallend$" || fail "uninstall: did not finish: $out"
[ "$(cat "$USERCONFIG")" = 'dtoverlay=hifiberry-dac
# mine' ] || fail "uninstall: the boot configuration is not as it was"
for left in /etc/systemd/system/evosabre-panel.service /etc/udev/rules.d/70-evosabre-remote.rules /etc/sudoers.d/volumio-user-audiophonics_evo_sabre; do
  [ ! -e "$left" ] || fail "uninstall: $left is still there"
done
grep -q "^systemctl disable evosabre-panel.service" "$CALLS" || fail "uninstall: the service was not disabled"
echo "ok: the uninstall takes back the unit, the rule, the sudoers entry and the block"

# --- Over version 2: its lines stay, its services go, nothing is doubled. ----
player
printf 'dtparam=spi=on\ndtoverlay=gpio-ir,gpio_pin=4\n' > "$USERCONFIG"
mkdir -p "$PLUGIN/apps/evo_oled" "$PLUGIN/service"
for old in evo_oled2 evo_remote evo_irexec; do
  : > "$PLUGIN/service/$old.service"
  ln -s "$PLUGIN/service/$old.service" "/etc/systemd/system/$old.service"
done
out=$(sh "$PLUGIN/install.sh")
installed upgrade
[ "$(cat "$USERCONFIG")" = 'dtparam=spi=on
dtoverlay=gpio-ir,gpio_pin=4' ] || fail "upgrade: the boot configuration was touched"
echo "$out" | grep -q "Restart the player once" && fail "upgrade: asked for a restart with nothing changed"
for old in evo_oled2 evo_remote evo_irexec; do
  if [ -e "/etc/systemd/system/$old.service" ] || [ -L "/etc/systemd/system/$old.service" ]; then
    fail "upgrade: $old.service is still there"
  fi
  grep -q "^systemctl stop $old.service" "$CALLS" || fail "upgrade: $old was not stopped"
done
if [ -e "$PLUGIN/apps" ] || [ -e "$PLUGIN/service" ]; then
  fail "upgrade: version 2's folders are still there"
fi
echo "ok: an install over version 2 removes its services and keeps its boot lines"

# --- The receiver's overlay written another way is not written twice. --------
player
printf 'dtoverlay=gpio-ir,gpio_pin=4,rc-map-name=rc-empty\n' > "$USERCONFIG"
sh "$PLUGIN/install.sh" >/dev/null
[ "$(grep -c '^dtoverlay=gpio-ir' "$USERCONFIG")" = 1 ] || fail "variant: the receiver's overlay is there twice"
grep -q "^dtparam=spi=on$" "$USERCONFIG" || fail "variant: the SPI bus is not switched on"
echo "ok: a receiver overlay already there is not written a second time"

# --- A program that does not run is refused, and nothing is left. ------------
player
printf '#!/bin/sh\nexit 126\n' > "$PLUGIN/bin/evosabre-panel"
if out=$(sh "$PLUGIN/install.sh"); then fail "refusal: an install with a program that does not run went through"; fi
echo "$out" | grep -q "does not run on this machine" || fail "refusal: no word why: $out"
[ ! -e "$PLUGIN" ] || fail "refusal: the plugin folder was left behind"
[ ! -e /etc/systemd/system/evosabre-panel.service ] || fail "refusal: the unit was installed"
echo "ok: a program that does not run is refused before anything is installed"

echo "installers: every case passed"
