#!/bin/sh
# The installer and the uninstaller, run as root in a container laid out
# like a player. See scripts/installers.sh.
set -eu

PLUGIN=/data/plugins/system_hardware/raspdac_mini_lcd
SETTINGS=/data/configuration/system_hardware/raspdac_mini_lcd/config.json
PLUGINS=/data/configuration/plugins.json
CALLS=/tmp/calls
ACTIVE=/tmp/active
FAILING=/tmp/failing
LOADED=/tmp/overlays-loaded
SPI=/tmp/spidev0.0
USERCONFIG=/boot/userconfig.txt
UNIT=/etc/systemd/system/raspdacmini-panel.service
RULES=/etc/udev/rules.d/70-raspdacmini-remote.rules
SUDOERS=/etc/sudoers.d/volumio-user-raspdac_mini_lcd
STARTED="systemctl enable --now raspdacmini-panel.service"
OLD_UNITS="rdmlcd rdmlcd-splash rdmlcd-shutdown rdm_remote rdm_irexec"
OLD_HELPERS="rdmlcd-update-env.sh rdmlcd-show-splash.sh rdmlcd-shutdown-splash.sh rdmlcd-plugin-services.sh rdmlcd-overlay.sh"
OLD_MASKED="lircd.service lircd.socket plymouth-start.service plymouth-kexec.service plymouth-poweroff.service plymouth-reboot.service plymouth-halt.service"

# What the scripts ask of the system is written down, not done. A unit is
# active when the case says so, or once it has been started, unless the case
# says it does not stay up. An overlay is loaded while its name is in a
# file, and taking it out takes the name out.
mkdir -p /usr/local/sbin
for tool in udevadm visudo sleep; do
  printf '#!/bin/sh\necho "%s $*" >> %s\n' "$tool" "$CALLS" > "/usr/local/sbin/$tool"
  chmod +x "/usr/local/sbin/$tool"
done
cat > /usr/local/sbin/systemctl << STUB
#!/bin/sh
echo "systemctl \$*" >> $CALLS
case "\$1" in
  is-active)
    for unit in "\$@"; do
      [ -e "$ACTIVE/\$unit" ] && exit 0
    done
    exit 3
    ;;
  enable)
    if [ "\$2" = --now ] && [ ! -e "$FAILING/\$3" ]; then
      : > "$ACTIVE/\$3"
    fi
    ;;
  stop)
    rm -f "$ACTIVE/\$2"
    ;;
esac
exit 0
STUB
cat > /usr/local/sbin/dtoverlay << STUB
#!/bin/sh
echo "dtoverlay \$*" >> $CALLS
case "\$1" in
  -l)
    echo "Overlays (in load order):"
    [ -s "$LOADED" ] && sed 's/^/0:  /' "$LOADED"
    ;;
  -r)
    grep -qx "\$2" "$LOADED" 2>/dev/null || exit 1
    : > "$LOADED"
    # With the overlay gone the kernel gives the SPI device back.
    : > "$SPI"
    ;;
esac
exit 0
STUB
chmod +x /usr/local/sbin/systemctl /usr/local/sbin/dtoverlay

# The installer looks for the display's SPI device here, in place of /dev.
export RASPDACMINI_SPI_DEVICE="$SPI"
useradd -m volumio

fail() {
  echo "FAILED: $*" >&2
  echo "--- $USERCONFIG" >&2
  cat "$USERCONFIG" >&2 2>/dev/null || true
  echo "--- calls" >&2
  cat "$CALLS" >&2 2>/dev/null || true
  exit 1
}

