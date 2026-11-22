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
GRAPHICS=/tmp/graphics
HELPER=/usr/local/bin/raspdacmini-boot
LEAVING=/usr/local/bin/raspdacmini-leaving
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

# The installer looks for the display's SPI device and for the kernel's
# framebuffers here, in place of /dev and /sys.
export RASPDACMINI_SPI_DEVICE="$SPI"
export RASPDACMINI_GRAPHICS="$GRAPHICS"
useradd -m volumio

fail() {
  echo "FAILED: $*" >&2
  echo "--- $USERCONFIG" >&2
  cat "$USERCONFIG" >&2 2>/dev/null || true
  echo "--- calls" >&2
  cat "$CALLS" >&2 2>/dev/null || true
  exit 1
}

# A player with the plugin's files unpacked and nothing installed yet: no
# panel of either kind, and the firmware's own framebuffer.
player() {
  rm -rf /data "$CALLS" "$ACTIVE" "$FAILING" "$LOADED" "$SPI" "$GRAPHICS" /etc/systemd/system/rdm*.service \
    /etc/systemd/system/rdmlcd.service.d /etc/systemd/system/raspdacmini*.service "$RULES" "$SUDOERS" \
    /etc/sudoers.d/volumio-user-raspdac-mini-lcd /usr/local/bin/rdmlcd-*.sh "$HELPER" "$LEAVING" /boot/overlays
  rm -f /boot/config.txt /boot/volumioconfig.txt "$USERCONFIG"
  mkdir -p "$PLUGIN/bin" "$ACTIVE" "$FAILING" "$GRAPHICS/fb0" "$GRAPHICS/fbcon" /boot /etc/systemd/system \
    /etc/udev/rules.d /etc/sudoers.d /usr/local/bin
  echo "BCM2708 FB" > "$GRAPHICS/fb0/name"
  cp -r /src/. "$PLUGIN/"
  printf '#!/bin/sh\necho "raspdacmini-panel 2.0.0"\n' > "$PLUGIN/bin/raspdacmini-panel"
  printf 'dtparam=audio=on\n' > /boot/config.txt
  : > "$CALLS"
}

# The panel as the kernel's driver gives it: a framebuffer by its name.
panel_framebuffer() {
  mkdir -p "$GRAPHICS/fb1"
  echo "fb_ili9341" > "$GRAPHICS/fb1/name"
}

# The panel left to the plugin's program: the SPI device.
panel_spi() {
  : > "$SPI"
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
  printf '{"lcd_active":{"type":"boolean","value":%s},"sleep_after":{"type":"number","value":900},"boot_splash":{"type":"boolean","value":true}' "$1" > "$SETTINGS"
  if [ -n "${2:-}" ]; then
    printf ',"display_driver":{"type":"string","value":"%s"}' "$2" >> "$SETTINGS"
  fi
  printf '}' >> "$SETTINGS"
}

# What version 1 left on a player: its five services with the folder of
# settings for one, its five scripts, its sudoers entry, its overlay's file,
# and its lines in the boot configuration. Its display service is running,
# on the framebuffer its overlay gives.
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
  panel_framebuffer
  printf 'dtoverlay=hifiberry-dac\n\n# RaspDacMini LCD Display\ndtoverlay=raspdac-mini-lcd\n# IR Remote Control (GPIO 4)\ndtoverlay=gpio-ir,gpio_pin=4\n' > "$USERCONFIG"
}

