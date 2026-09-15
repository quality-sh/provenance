#!/bin/sh
# net-watch.sh — WiFi connectivity watchdog with staged repair (NetworkManager stack).
#
# Design:
#   * Probes several targets each round: local gateway (auto-detected from the
#     default route), 1.1.1.1, 8.8.8.8, and the tailscale MagicDNS address
#     (100.100.100.100 — reachable whenever tailscaled is healthy, so it doubles
#     as a tunnel-health probe).
#   * A single bad round does nothing. Repairs start only after FAIL_THRESHOLD
#     CONSECUTIVE majority-failed rounds, so brief blips never trigger action.
#   * Repairs escalate one stage per sustained failure episode:
#       0. re-activate the wifi profile (nmcli connection up = DHCP renew)
#       1. bounce the wifi interface (ip link down/up) + re-activate
#       2. restart wpa_supplicant, then NetworkManager
#   * Everything is logged to the journal (stdout is captured by journald under
#     the net-watch.service unit). The machine is NEVER rebooted by this script.
#
# Kill switch:  touch /run/net-watch.disabled   -> probes run, repairs skipped.
# Test mode:    DRY_RUN=1                       -> log intended repairs, do nothing.
#               Overridable: TARGETS, STATE_FILE, DISABLE_FILE, FAIL_THRESHOLD,
#               PING_WAIT, REPAIR_TIMEOUT (all env).

set -u

WIFI_IF_DEFAULT="wlp6s0"
STATE_FILE="${STATE_FILE:-/run/net-watch.state}"
DISABLE_FILE="${DISABLE_FILE:-/run/net-watch.disabled}"
FAIL_THRESHOLD="${FAIL_THRESHOLD:-3}"    # consecutive failed rounds before acting
PING_WAIT="${PING_WAIT:-3}"              # per-probe timeout, seconds
REPAIR_TIMEOUT="${REPAIR_TIMEOUT:-75}"   # per repair command timeout, seconds
DRY_RUN="${DRY_RUN:-0}"

log() { printf 'net-watch: %s\n' "$*"; }
die() { log "FATAL: $*"; exit 1; }

# --- interface resolution (default, then auto-detect) ----------------------
IF="$WIFI_IF_DEFAULT"
if ! ip link show "$IF" >/dev/null 2>&1; then
    IF=$(nmcli -t -f DEVICE,TYPE device status 2>/dev/null | awk -F: '$2=="wifi"{print $1; exit}')
    [ -n "$IF" ] || die "no wifi interface found"
fi

gateway() {
    ip route show default dev "$IF" 2>/dev/null | awk '{for (i = 1; i < NF; i++) if ($i == "via") {print $(i+1); exit}}'
}

wifi_profile() {
    # Prefer the profile currently bound to the interface; fall back to the
    # first 802-11-wireless profile known to NetworkManager.
    prof=$(nmcli -g GENERAL.CONNECTION device show "$IF" 2>/dev/null | head -n1)
    if [ -n "$prof" ] && [ "$prof" != "--" ]; then
        printf '%s' "$prof"
        return 0
    fi
    nmcli -t -f NAME,TYPE connection show 2>/dev/null | awk -F: '$2=="802-11-wireless"{print $1; exit}'
}

# --- probe targets ---------------------------------------------------------
GATEWAY=$(gateway)
if [ -z "$GATEWAY" ]; then
    GATEWAY="192.168.4.1"   # last known gateway (fallback only)
    log "WARNING: no default route via $IF; using fallback gateway $GATEWAY"
fi
TARGETS="${TARGETS:-$GATEWAY 1.1.1.1 8.8.8.8 100.100.100.100}"

probe() { ping -n -q -c 1 -W "$PING_WAIT" "$1" >/dev/null 2>&1; }

FAILS=0
TOTAL=0
MAJORITY_NEEDED=1
run_probes() {
    FAILS=0
    TOTAL=0
    for t in $TARGETS; do
        TOTAL=$((TOTAL + 1))
        if probe "$t"; then
            :
        else
            FAILS=$((FAILS + 1))
            log "probe FAIL: $t"
        fi
    done
    MAJORITY_NEEDED=$((TOTAL / 2 + 1))
}

healthy() { [ "$FAILS" -lt "$MAJORITY_NEEDED" ]; }

