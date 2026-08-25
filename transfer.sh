#!/usr/bin/env bash
# Transfer awgram controller state to a dedicated VPS without moving the VPN.
set -euo pipefail
STATE=/var/lib/awgram; CFG=/etc/awgram; OUT=/root/awgram-transfer.enc; TMP=""
trap '[ -n "$TMP" ] && rm -rf "$TMP"' EXIT
die(){ printf 'ERR %s\n' "$*" >&2; exit 1; }
root(){ [ "$(id -u)" = 0 ] || die 'запустите через sudo'; }
need(){ command -v "$1" >/dev/null 2>&1 || die "не найдена команда: $1"; }

install_node_bridge(){
  local pub="$1" forced
  [ -x /root/awg/manage_amneziawg.sh ] || die 'не найден /root/awg/manage_amneziawg.sh'
  install -d -m700 /etc/awgram-node /root/.ssh /usr/local/libexec
  cat >/usr/local/libexec/awgram-nodectl <<'NODE'
#!/usr/bin/env bash
set -euo pipefail
set -- ${SSH_ORIGINAL_COMMAND:-${*:-status}}; a="${1:-status}"; n="${2:-}"; e="${3:-}"
valid(){ [[ "$1" =~ ^[A-Za-z0-9_][A-Za-z0-9_-]{0,63}$ ]]; }
case "$a" in
 status) systemctl is-active --quiet awg-quick@awg0 && printf '{"ok":true,"status":"online"}\n' || { printf '{"ok":false,"status":"offline"}\n'; exit 1; };;
 add) valid "$n"||exit 2; bash /root/awg/manage_amneziawg.sh add "$n">/dev/null; systemctl restart awg-quick@awg0; c=/root/awg/$n.conf; [ -f "$c" ]||exit 4; printf '{"ok":true,"name":"%s","conf_b64":"%s","qr_b64":"%s"}\n' "$n" "$(base64 -w0<"$c")" "$([ -f /root/awg/$n.png ]&&base64 -w0</root/awg/$n.png||true)";;
 remove) valid "$n"||exit 2; bash /root/awg/manage_amneziawg.sh remove "$n">/dev/null||true; systemctl restart awg-quick@awg0; printf '{"ok":true}\n';;
 set-expiry) valid "$n"||exit 2; [[ "$e" =~ ^[0-9]+$ ]]||exit 2; install -d -m700 /etc/awgram-node/expiry; printf '%s\n' "$e">"/etc/awgram-node/expiry/$n"; printf '{"ok":true}\n';;
 *) exit 2;; esac
NODE
  chmod 700 /usr/local/libexec/awgram-nodectl
  forced="command=\"/usr/local/libexec/awgram-nodectl\",no-agent-forwarding,no-port-forwarding,no-pty,no-user-rc,no-X11-forwarding $pub"
  touch /root/.ssh/authorized_keys; chmod 600 /root/.ssh/authorized_keys
  grep -Fqx "$forced" /root/.ssh/authorized_keys||printf '%s\n' "$forced">>/root/.ssh/authorized_keys
  touch /etc/awgram-node/ready
}

export_data(){
  root; need openssl; need ssh-keygen; need tar
  command -v sqlite3 >/dev/null 2>&1||{ apt-get update -qq&&apt-get install -y -qq sqlite3; }
  [ -s "$STATE/awgram.db" ]||die 'база awgram не найдена'; [ -s "$CFG/env" ]||die 'токен не найден'
  TMP="$(mktemp -d)"; install -d -m700 "$TMP/payload"
  [ -s "$STATE/node_id_ed25519" ]||ssh-keygen -q -t ed25519 -N '' -f "$STATE/node_id_ed25519"
  install_node_bridge "$(cat "$STATE/node_id_ed25519.pub")"
  sqlite3 "$STATE/awgram.db" ".backup '$TMP/payload/awgram.db'"
  sqlite3 "$TMP/payload/awgram.db" "UPDATE vpn_servers SET is_local=0,status='online' WHERE is_local=1;"
  cp -a "$CFG/env" "$CFG/config.toml" "$STATE/node_id_ed25519" "$STATE/node_id_ed25519.pub" "$TMP/payload/"
  [ -f "$STATE/node_known_hosts" ]&&cp -a "$STATE/node_known_hosts" "$TMP/payload/"
  [ -d "$STATE/clients" ]&&cp -a "$STATE/clients" "$TMP/payload/"
  sqlite3 "$STATE/awgram.db" "SELECT public_ip FROM vpn_servers WHERE is_local=1 LIMIT 1;">"$TMP/payload/source-ip"
  [ -s "$TMP/payload/source-ip" ]||hostname -I|awk '{print $1}'>"$TMP/payload/source-ip"
  tar -C "$TMP/payload" -czf "$TMP/payload.tar.gz" .
  printf 'Введите новый пароль архива (он не сохраняется):\n' >&2
  openssl enc -aes-256-cbc -pbkdf2 -salt -in "$TMP/payload.tar.gz" -out "$OUT"
  chmod 600 "$OUT"; printf 'OK Создан %s\nСкопируйте файл на новую VPS. Старый бот пока продолжает работать.\n' "$OUT"
}