installed() {
  [ -f "$UNIT" ] || fail "$1: the unit is not installed"
  [ -f "$RULES" ] || fail "$1: the udev rule is not installed"
  [ "$(stat -c %a "$SUDOERS")" = 440 ] || fail "$1: the sudoers entry is not 0440"
  grep -q "enable --now raspdacmini-panel.service" "$SUDOERS" || fail "$1: the plugin may not start the display"
  grep -q "$HELPER framebuffer, $HELPER spi" "$SUDOERS" || fail "$1: the plugin may not set the boot configuration"
  [ -x "$HELPER" ] || fail "$1: the boot configuration's helper is not installed"
  [ "$(stat -c %U "$HELPER")" = root ] || fail "$1: the boot configuration's helper is not root's"
  [ -x "$LEAVING" ] || fail "$1: the helper the service runs as it stops is not installed"
  [ "$(stat -c %U "$LEAVING")" = root ] || fail "$1: the helper the service runs as it stops is not root's"
  grep -q "^ExecStop=+$LEAVING \$MAINPID\$" "$UNIT" || fail "$1: the service does not run that helper as it stops"
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
dtoverlay=fbtft,spi0-0,ili9341,bgr,reset_pin=24,dc_pin=27,rotate=90,speed=64000000,fps=100
dtoverlay=gpio-ir,gpio_pin=4
# <<< raspdac_mini_lcd'
block_spi='# >>> raspdac_mini_lcd
dtparam=spi=on
dtoverlay=gpio-ir,gpio_pin=4
# <<< raspdac_mini_lcd'

# --- A fresh install: the panel's overlay and the receiver's go into the
# --- plugin's own block, for the kernel to drive the panel. ------------------
player
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

# --- The helper the plugin runs when the setting changes: the block follows
# --- the way the display is driven, and says whether it changed. -------------
[ "$("$HELPER" framebuffer)" = unchanged ] || fail "helper: said the block changed when it had not"
[ "$("$HELPER" spi)" = changed ] || fail "helper: did not say the block changed"
[ "$(cat "$USERCONFIG")" = "dtoverlay=hifiberry-dac
# mine
$block_spi" ] || fail "helper: the block is not the one for the plugin's program"
[ "$("$HELPER" framebuffer)" = changed ] || fail "helper: did not say the block changed back"
[ "$(cat "$USERCONFIG")" = "$expected" ] || fail "helper: the block did not come back as it was"
if "$HELPER" hdmi > /dev/null 2>&1; then fail "helper: took a driver there is not"; fi
[ "$(cat "$USERCONFIG")" = "$expected" ] || fail "helper: a driver there is not changed the file"
echo "ok: the helper turns the block from one driver to the other and back, and says when it changed"

# --- The helper the service runs as it is stopped: it tells the display's
# --- program which way the system is going down, and says nothing where
# --- the service alone is stopped. --------------------------------------------
printf '#!/bin/sh\ncat /tmp/jobs\n' > /tmp/systemctl-jobs
chmod +x /tmp/systemctl-jobs
# The sleep on the path only writes down that it was asked for, which is
# what the installer's waits are checked by. What is waited for here is
# real, so this asks for the system's own.
NAP=/bin/sleep
rm -f /tmp/ready
sh -c 'trap "echo USR1 > /tmp/told" USR1; trap "echo USR2 > /tmp/told" USR2; : > /tmp/ready; while :; do /bin/sleep 0.05; done' &
program=$!
# It is told nothing before it can hear: a signal that came before its
# traps were set would end it.
waited=0
until [ -e /tmp/ready ]; do
  [ "$waited" -lt 100 ] || fail "leaving: the stand-in for the display's program did not start"
  "$NAP" 0.05
  waited=$((waited + 1))
done
# What the stand-in was told once the helper had run against these jobs: a
# signal's name, or "nothing". A signal that is expected is waited for;
# where none is, one is given the time it would take to arrive.
told() {
  rm -f /tmp/told
  printf '%s\n' "$1" > /tmp/jobs
  RASPDACMINI_SYSTEMCTL=/tmp/systemctl-jobs "$LEAVING" "$program" || fail "leaving: the helper failed"
  case "$2" in
    nothing) "$NAP" 0.3 ;;
    *)
      waited=0
      while [ ! -s /tmp/told ] && [ "$waited" -lt 100 ]; do
        "$NAP" 0.05
        waited=$((waited + 1))
      done
      ;;
  esac
  cat /tmp/told 2> /dev/null || echo nothing
}
[ "$(told '   812 reboot.target                start waiting
   813 raspdacmini-panel.service    stop  running
   814 systemd-reboot.service       start waiting' USR2)" = USR2 ] || fail "leaving: a restart was not told as one"
[ "$(told '    90 poweroff.target start waiting' USR1)" = USR1 ] || fail "leaving: a power-off was not told as one"
[ "$(told '    91 halt.target start waiting' USR1)" = USR1 ] || fail "leaving: a halt was not told as a power-off"
[ "$(told '    92 kexec.target start waiting' USR2)" = USR2 ] || fail "leaving: a kexec was not told as a restart"
[ "$(told '   813 raspdacmini-panel.service stop running' nothing)" = nothing ] || fail "leaving: a stop of the service alone was told as the system going down"
[ "$(told '    95 reboot.target stop waiting' nothing)" = nothing ] || fail "leaving: a target being stopped was taken for one being started"
[ "$(told '' nothing)" = nothing ] || fail "leaving: no jobs at all was told as something"
printf '#!/bin/sh\nexit 1\n' > /tmp/systemctl-jobs
[ "$(told '   812 reboot.target start waiting' nothing)" = nothing ] || fail "leaving: a systemd that does not answer was taken for an answer"
"$LEAVING" "not a number" || fail "leaving: a PID that is none made the helper fail"
"$LEAVING" || fail "leaving: no PID made the helper fail"
kill "$program" 2> /dev/null || true
echo "ok: the service's helper tells a restart from a power-off, and both from a stop of the service alone"

# --- The uninstall takes back everything and leaves the rest. ----------------
: > "$CALLS"
out=$(sh "$PLUGIN/uninstall.sh")
echo "$out" | grep -q "^pluginuninstallend$" || fail "uninstall: did not finish: $out"
[ "$(cat "$USERCONFIG")" = 'dtoverlay=hifiberry-dac
# mine' ] || fail "uninstall: the boot configuration is not as it was"
for left in "$UNIT" "$RULES" "$SUDOERS" "$HELPER" "$LEAVING"; do
  [ ! -e "$left" ] || fail "uninstall: $left is still there"
done
grep -q "^systemctl disable raspdacmini-panel.service" "$CALLS" || fail "uninstall: the service was not disabled"
echo "ok: the uninstall takes back the unit, the rules, the sudoers entry, the helper and the block"

# --- A last line with no end is not joined to the block's mark. --------------
player
printf 'dtoverlay=hifiberry-dac' > "$USERCONFIG"
sh "$PLUGIN/install.sh" > /dev/null
[ "$(cat "$USERCONFIG")" = "dtoverlay=hifiberry-dac
$block" ] || fail "no end: the boot configuration is not as expected"
sh "$PLUGIN/uninstall.sh" > /dev/null
[ "$(cat "$USERCONFIG")" = 'dtoverlay=hifiberry-dac' ] || fail "no end: the uninstall did not leave the line as it found it"
echo "ok: a last line with no end keeps to itself"

# --- Over version 1 in use: everything of it goes, its lines become this
# --- plugin's block, and the display starts at once on the framebuffer
# --- version 1's overlay still gives. ----------------------------------------
player
enabled_plugin true
version_1
echo "raspdac-mini-lcd" > "$LOADED"
out=$(sh "$PLUGIN/install.sh")
echo "$out" | grep -q "^plugininstallend$" || fail "over 1: the install did not finish: $out"
installed "over 1"
no_version_1 "over 1"
[ "$(cat "$USERCONFIG")" = "dtoverlay=hifiberry-dac

$block" ] || fail "over 1: the boot configuration is not as expected"
grep -q "^$STARTED" "$CALLS" || fail "over 1: the display was not started"
grep -q "^dtoverlay -r" "$CALLS" && fail "over 1: the overlay that gives the framebuffer was taken out"
echo "$out" | grep -q "The display is running the new version" || fail "over 1: no word that it runs: $out"
echo "$out" | grep -q "Restart the player to finish the update" || fail "over 1: no word about finishing the update: $out"
[ "$(grep -c '^dtoverlay=gpio-ir' "$USERCONFIG")" = 1 ] || fail "over 1: the receiver's overlay is there twice"
echo "ok: over version 1 its footprint goes and the display starts at once on the framebuffer it left"

# --- Set to be driven by the plugin's program, with the kernel still
# --- holding the panel: the block is the other one, and the display waits
# --- for the restart. --------------------------------------------------------
player
enabled_plugin true spi
panel_framebuffer
printf '%s\n' "$block" > "$USERCONFIG"
out=$(sh "$PLUGIN/install.sh")
installed "to spi"
[ "$(cat "$USERCONFIG")" = "$block_spi" ] || fail "to spi: the boot configuration is not as expected"
grep -q "^systemctl enable raspdacmini-panel.service" "$CALLS" || fail "to spi: the display was not enabled for the restart"
grep -q "^$STARTED" "$CALLS" && fail "to spi: the display was started while the kernel holds the panel"
echo "$out" | grep -q "stays dark until then" || fail "to spi: no word that the display waits for the restart: $out"
echo "ok: set to the plugin's program while the kernel holds the panel, the display waits for the restart"

# --- The same with version 1's overlay loaded while the player ran: it is
# --- taken out, the SPI device comes back, and the display starts at once. ---
player
enabled_plugin true spi
version_1
echo "raspdac-mini-lcd" > "$LOADED"
out=$(sh "$PLUGIN/install.sh")
installed "to spi, unloaded"
no_version_1 "to spi, unloaded"
grep -q "^dtoverlay -r raspdac-mini-lcd" "$CALLS" || fail "to spi, unloaded: the overlay was not taken out"
grep -q "^$STARTED" "$CALLS" || fail "to spi, unloaded: the display was not started"
echo "$out" | grep -q "The display is running the new version" || fail "to spi, unloaded: no word that it runs: $out"
echo "ok: an overlay version 1 loaded while running is taken out for the plugin's program, which starts at once"

# --- An update of this version in use, either way it is driven. --------------
for driver in framebuffer spi; do
  player
  enabled_plugin true "$driver"
  if [ "$driver" = spi ]; then
    panel_spi
    printf '%s\n' "$block_spi" > "$USERCONFIG"
  else
    panel_framebuffer
    printf '%s\n' "$block" > "$USERCONFIG"
  fi
  before=$(cat "$USERCONFIG")
  out=$(sh "$PLUGIN/install.sh")
  installed "update, $driver"
  grep -q "^$STARTED" "$CALLS" || fail "update, $driver: the display was not started"
  [ "$(cat "$USERCONFIG")" = "$before" ] || fail "update, $driver: the boot configuration changed"
  echo "$out" | grep -q "The display is running the new version" || fail "update, $driver: no word that it runs: $out"
  echo "$out" | grep -q "Restart the player to finish the update" || fail "update, $driver: no word about finishing the update"
  grep -q "^systemctl unmask" "$CALLS" && fail "update, $driver: something was unmasked with nothing of version 1 there"
done
echo "ok: an update of a plugin in use starts the display on the new program, whichever way it is driven"

# --- A display that does not stay up is said, not passed over. ---------------
player
enabled_plugin true
panel_framebuffer
: > "$FAILING/raspdacmini-panel.service"
out=$(sh "$PLUGIN/install.sh")
echo "$out" | grep -q "did not stay up" || fail "not up: no word about it: $out"
echo "$out" | grep -q "The display is running" && fail "not up: said to be running"
echo "ok: a display that does not stay up is said so"

# --- A display switched off in the settings stays off through an update. -----
player
enabled_plugin false
panel_framebuffer
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
dtoverlay=fbtft,spi0-0,ili9341,bgr,reset_pin=24,dc_pin=27,rotate=90,speed=64000000,fps=100
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