# --- state -----------------------------------------------------------------
# State file format: "<consecutive_failed_rounds> <repair_level>"
state_read() {
    f=0
    l=0
    if [ -r "$STATE_FILE" ]; then
        set -- $(cat "$STATE_FILE" 2>/dev/null)
        case "${1:-}" in '' | *[!0-9]*) f=0 ;; *) f=$1 ;; esac
        case "${2:-}" in '' | *[!0-9]*) l=0 ;; *) l=$2 ;; esac
    fi
}
state_write() { printf '%s %s\n' "$1" "$2" > "$STATE_FILE" 2>/dev/null; }
state_clear() { rm -f "$STATE_FILE" 2>/dev/null; }

# --- recovery wait ---------------------------------------------------------
# Returns 0 once a probe round is healthy on 2 consecutive rounds (10s apart).
wait_recovery() {
    rounds="${1:-6}"
    streak=0
    i=0
    while [ "$i" -lt "$rounds" ]; do
        sleep 10
        run_probes
        if healthy; then
            streak=$((streak + 1))
            if [ "$streak" -ge 2 ]; then
                return 0
            fi
        else
            streak=0
        fi
        i=$((i + 1))
    done
    return 1
}

# --- repair stages (escalating) --------------------------------------------
stage1_renew() {
    prof=$(wifi_profile) || { log "stage 1: no wifi profile found"; return 1; }
    log "STAGE 1: re-activating wifi profile '$prof' on $IF (nmcli connection up / DHCP renew)"
    if [ "$DRY_RUN" = "1" ]; then
        log "DRY-RUN: nmcli connection up '$prof'"
        return 0
    fi
    timeout "$REPAIR_TIMEOUT" nmcli connection up "$prof" >/dev/null 2>&1
}

stage2_bounce() {
    log "STAGE 2: bouncing interface $IF (ip link down/up), then re-activating profile"
    if [ "$DRY_RUN" = "1" ]; then
        log "DRY-RUN: ip link set dev $IF down/up + nmcli connection up"
        return 0
    fi
    ip link set dev "$IF" down
    sleep 3
    ip link set dev "$IF" up
    sleep 2
    prof=$(wifi_profile) || { log "stage 2: no wifi profile found"; return 1; }
    timeout "$REPAIR_TIMEOUT" nmcli connection up "$prof" >/dev/null 2>&1
}

stage3_restart_stack() {
    log "STAGE 3: restarting wpa_supplicant, then NetworkManager (autoconnect re-raises wifi)"
    if [ "$DRY_RUN" = "1" ]; then
        log "DRY-RUN: systemctl try-restart wpa_supplicant.service NetworkManager.service"
        return 0
    fi
    systemctl try-restart wpa_supplicant.service 2>/dev/null
    sleep 2
    systemctl try-restart NetworkManager.service 2>/dev/null
}

repair_stage() {
    case "$1" in
        0) stage1_renew ;;
        1) stage2_bounce ;;
        *) stage3_restart_stack ;;
    esac
}

# --- main ------------------------------------------------------------------
run_probes

if healthy; then
    state_read
    if [ "$f" -gt 0 ]; then
        log "connectivity RECOVERED (was degraded; $FAILS/$TOTAL probes failed this round) — no action taken"
    else
        log "connectivity OK ($FAILS/$TOTAL probes failed)"
    fi
    state_clear
    exit 0
fi

state_read
f=$((f + 1))
level=$l

log "connectivity DEGRADED: $FAILS/$TOTAL probes failed (majority threshold: $MAJORITY_NEEDED/$TOTAL); consecutive failed rounds: $f/$FAIL_THRESHOLD; current repair level: $level"

if [ "$f" -lt "$FAIL_THRESHOLD" ]; then
    state_write "$f" "$level"
    exit 0
fi

if [ -e "$DISABLE_FILE" ]; then
    log "repairs DISABLED via $DISABLE_FILE — logging only"
    state_write 0 "$level"   # hold at threshold; do not accumulate further
    exit 0
fi

log "PERSISTENT outage confirmed ($f consecutive failed rounds) — beginning staged repair (level $level)"

repair_stage "$level"

if [ "$level" -lt 3 ]; then
    next_level=$((level + 1))
else
    next_level=3
fi
state_write 0 "$next_level"

if wait_recovery 6; then
    log "RECOVERED after repair level $level — connectivity restored"
    state_clear
    exit 0
fi

# After a stack-level repair allow a longer settling window before giving up.
if [ "$level" -ge 2 ]; then
    if wait_recovery 6; then
        log "RECOVERED after repair level $level (extended window) — connectivity restored"
        state_clear
        exit 0
    fi
fi

log "repair level $level did NOT restore connectivity; will escalate to level $next_level on the next sustained failure episode"
exit 1
