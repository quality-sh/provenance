# host-reliability — mint

Diagnosis + artifacts for two reliability gaps on `mint`. Nothing here touches the
live machine on its own; `install.sh` (or the manual commands in the final report)
applies it.

## Findings (live network stack, 2026-09-15)

| Question | Answer | Evidence |
|---|---|---|
| Network stack | **NetworkManager** (+ wpa_supplicant, systemd-resolved) | `systemctl is-active/enabled`: NetworkManager active+enabled; systemd-networkd **inactive+disabled**; wpa_supplicant active+enabled |
| Wifi interface / profile | `wlp6s0`, profile **`Auto 丹丹`** (uuid `9b764221-463e-4441-b664-05ed98daa025`), autoconnect already `yes` | `nmcli -t device status`; `nmcli -g connection.autoconnect connection show "Auto 丹丹"` |
| Default route | `default via 192.168.4.1 dev wlp6s0 proto dhcp` | `ip route show` |
| Tailscale dependency | Node `mint` = `100.89.46.40`; only connectivity path is wifi (wired `enp0s31f6` is DOWN/NO-CARRIER) | `tailscale ip`; `ip -br addr` |
| Root cause of "link never came back" | During the upstream outage (Sep 15, 09:16–22:30) the supplicant disconnected, NM's re-activation **failed with `reason 'no-secrets'`**, the device fell to `disconnected` and NM's autoconnect retry budget was exhausted — it sat scanning/disconnected for ~13h until manual intervention at 22:30 | `journalctl -u NetworkManager`: 09:16:29 "disconnected during association", 09:19:47 "state change: need-auth -> failed (reason 'no-secrets')", 09:19:50 "Activation: failed for connection 'Auto 丹丹'", … 22:30:37 "disconnected -> prepare" (manual) |
| Existing watchdogs | None (dispatcher.d empty, no crontab, no watch units) | checked |
| opencode2-server | systemd **user** unit, `ExecStart=opencode2 serve --hostname 127.0.0.1 --port 4097`, `Restart=on-failure`; creds live in `~/.config/opencode/server.env` (keys `OPENCODE_SERVER_USERNAME`/`OPENCODE_SERVER_PASSWORD`, unquoted) | unit file + env file |

Because the stack is NetworkManager, recovery = periodic system-level probe unit
(dispatcher.d is event-driven and empty here, so a timer fits better), plus
`connection.autoconnect yes` enforced as part of install.

## Artifacts

```
network/net-watch.sh                      staged wifi repair watchdog (POSIX sh, root)
network/net-watch.service                 oneshot system unit (hardened)
network/net-watch.timer                   every 45s (within 30–60s window)
opencode2/opencode2-provider-watch.sh     /api/provider probe + self-heal restart
opencode2/opencode2-provider-watch.service  user oneshot unit
opencode2/opencode2-provider-watch.timer    every 15 min (*:0/15, persistent)
install.sh                                idempotent installer for both
```

## What the watchdogs do

**net-watch** (system, every 45s): probes `{gateway, 1.1.1.1, 8.8.8.8, 100.100.100.100}`;
acts only after **3 consecutive majority-failed rounds** (~2.5 min sustained outage).
Then escalates one level per sustained episode: (1) `nmcli connection up <profile>`
(DHCP renew) → (2) `ip link down/up` + re-activate → (3) `systemctl try-restart
wpa_supplicant` + `NetworkManager`. Recovery requires 2 consecutive healthy rounds
before escalation stops. Every action is logged to the journal under
`net-watch.service`. **Never reboots.** Kill switch: `touch /run/net-watch.disabled`.
Test: `DRY_RUN=1 /usr/local/sbin/net-watch.sh`.

**opencode2-provider-watch** (user, every 15 min): reads basic-auth creds at
runtime from `~/.config/opencode/server.env` (grep/cut — never sourced, never
logged, absent from unit files), curls `http://127.0.0.1:4097/api/provider`; if
missing/erroring or lacking `zai-coding-plan`, runs
`systemctl --user restart opencode2-server.service` (with a 300s cooldown against
restart storms) and logs it. Deps: curl + grep/coreutils only.
Test: `DRY_RUN=1 OPENCODE_PROVIDER_URL=http://127.0.0.1:49999/api/provider ~/.local/bin/opencode2-provider-watch.sh`

## Install

```
cd host-reliability && sudo ./install.sh
```

## Risks / assumptions

- Timer-level probing means worst-case detection-to-repair ≈ 2.5–4 min of true
  outage (3 failed rounds at 45s cadence) — deliberate, to never act on a blip.
- If the AP itself is down, all three repair stages run (harmless retries), one
  per ~3 min episode; stage 3 restarts NM which momentarily drops docker0/externally
  managed devices from NM's view but does not touch tailscaled (userspace, own tun).
- Stage 2/3 will briefly drop SSH-over-LAN if run while wifi is the only path —
  but they only run when the path is already dead.
- `install.sh` assumes the invoking sudo user is `ben`'s session (user units need
  `SUDO_USER` or a plain `ben` shell).
- Provider watch assumes `/api/provider` returning 200 + containing the string
  `zai-coding-plan` equals "healthy" (verified against the live endpoint).