import_data(){
  root; need openssl; need tar; need curl; need ssh-keyscan
  local archive="${1:-$OUT}" admins token source_ip
  [ -s "$archive" ]||die "архив не найден: $archive"
  printf 'Перед импортом остановите бот на СТАРОЙ VPS: systemctl stop awgram\nПродолжить импорт? Введите IMPORT: ' >&2
  local confirmation=""; read -r confirmation
  [ "$confirmation" = IMPORT ]||die 'импорт отменён'
  TMP="$(mktemp -d)"; install -d -m700 "$TMP/payload"
  openssl enc -d -aes-256-cbc -pbkdf2 -in "$archive" -out "$TMP/payload.tar.gz"
  tar -xzf "$TMP/payload.tar.gz" -C "$TMP/payload"
  if [ ! -s "$TMP/payload/awgram.db" ] || [ ! -s "$TMP/payload/env" ]; then
    die 'архив повреждён'
  fi
  admins="$(sed -n 's/^admin_ids[[:space:]]*=[[:space:]]*\[\(.*\)\].*/\1/p' "$TMP/payload/config.toml"|tr -d ' ')"
  token="$(sed -n 's/^AWGRAM_TOKEN=//p' "$TMP/payload/env")"
  if [ -z "$admins" ] || [ -z "$token" ]; then die 'нет token/admin_ids'; fi
  curl -fsSL https://github.com/stevefoxru/awgram/releases/latest/download/install.sh -o "$TMP/install.sh"
  bash "$TMP/install.sh" install --yes --no-systemd --mode hardened --controller-only --token "$token" --admins "$admins"
  systemctl stop awgram
  install -m640 "$TMP/payload/awgram.db" "$STATE/awgram.db"; install -m600 "$TMP/payload/env" "$CFG/env"
  install -m600 "$TMP/payload/node_id_ed25519" "$STATE/node_id_ed25519"; install -m644 "$TMP/payload/node_id_ed25519.pub" "$STATE/node_id_ed25519.pub"
  [ -f "$TMP/payload/node_known_hosts" ]&&install -m600 "$TMP/payload/node_known_hosts" "$STATE/node_known_hosts"
  [ -d "$TMP/payload/clients" ]&&cp -a "$TMP/payload/clients/." "$STATE/clients/"
  source_ip="$(cat "$TMP/payload/source-ip")"; touch "$STATE/node_known_hosts"
  ssh-keyscan -H "$source_ip" >>"$STATE/node_known_hosts" 2>/dev/null||die 'старая VPS недоступна по SSH'
  sort -u "$STATE/node_known_hosts" -o "$STATE/node_known_hosts"; chown -R awgram:awgram "$STATE"
  systemctl daemon-reload; systemctl enable --now awgram; sleep 4
  systemctl is-active --quiet awgram||die 'бот не запустился: journalctl -u awgram -n 50'
  sudo -u awgram ssh -i "$STATE/node_id_ed25519" -o BatchMode=yes -o StrictHostKeyChecking=yes -o "UserKnownHostsFile=$STATE/node_known_hosts" "root@$source_ip" status >/dev/null||die 'бот работает, но VPN-узел недоступен'
  printf 'OK Перенос завершён. На старой VPS отключите автозапуск: systemctl disable awgram\n'
}

case "${1:-}" in export) export_data;; import) import_data "${2:-$OUT}";; *) die 'usage: transfer.sh export | transfer.sh import [archive]';; esac
