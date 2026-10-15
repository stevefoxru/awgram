#!/bin/sh
set -eu
action=${1:-help}
root=/var/lib/zpnet-routing
backup="$root/backups/$(date -u +%Y%m%dT%H%M%SZ)"
case "$action" in
  preflight)
    command -v ip >/dev/null; command -v wg >/dev/null; command -v python3 >/dev/null
    test -r /etc/zpnet-routing/config.json
    /usr/local/libexec/zpnet-routing-agent dry-run
    ;;
  install-ru)
    test "$(id -u)" = 0
    install -d -m 700 /etc/zpnet-routing "$root/backups"
    install -m 755 "$(dirname "$0")/zpnet-routing-agent.py" /usr/local/libexec/zpnet-routing-agent
    test -e /etc/zpnet-routing/config.json || install -m 600 "$(dirname "$0")/config.example.json" /etc/zpnet-routing/config.json
    printf '%s\n' '[Unit]' 'Description=ZPNet routing reconciliation' 'After=network-online.target' '' '[Service]' 'Type=oneshot' 'ExecStart=/usr/local/libexec/zpnet-routing-agent reconcile' > /etc/systemd/system/zpnet-routing.service
    printf '%s\n' '[Unit]' 'Description=Run ZPNet routing health checks' '' '[Timer]' 'OnBootSec=30s' 'OnUnitActiveSec=30s' 'AccuracySec=2s' '' '[Install]' 'WantedBy=timers.target' > /etc/systemd/system/zpnet-routing.timer
    systemctl daemon-reload
    echo 'Installed but NOT activated. Edit config, run: install.sh preflight, then install.sh activate --confirm'
    ;;
  install-exit)
    test "$(id -u)" = 0; test -n "${2:-}"; test -n "${3:-}"
    install -m 755 "$(dirname "$0")/zpnet-exit-health.py" /usr/local/libexec/zpnet-exit-health
    printf '%s\n' '[Unit]' 'Description=ZPNet real egress health endpoint' 'After=network-online.target' '' '[Service]' 'Environment=ZPNET_EXPECTED_PUBLIC_IP='"$2" 'Environment=ZPNET_HEALTH_BIND='"$3" 'ExecStart=/usr/local/libexec/zpnet-exit-health' 'Restart=on-failure' '' '[Install]' 'WantedBy=multi-user.target' > /etc/systemd/system/zpnet-exit-health.service
    systemctl daemon-reload; systemctl enable --now zpnet-exit-health.service
    ;;
  activate)
    test "${2:-}" = --confirm || { echo 'explicit --confirm is required'; exit 64; }
    mkdir -p "$backup"; ip rule show > "$backup/ip-rule.txt"; ip route show table 100 > "$backup/table-100.txt"; ip route show table 200 > "$backup/table-200.txt"
    systemctl is-enabled zpnet-failover.timer > "$backup/old-watchdog-enabled" 2>/dev/null || true
    /usr/local/libexec/zpnet-routing-agent dry-run >/dev/null
    systemctl disable --now zpnet-failover.timer 2>/dev/null || true
    systemctl enable --now zpnet-routing.timer; systemctl start zpnet-routing.service
    echo "Activated. Backup: $backup"
    ;;
  rollback)
    systemctl disable --now zpnet-routing.timer 2>/dev/null || true
    /usr/local/libexec/zpnet-routing-agent rollback
    echo 'Routing state rolled back; restore a legacy watchdog manually only after verifying ownership.'
    ;;
  *) echo 'usage: install.sh {install-ru|install-exit PUBLIC_IP TUNNEL_IP|preflight|activate --confirm|rollback}'; exit 64;;
esac
