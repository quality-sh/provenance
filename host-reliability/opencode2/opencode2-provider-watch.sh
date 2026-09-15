#!/bin/sh
# opencode2-provider-watch.sh — self-heal watchdog for opencode2-server (port 4097).
#
# Reads basic-auth credentials at RUNTIME from ~/.config/opencode/server.env
# (they never appear in any unit file or in the journal). Probes
# http://127.0.0.1:4097/api/provider; if the response is missing, erroring, or
# lacks "zai-coding-plan", restarts the opencode2-server.service user unit.
#
# Test mode: DRY_RUN=1 logs the intended restart instead of executing it.
# Optional overrides: OPENCODE_PROVIDER_URL, OPENCODE_PROVIDER_UNIT,
#                     OPENCODE_PROVIDER_MARKER, OPENCODE_PROVIDER_COOLDOWN.

set -u

ENV_FILE="$HOME/.config/opencode/server.env"
URL="${OPENCODE_PROVIDER_URL:-http://127.0.0.1:4097/api/provider}"
UNIT="${OPENCODE_PROVIDER_UNIT:-opencode2-server.service}"
: "${XDG_RUNTIME_DIR:=$HOME}"
MARKER="${OPENCODE_PROVIDER_MARKER:-$XDG_RUNTIME_DIR/opencode2-provider-watch.last-restart}"
COOLDOWN="${OPENCODE_PROVIDER_COOLDOWN:-300}"   # seconds; skip restart if one just happened
DRY_RUN="${DRY_RUN:-0}"

log() { printf 'opencode2-provider-watch: %s\n' "$*"; }

[ -r "$ENV_FILE" ] || { log "FATAL: env file not readable: $ENV_FILE"; exit 1; }

# Extract credentials at runtime (grep/cut only — never sourced, never logged).
USER_NAME=$(grep -E '^OPENCODE_SERVER_USERNAME=' "$ENV_FILE" | head -n1 | cut -d= -f2-)
PASS_WORD=$(grep -E '^OPENCODE_SERVER_PASSWORD=' "$ENV_FILE" | head -n1 | cut -d= -f2-)
if [ -z "$USER_NAME" ] || [ -z "$PASS_WORD" ]; then
    log "FATAL: OPENCODE_SERVER_USERNAME/PASSWORD missing from $ENV_FILE — refusing to restart"
    exit 1
fi

BODY=$(curl -fsS --connect-timeout 5 --max-time 20 -u "$USER_NAME:$PASS_WORD" "$URL" 2>/dev/null)
RC=$?

if [ "$RC" -eq 0 ] && printf '%s' "$BODY" | grep -q 'zai-coding-plan'; then
    log "provider view OK (zai-coding-plan present)"
    exit 0
fi

reason="curl rc=$RC (response missing/unreachable)"
if [ "$RC" -eq 0 ]; then
    reason="response lacks zai-coding-plan"
fi
log "provider view UNHEALTHY ($reason)"

# Cooldown: if a restart was performed very recently, give the server time to
# come up and repopulate its provider view before restarting again.
now=$(date +%s)
if [ -f "$MARKER" ]; then
    last=$(cat "$MARKER" 2>/dev/null)
    case "${last:-}" in ''|*[!0-9]*) last=0 ;; esac
    if [ "$((now - last))" -lt "$COOLDOWN" ]; then
        log "restart was performed $((now - last))s ago (cooldown ${COOLDOWN}s) — skipping restart this cycle"
        exit 0
    fi
fi

if [ "$DRY_RUN" = "1" ]; then
    log "DRY-RUN: would run: systemctl --user restart $UNIT (trigger: $reason)"
    exit 0
fi

log "restarting $UNIT (trigger: $reason)"
if systemctl --user restart "$UNIT"; then
    printf '%s\n' "$now" > "$MARKER" 2>/dev/null
    log "restart of $UNIT completed"
else
    log "ERROR: systemctl --user restart $UNIT failed"
    exit 1
fi
