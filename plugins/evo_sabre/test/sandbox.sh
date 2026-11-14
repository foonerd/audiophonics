#!/bin/sh
# The installer and the uninstaller, run as root in a container laid out
# like a player. See scripts/installers.sh.
set -eu

PLUGIN=/data/plugins/system_hardware/audiophonics_evo_sabre
SETTINGS=/data/configuration/system_hardware/audiophonics_evo_sabre/config.json
PLUGINS=/data/configuration/plugins.json
CALLS=/tmp/calls
ACTIVE=/tmp/active
USERCONFIG=/boot/userconfig.txt
STARTED="systemctl enable --now evosabre-panel.service"

# What the scripts ask of the system is written down, not done. A unit is
# active when the case says so.
mkdir -p /usr/local/sbin
for tool in udevadm visudo; do
  printf '#!/bin/sh\necho "%s $*" >> %s\n' "$tool" "$CALLS" > "/usr/local/sbin/$tool"
  chmod +x "/usr/local/sbin/$tool"
done
cat > /usr/local/sbin/systemctl << STUB
#!/bin/sh
echo "systemctl \$*" >> $CALLS
if [ "\$1" = is-active ]; then
  for unit in "\$@"; do
    [ -e "$ACTIVE/\$unit" ] && exit 0
  done
  exit 3
fi
exit 0
STUB
chmod +x /usr/local/sbin/systemctl
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
  rm -rf /data "$CALLS" "$ACTIVE" /etc/systemd/system/evo*.service \
    /etc/udev/rules.d/70-evosabre-remote.rules /etc/sudoers.d/volumio-user-audiophonics_evo_sabre
  rm -f /boot/config.txt /boot/volumioconfig.txt "$USERCONFIG"
  mkdir -p "$PLUGIN/bin" "$ACTIVE" /boot /etc/systemd/system /etc/udev/rules.d /etc/sudoers.d
  cp -r /src/. "$PLUGIN/"
  printf '#!/bin/sh\necho "evosabre-panel 2.99.0"\n' > "$PLUGIN/bin/evosabre-panel"
  printf 'dtparam=audio=on\n' > /boot/config.txt
  : > "$CALLS"
}

# Volumio's record of the plugin, as it writes it, and the plugin's settings.
enabled_plugin() {
  mkdir -p "$(dirname "$SETTINGS")"
  cat > "$PLUGINS" << 'JSON'
{
  "system_hardware": {
    "audiophonics_evo_sabre": {
      "enabled": {
        "type": "boolean",
        "value": true
      },
      "status": {
        "type": "string",
        "value": "STARTED"
      }
    }
  }
}
JSON
  printf '{"oled_active":{"type":"boolean","value":%s},"contrast":{"type":"number","value":100}}' "$1" > "$SETTINGS"
}