# A player with the plugin's files unpacked and nothing installed yet. The
# SPI bus is on, as it is on a player that has restarted since the plugin
# was first installed.
player() {
  rm -rf /data "$CALLS" "$ACTIVE" "$FAILING" "$LOADED" "$SPI" /etc/systemd/system/rdm*.service \
    /etc/systemd/system/rdmlcd.service.d /etc/systemd/system/raspdacmini*.service "$RULES" "$SUDOERS" \
    /etc/sudoers.d/volumio-user-raspdac-mini-lcd /usr/local/bin/rdmlcd-*.sh /boot/overlays
  rm -f /boot/config.txt /boot/volumioconfig.txt "$USERCONFIG"
  mkdir -p "$PLUGIN/bin" "$ACTIVE" "$FAILING" /boot /etc/systemd/system /etc/udev/rules.d /etc/sudoers.d /usr/local/bin
  cp -r /src/. "$PLUGIN/"
  printf '#!/bin/sh\necho "raspdacmini-panel 1.99.0"\n' > "$PLUGIN/bin/raspdacmini-panel"
  printf 'dtparam=audio=on\n' > /boot/config.txt
  : > "$SPI"
  : > "$CALLS"
}

# Volumio's record of the plugin, as it writes it, and the plugin's settings.
enabled_plugin() {
  mkdir -p "$(dirname "$SETTINGS")"
  cat > "$PLUGINS" << 'JSON'
{
  "system_hardware": {
    "raspdac_mini_lcd": {
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
  printf '{"lcd_active":{"type":"boolean","value":%s},"sleep_after":{"type":"number","value":900},"boot_splash":{"type":"boolean","value":true}}' "$1" > "$SETTINGS"
}

# What version 1 left on a player: its five services with the folder of
# settings for one, its five scripts, its sudoers entry, its overlay's file,
# and its lines in the boot configuration. Its display service is running.
version_1() {
  mkdir -p "$PLUGIN/compositor" "$PLUGIN/native" "$PLUGIN/scripts" "$PLUGIN/lirc" \
    /etc/systemd/system/rdmlcd.service.d /boot/overlays
  for old in $OLD_UNITS; do
    : > "/etc/systemd/system/$old.service"
  done
  : > /etc/systemd/system/rdmlcd.service.d/override.conf
  for helper in $OLD_HELPERS; do
    : > "/usr/local/bin/$helper"
  done
  : > /etc/sudoers.d/volumio-user-raspdac-mini-lcd
  : > /boot/overlays/raspdac-mini-lcd.dtbo
  : > /data/.rdmlcd-shutdown.log
  : > "$ACTIVE/rdmlcd.service"
  printf 'dtoverlay=hifiberry-dac\n\n# RaspDacMini LCD Display\ndtoverlay=raspdac-mini-lcd\n# IR Remote Control (GPIO 4)\ndtoverlay=gpio-ir,gpio_pin=4\n' > "$USERCONFIG"
}

installed() {
  [ -f "$UNIT" ] || fail "$1: the unit is not installed"
  [ -f "$RULES" ] || fail "$1: the udev rule is not installed"
  [ "$(stat -c %a "$SUDOERS")" = 440 ] || fail "$1: the sudoers entry is not 0440"
  grep -q "enable --now raspdacmini-panel.service" "$SUDOERS" || fail "$1: the plugin may not start the display"
  [ "$(stat -c %U "$PLUGIN/index.js")" = volumio ] || fail "$1: the plugin folder is not volumio's"
  [ -x "$PLUGIN/bin/raspdacmini-panel" ] || fail "$1: the display's program is not executable"
  grep -q "^systemctl daemon-reload" "$CALLS" || fail "$1: systemd was not asked to reload"
  grep -q "^udevadm trigger --action=change --subsystem-match=rc --subsystem-match=lirc" "$CALLS" || fail "$1: udev was not asked for the decoder and the device"
  grep -q "^visudo -c -f $SUDOERS" "$CALLS" || fail "$1: the sudoers entry was not checked"
}

no_version_1() {
  for old in $OLD_UNITS; do
    [ ! -e "/etc/systemd/system/$old.service" ] || fail "$1: $old.service is still there"
    grep -q "^systemctl stop $old.service" "$CALLS" || fail "$1: $old was not stopped"
    grep -q "^systemctl disable $old.service" "$CALLS" || fail "$1: $old was not disabled"
  done
  [ ! -e /etc/systemd/system/rdmlcd.service.d ] || fail "$1: version 1's settings for its service are still there"
  for helper in $OLD_HELPERS; do
    [ ! -e "/usr/local/bin/$helper" ] || fail "$1: $helper is still there"
  done
  [ ! -e /etc/sudoers.d/volumio-user-raspdac-mini-lcd ] || fail "$1: version 1's sudoers entry is still there"
  [ ! -e /boot/overlays/raspdac-mini-lcd.dtbo ] || fail "$1: version 1's overlay file is still there"
  [ ! -e /data/.rdmlcd-shutdown.log ] || fail "$1: version 1's log is still there"
  for unit in $OLD_MASKED; do
    grep -q "^systemctl unmask $unit" "$CALLS" || fail "$1: $unit was not given back"
  done
  for folder in compositor native scripts lirc; do
    [ ! -e "$PLUGIN/$folder" ] || fail "$1: version 1's $folder is still in the plugin's folder"
  done
}

block='# >>> raspdac_mini_lcd
dtparam=spi=on
dtoverlay=gpio-ir,gpio_pin=4
# <<< raspdac_mini_lcd'

# --- A fresh install: the two lines go into the plugin's own block. ----------
player
rm -f "$SPI"
printf 'dtoverlay=hifiberry-dac\n# mine\n' > "$USERCONFIG"
out=$(sh "$PLUGIN/install.sh")
echo "$out" | grep -q "^plugininstallend$" || fail "fresh: the install did not finish: $out"
installed fresh
expected="dtoverlay=hifiberry-dac
# mine
$block"
[ "$(cat "$USERCONFIG")" = "$expected" ] || fail "fresh: the boot configuration is not as expected"
echo "$out" | grep -q "Restart the player once" || fail "fresh: no word about the restart"
grep -q "^systemctl enable" "$CALLS" && fail "fresh: the display was enabled for a plugin nobody has enabled"
grep -q "^systemctl unmask" "$CALLS" && fail "fresh: something was unmasked on a player version 1 was never on"
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
for left in "$UNIT" "$RULES" "$SUDOERS"; do
  [ ! -e "$left" ] || fail "uninstall: $left is still there"
done
grep -q "^systemctl disable raspdacmini-panel.service" "$CALLS" || fail "uninstall: the service was not disabled"
echo "ok: the uninstall takes back the unit, the rules, the sudoers entry and the block"

# --- A last line with no end is not joined to the block's mark. --------------
player
printf 'dtoverlay=hifiberry-dac' > "$USERCONFIG"
sh "$PLUGIN/install.sh" > /dev/null
[ "$(cat "$USERCONFIG")" = "dtoverlay=hifiberry-dac
$block" ] || fail "no end: the boot configuration is not as expected"
sh "$PLUGIN/uninstall.sh" > /dev/null
[ "$(cat "$USERCONFIG")" = 'dtoverlay=hifiberry-dac' ] || fail "no end: the uninstall did not leave the line as it found it"
echo "ok: a last line with no end keeps to itself"

# --- Over version 1 in use, its overlay loaded at boot: everything of it
# --- goes, its lines become this plugin's block, and the display, which the
# --- old overlay still holds, is enabled for the restart and not started. ----
player
enabled_plugin true
version_1
rm -f "$SPI"
out=$(sh "$PLUGIN/install.sh")
echo "$out" | grep -q "^plugininstallend$" || fail "over 1: the install did not finish: $out"
installed "over 1"
no_version_1 "over 1"
[ "$(cat "$USERCONFIG")" = "dtoverlay=hifiberry-dac

$block" ] || fail "over 1: the boot configuration is not as expected"
grep -q "^systemctl enable raspdacmini-panel.service" "$CALLS" || fail "over 1: the display was not enabled for the restart"
grep -q "^$STARTED" "$CALLS" && fail "over 1: the display was started while the old overlay holds it"
echo "$out" | grep -q "stays dark until then" || fail "over 1: no word that the display waits for the restart: $out"
[ "$(grep -c '^dtoverlay=gpio-ir' "$USERCONFIG")" = 1 ] || fail "over 1: the receiver's overlay is there twice"
echo "ok: over version 1 its footprint goes, and the display waits for the restart the old overlay needs"

# --- Over version 1 with its overlay loaded while the player ran: it is
# --- taken out, the SPI device comes back, and the display starts at once. ---
player
enabled_plugin true
version_1
rm -f "$SPI"
echo "raspdac-mini-lcd" > "$LOADED"
out=$(sh "$PLUGIN/install.sh")
installed "over 1, unloaded"
no_version_1 "over 1, unloaded"
grep -q "^dtoverlay -r raspdac-mini-lcd" "$CALLS" || fail "over 1, unloaded: the overlay was not taken out"
grep -q "^$STARTED" "$CALLS" || fail "over 1, unloaded: the display was not started"
echo "$out" | grep -q "The display is running the new version" || fail "over 1, unloaded: no word that it runs: $out"
echo "ok: an overlay version 1 loaded while running is taken out and the display starts at once"

# --- An update of this version in use starts the display on the new program. -
player
enabled_plugin true
printf '%s\n' "$block" > "$USERCONFIG"
out=$(sh "$PLUGIN/install.sh")
installed update
grep -q "^$STARTED" "$CALLS" || fail "update: the display was not started"
[ "$(cat "$USERCONFIG")" = "$block" ] || fail "update: the boot configuration changed"
echo "$out" | grep -q "The display is running the new version" || fail "update: no word that it runs: $out"
echo "$out" | grep -q "Restart the player to finish the update" || fail "update: no word about finishing the update"
grep -q "^systemctl unmask" "$CALLS" && fail "update: something was unmasked with nothing of version 1 there"
echo "ok: an update of a plugin in use starts the display on the new program"

# --- A display that does not stay up is said, not passed over. ---------------
player
enabled_plugin true
: > "$FAILING/raspdacmini-panel.service"
out=$(sh "$PLUGIN/install.sh")
echo "$out" | grep -q "did not stay up" || fail "not up: no word about it: $out"
echo "$out" | grep -q "The display is running" && fail "not up: said to be running"
echo "ok: a display that does not stay up is said so"

# --- A display switched off in the settings stays off through an update. -----
player
enabled_plugin false
printf '%s\n' "$block" > "$USERCONFIG"
out=$(sh "$PLUGIN/install.sh")
installed "display off"
grep -q "^systemctl enable" "$CALLS" && fail "display off: the display was enabled"
echo "$out" | grep -q "Restart the player to finish the update" || fail "display off: no word about finishing the update"
echo "ok: a display switched off in the settings stays off through an update"

# --- The receiver's overlay written another way is not written twice, and
# --- a line of the user's own is not version 1's to take. --------------------
player
printf 'dtoverlay=gpio-ir,gpio_pin=4,rc-map-name=rc-empty\n# IR Remote Control (GPIO 4)\n' > "$USERCONFIG"
sh "$PLUGIN/install.sh" > /dev/null
[ "$(cat "$USERCONFIG")" = 'dtoverlay=gpio-ir,gpio_pin=4,rc-map-name=rc-empty
# >>> raspdac_mini_lcd
dtparam=spi=on
# <<< raspdac_mini_lcd' ] || fail "variant: the boot configuration is not as expected"
echo "ok: a receiver overlay already there is kept and not written a second time"

# --- Version 1's comment over another line takes only the comment. -----------
player
printf '# IR Remote Control (GPIO 4)\ndtoverlay=hifiberry-dac\n' > "$USERCONFIG"
sh "$PLUGIN/install.sh" > /dev/null
[ "$(cat "$USERCONFIG")" = "dtoverlay=hifiberry-dac
$block" ] || fail "comment: the boot configuration is not as expected"
echo "ok: version 1's comment goes and the line under it stays when it is not version 1's"

# --- A program that does not run is refused, and nothing is left. ------------
player
printf '#!/bin/sh\nexit 126\n' > "$PLUGIN/bin/raspdacmini-panel"
if out=$(sh "$PLUGIN/install.sh"); then fail "refusal: an install with a program that does not run went through"; fi
echo "$out" | grep -q "does not run on this machine" || fail "refusal: no word why: $out"
[ ! -e "$PLUGIN" ] || fail "refusal: the plugin folder was left behind"
[ ! -e "$UNIT" ] || fail "refusal: the unit was installed"
echo "ok: a program that does not run is refused before anything is installed"

echo "installers: every case passed"
