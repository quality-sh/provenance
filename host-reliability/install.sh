#!/bin/sh
# install.sh — idempotent installer for the host-reliability artifacts on "mint".
#
# Safe by construction:
#   * copies files, reloads systemd unit definitions, enables two TIMERS
#   * ensures connection.autoconnect=yes on the wifi profile (metadata-only
#     change; does NOT bounce any interface or restart any running service)
#   * runs one harmless probe round of net-watch at the end as verification
#   * never restarts opencode2-server, never touches links, never reboots
#
# Can be invoked two ways (both handled):
#   as ben:            ./install.sh            (uses sudo for the system side)
#   via sudo:          sudo ./install.sh       (user side runs as SUDO_USER)

set -eu
cd "$(dirname "$0")"

# --- figure out user context -------------------------------------------------
if [ "$(id -u)" -eq 0 ]; then
    [ -n "${SUDO_USER:-}" ] || { echo "ERROR: run as the target user (ben), or via 'sudo ./install.sh'"; exit 1; }
    TARGET_USER="$SUDO_USER"
    TARGET_HOME=$(getent passwd "$TARGET_USER" | cut -d: -f6)
    TARGET_RUNTIME="/run/user/$(id -u "$TARGET_USER")"
    as_root() { "$@"; }
    as_user() { sudo -u "$TARGET_USER" env XDG_RUNTIME_DIR="$TARGET_RUNTIME" HOME="$TARGET_HOME" "$@"; }
else
    as_root() { sudo "$@"; }
    as_user() { "$@"; }
fi

# --- system side (network watchdog) ------------------------------------------
as_root install -D -m 0755 network/net-watch.sh      /usr/local/sbin/net-watch.sh
as_root install -D -m 0644 network/net-watch.service /etc/systemd/system/net-watch.service
as_root install -D -m 0644 network/net-watch.timer   /etc/systemd/system/net-watch.timer
as_root systemctl daemon-reload

# --- user side (opencode2 provider watchdog) ---------------------------------
as_user install -D -m 0755 opencode2/opencode2-provider-watch.sh \
    "$HOME/.local/bin/opencode2-provider-watch.sh"
as_user install -D -m 0644 opencode2/opencode2-provider-watch.service \
    "$HOME/.config/systemd/user/opencode2-provider-watch.service"
as_user install -D -m 0644 opencode2/opencode2-provider-watch.timer \
    "$HOME/.config/systemd/user/opencode2-provider-watch.timer"
as_user systemctl --user daemon-reload

# --- verify/enable autoconnect on the wifi profile ---------------------------
# Metadata-only change (no disconnect). Runs as root so it also works over SSH.
WIFI_IF=$(nmcli -t -f DEVICE,TYPE device status | awk -F: '$2=="wifi"{print $1; exit}')
[ -n "$WIFI_IF" ] || { echo "ERROR: no wifi device found"; exit 1; }
PROFILE=$(nmcli -g GENERAL.CONNECTION device show "$WIFI_IF" | head -n1)
if [ -z "$PROFILE" ] || [ "$PROFILE" = "--" ]; then
    PROFILE=$(nmcli -t -f NAME,TYPE connection show | awk -F: '$2=="802-11-wireless"{print $1; exit}')
fi
[ -n "$PROFILE" ] || { echo "ERROR: no wifi profile found"; exit 1; }
CURRENT=$(nmcli -g connection.autoconnect connection show "$PROFILE")
echo "wifi profile '$PROFILE' on $WIFI_IF: connection.autoconnect=$CURRENT"
if [ "$CURRENT" != "yes" ]; then
    as_root nmcli connection modify "$PROFILE" connection.autoconnect yes
    echo "set connection.autoconnect=yes on '$PROFILE'"
fi

# --- enable timers (idempotent) -----------------------------------------------
as_root systemctl enable --now net-watch.timer
as_user systemctl --user enable --now opencode2-provider-watch.timer

# --- verification --------------------------------------------------------------
echo
echo "== verification =="
as_root systemctl is-enabled net-watch.timer
as_root systemctl is-active net-watch.timer
as_user systemctl --user is-enabled opencode2-provider-watch.timer
as_user systemctl --user is-active opencode2-provider-watch.timer
# One harmless probe round (read-only; worst case ~90s). On a healthy network
# this logs "connectivity OK" and exits 0.
/usr/local/sbin/net-watch.sh || true
echo
echo "Installed. Follow with: journalctl -u net-watch -f   |   journalctl --user -u opencode2-provider-watch -f"
