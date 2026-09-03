#!/bin/sh
set -eu

DOMAIN=""
UPSTREAM="127.0.0.1:8787"
CONFIG="/etc/awgram/config.toml"
EXPECTED_IP=""

usage() { echo "Usage: sudo sh portal-domain.sh DOMAIN [--expected-ip IPv4] [--upstream HOST:PORT] [--config PATH]"; }
while [ "$#" -gt 0 ]; do
  case "$1" in
    --upstream) UPSTREAM=${2:?missing upstream}; shift 2 ;;
    --config) CONFIG=${2:?missing config path}; shift 2 ;;
    --expected-ip) EXPECTED_IP=${2:?missing IPv4}; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) [ -z "$DOMAIN" ] || { echo "Unknown argument: $1" >&2; exit 2; }; DOMAIN=$1; shift ;;
  esac
done

[ "$(id -u)" -eq 0 ] || { echo "Run as root (sudo)." >&2; exit 1; }
echo "$DOMAIN" | grep -Eq '^[A-Za-z0-9]([A-Za-z0-9.-]*[A-Za-z0-9])?$' || { echo "Invalid domain." >&2; exit 2; }
echo "$UPSTREAM" | grep -Eq '^[A-Za-z0-9.:-]+$' || { echo "Invalid upstream." >&2; exit 2; }
[ -f "$CONFIG" ] || { echo "AWGram config not found: $CONFIG" >&2; exit 1; }

resolved=$(getent ahostsv4 "$DOMAIN" 2>/dev/null | awk '{print $1}' | sort -u | tr '\n' ' ')
[ -n "$resolved" ] || { echo "DNS A record for $DOMAIN is not available yet." >&2; exit 1; }
echo "DNS $DOMAIN -> $resolved"
if [ -z "$EXPECTED_IP" ] && command -v curl >/dev/null 2>&1; then
  EXPECTED_IP=$(curl -4fsS --max-time 8 https://api.ipify.org 2>/dev/null || true)
fi
if [ -n "$EXPECTED_IP" ] && ! printf '%s\n' "$resolved" | tr ' ' '\n' | grep -Fxq "$EXPECTED_IP"; then
  echo "DNS mismatch: $DOMAIN does not point to this server ($EXPECTED_IP)." >&2
  echo "Change the A record and run the installer again after DNS propagation." >&2
  exit 1
fi

if ! command -v caddy >/dev/null 2>&1; then
  command -v apt-get >/dev/null 2>&1 || { echo "Install Caddy manually, then run this script again." >&2; exit 1; }
  apt-get update
  DEBIAN_FRONTEND=noninteractive apt-get install -y caddy
fi

install -d -m 0755 /etc/caddy/Caddyfile.d
fragment="/etc/caddy/Caddyfile.d/awgram.caddy"
tmp=$(mktemp)
config_tmp=$(mktemp)
trap 'rm -f "$tmp" "$config_tmp"' EXIT
{
  echo "$DOMAIN {"
  echo "    encode zstd gzip"
  echo "    reverse_proxy $UPSTREAM"
  echo "    header {"
  echo "        Strict-Transport-Security \"max-age=31536000; includeSubDomains\""
  echo "        -Server"
  echo "    }"
  echo "}"
} > "$tmp"
install -m 0644 "$tmp" "$fragment"

main_caddy="/etc/caddy/Caddyfile"
touch "$main_caddy"
if ! grep -Fq 'import /etc/caddy/Caddyfile.d/*.caddy' "$main_caddy"; then
  cp -a "$main_caddy" "$main_caddy.awgram-backup-$(date +%Y%m%d%H%M%S)"
  printf '\nimport /etc/caddy/Caddyfile.d/*.caddy\n' >> "$main_caddy"
fi
caddy validate --config "$main_caddy" --adapter caddyfile

awk -v url="https://$DOMAIN" '
  BEGIN { done=0 }
  /^[[:space:]]*portal_public_url[[:space:]]*=/ { print "portal_public_url = \"" url "\""; done=1; next }
  { print }
  END { if (!done) print "portal_public_url = \"" url "\"" }
' "$CONFIG" > "$config_tmp"
cp -a "$CONFIG" "$CONFIG.awgram-backup-$(date +%Y%m%d%H%M%S)"
install -m 0640 "$config_tmp" "$CONFIG"

systemctl enable --now caddy
systemctl reload caddy
systemctl restart awgram
sleep 2
systemctl is-active --quiet awgram
systemctl is-active --quiet caddy
echo "Configured: https://$DOMAIN -> http://$UPSTREAM"
echo "If HTTPS is not ready, verify DNS and that inbound ports 80/443 are open."