installed() {
  [ -f /etc/systemd/system/evosabre-panel.service ] || fail "$1: the unit is not installed"
  [ -f /etc/udev/rules.d/70-evosabre-remote.rules ] || fail "$1: the udev rule is not installed"
  [ "$(stat -c %a /etc/sudoers.d/volumio-user-audiophonics_evo_sabre)" = 440 ] || fail "$1: the sudoers entry is not 0440"
  [ "$(stat -c %U "$PLUGIN/index.js")" = volumio ] || fail "$1: the plugin folder is not volumio's"
  grep -q "^systemctl daemon-reload" "$CALLS" || fail "$1: systemd was not asked to reload"
  grep -q "^udevadm trigger --action=change --subsystem-match=rc --subsystem-match=lirc" "$CALLS" || fail "$1: udev was not asked for the decoder and the device"
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
grep -q "^$STARTED" "$CALLS" && fail "fresh: the display was started for a plugin nobody has enabled"
echo "ok: a fresh install writes its block, asks for a restart and starts nothing"

# --- A second install changes nothing and asks for no restart. ---------------
: > "$CALLS"
out=$(sh "$PLUGIN/install.sh")
[ "$(cat "$USERCONFIG")" = "$expected" ] || fail "second: the boot configuration changed"
echo "$out" | grep -q "Restart the player" && fail "second: asked for a restart with nothing changed"
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
echo "ok: the uninstall takes back the unit, the rules, the sudoers entry and the block"

# --- A last line with no end is not joined to the block's mark. --------------
player
printf 'dtoverlay=hifiberry-dac' > "$USERCONFIG"
sh "$PLUGIN/install.sh" > /dev/null
[ "$(cat "$USERCONFIG")" = 'dtoverlay=hifiberry-dac
# >>> audiophonics_evo_sabre
dtparam=spi=on
dtoverlay=gpio-ir,gpio_pin=4
# <<< audiophonics_evo_sabre' ] || fail "no end: the boot configuration is not as expected"
sh "$PLUGIN/uninstall.sh" > /dev/null
[ "$(cat "$USERCONFIG")" = 'dtoverlay=hifiberry-dac' ] || fail "no end: the uninstall did not leave the line as it found it"
echo "ok: a last line with no end keeps to itself"

# --- Over version 2 in use: its lines stay, its services go, nothing is
# --- doubled, and the display is started rather than left dark. --------------
player
printf 'dtparam=spi=on\ndtoverlay=gpio-ir,gpio_pin=4\n' > "$USERCONFIG"
mkdir -p "$PLUGIN/apps/evo_oled" "$PLUGIN/service"
for old in evo_oled2 evo_remote evo_irexec; do
  : > "$PLUGIN/service/$old.service"
  ln -s "$PLUGIN/service/$old.service" "/etc/systemd/system/$old.service"
done
: > "$ACTIVE/evo_oled2.service"
out=$(sh "$PLUGIN/install.sh")
installed upgrade
[ "$(cat "$USERCONFIG")" = 'dtparam=spi=on
dtoverlay=gpio-ir,gpio_pin=4' ] || fail "upgrade: the boot configuration was touched"
for old in evo_oled2 evo_remote evo_irexec; do
  if [ -e "/etc/systemd/system/$old.service" ] || [ -L "/etc/systemd/system/$old.service" ]; then
    fail "upgrade: $old.service is still there"
  fi
  grep -q "^systemctl stop $old.service" "$CALLS" || fail "upgrade: $old was not stopped"
done
if [ -e "$PLUGIN/apps" ] || [ -e "$PLUGIN/service" ]; then
  fail "upgrade: version 2's folders are still there"
fi
grep -q "^$STARTED" "$CALLS" || fail "upgrade: the display was left dark"
echo "$out" | grep -q "Restart the player to finish the update" || fail "upgrade: no word about finishing the update: $out"
echo "$out" | grep -q "Restart the player once" && fail "upgrade: asked for a boot-configuration restart with nothing changed"
echo "ok: an install over version 2 in use removes its services, keeps its boot lines and starts the display"

# --- Over this plugin, enabled: the display is started again. ----------------
player
enabled_plugin true
printf 'dtparam=spi=on\ndtoverlay=gpio-ir,gpio_pin=4\n' > "$USERCONFIG"
out=$(sh "$PLUGIN/install.sh")
grep -q "^$STARTED" "$CALLS" || fail "update: the display was left dark"
echo "$out" | grep -q "Restart the player to finish the update" || fail "update: no word about finishing the update"
echo "ok: an update of the plugin in use starts the display again"

# --- Enabled, with the display switched off in its settings: left off. -------
player
enabled_plugin false
printf 'dtparam=spi=on\ndtoverlay=gpio-ir,gpio_pin=4\n' > "$USERCONFIG"
out=$(sh "$PLUGIN/install.sh")
grep -q "^$STARTED" "$CALLS" && fail "display off: the display was started against its setting"
echo "$out" | grep -q "Restart the player to finish the update" || fail "display off: no word about finishing the update"
echo "ok: a display switched off in the settings stays off through an update"

# --- The receiver's overlay written another way is not written twice. --------
player
printf 'dtoverlay=gpio-ir,gpio_pin=4,rc-map-name=rc-empty\n' > "$USERCONFIG"
sh "$PLUGIN/install.sh" > /dev/null
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
