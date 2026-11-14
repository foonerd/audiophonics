#!/bin/sh
# Audiophonics Evo Sabre installer. Runs as root through the plugin manager;
# its output is shown in the install dialog and a non-zero exit fails the
# install. What it puts outside the plugin folder, uninstall.sh takes back.
# Volumio runs this with sh, so it keeps to what sh has.

echo "Installing Audiophonics Evo Sabre"

PLUGIN_DIR="/data/plugins/system_hardware/audiophonics_evo_sabre"
BIN="$PLUGIN_DIR/bin/evosabre-panel"
SERVICE="evosabre-panel"
UNIT="/etc/systemd/system/$SERVICE.service"
RULES="/etc/udev/rules.d/70-evosabre-remote.rules"
SUDOERS="/etc/sudoers.d/volumio-user-audiophonics_evo_sabre"
USERCONFIG="/boot/userconfig.txt"
MARK_BEGIN="# >>> audiophonics_evo_sabre"
MARK_END="# <<< audiophonics_evo_sabre"

# A refused install leaves nothing behind, so the next attempt starts clean.
refuse() {
  echo ""
  echo "$@"
  rm -rf "$PLUGIN_DIR"
  exit 1
}

# =============================================================================
# CHECK: the display's program runs on this machine. The unzip does not keep
# modes. It is one static 32-bit ARM binary, for the userland Volumio runs on
# every Raspberry Pi.
# =============================================================================
chmod +x "$BIN" 2>/dev/null || true
if [ ! -x "$BIN" ]; then
  refuse "ERROR: the display's program is missing from the package ($BIN)"
fi
if ! VERSION=$("$BIN" --version 2>/dev/null); then
  refuse "ERROR: the display's program does not run on this machine ($(uname -m)); the EVO Sabre is a Raspberry Pi"
fi
echo "Program: $VERSION"

# =============================================================================
# REPLACE: what version 2 of this plugin installed. Its three services ran
# the display and the remote through Node and lirc; this version needs
# neither. The lirc package is left alone: another plugin may use it.
# =============================================================================
for old in evo_oled2 evo_remote evo_irexec; do
  if [ -e "/etc/systemd/system/$old.service" ] || [ -L "/etc/systemd/system/$old.service" ]; then
    systemctl stop "$old.service" 2>/dev/null || true
    systemctl disable "$old.service" 2>/dev/null || true
    rm -f "/etc/systemd/system/$old.service"
    echo "Removed the version 2 service $old"
  fi
done
rm -rf "$PLUGIN_DIR/apps" "$PLUGIN_DIR/service"

# =============================================================================
# SETUP: the boot configuration. The display is on the SPI bus and the
# receiver is the gpio-ir overlay on GPIO 4. Each line goes into this
# plugin's own block of userconfig.txt, unless the boot configuration already
# carries it: a line written twice loads the overlay twice.
# =============================================================================
REBOOT=0
if [ -f /boot/config.txt ]; then
  outside=$(mktemp)
  block=$(mktemp)
  touch "$USERCONFIG"
  sed "/^$MARK_BEGIN\$/,/^$MARK_END\$/d" "$USERCONFIG" > "$outside"
  : > "$block"
  if ! cat /boot/config.txt /boot/volumioconfig.txt "$outside" 2>/dev/null | grep -qx "dtparam=spi=on"; then
    echo "dtparam=spi=on" >> "$block"
  fi
  if ! cat /boot/config.txt /boot/volumioconfig.txt "$outside" 2>/dev/null | grep -qE "^dtoverlay=gpio-ir(,|\$)"; then
    echo "dtoverlay=gpio-ir,gpio_pin=4" >> "$block"
  fi
  if [ -s "$block" ]; then
    {
      echo "$MARK_BEGIN"
      cat "$block"
      echo "$MARK_END"
    } >> "$outside"
  fi
  if ! cmp -s "$outside" "$USERCONFIG"; then
    cat "$outside" > "$USERCONFIG"
    REBOOT=1
    echo "Boot configuration updated: $USERCONFIG"
  fi
  rm -f "$outside" "$block"
else
  echo "WARNING: no /boot/config.txt here; the SPI bus and the gpio-ir overlay are not set up"
fi

# =============================================================================
# SETUP: the receiver decodes the handset's frames. The rule asks for the NEC
# decoder whenever the receiver appears; the trigger asks for it now, on a
# player whose receiver is already there.
# =============================================================================
install -m 644 "$PLUGIN_DIR/udev/70-evosabre-remote.rules" "$RULES"
udevadm control --reload 2>/dev/null || true
udevadm trigger --action=change --subsystem-match=rc 2>/dev/null || true

# =============================================================================
# SETUP: the groups the service takes for the SPI bus, the GPIO lines and the
# receiver's input device. Volumio's Raspberry Pi image has all three; one
# that is missing is made, so the service starts and says what it cannot
# open.
# =============================================================================
for group in spi gpio input; do
  if ! getent group "$group" >/dev/null 2>&1; then
    groupadd -f "$group"
    echo "WARNING: the group $group was missing and has been made; its devices may not be open to it"
  fi
done

# =============================================================================
# SETUP: sudo for what the plugin asks of the service as the volumio user.
# Named volumio-user-* so it sorts after Volumio's own rules.
# =============================================================================
cat > "$SUDOERS" << 'SUDO'
# audiophonics_evo_sabre plugin: the display's service, switched with the plugin's Display setting
volumio ALL=(ALL) NOPASSWD: /bin/systemctl enable --now evosabre-panel.service, /bin/systemctl disable --now evosabre-panel.service
SUDO
chmod 0440 "$SUDOERS"
if ! visudo -c -f "$SUDOERS" >/dev/null 2>&1; then
  rm -f "$SUDOERS"
  refuse "ERROR: the sudoers entry did not validate"
fi

# =============================================================================
# SETUP: the display's service. The plugin enables it when it starts and
# disables it when it stops, so the display greets from boot only while the
# plugin is on.
# =============================================================================
systemctl stop "$SERVICE.service" 2>/dev/null || true
install -m 644 "$PLUGIN_DIR/systemd/$SERVICE.service" "$UNIT"
systemctl daemon-reload

chown -R volumio:volumio "$PLUGIN_DIR"

echo "Audiophonics Evo Sabre installed"
if [ "$REBOOT" = 1 ]; then
  echo "Restart the player once: the display and the remote come up on the new boot configuration"
fi
echo "plugininstallend"
