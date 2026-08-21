#!/usr/bin/env bash
set -euo pipefail

SERVER_ID=""
TOKEN=""
PROTOCOL=""
CONFIG_DIR="/etc/awgram-node"

die() { printf 'ERR %s\n' "$*" >&2; exit 1; }

while (($#)); do
  case "$1" in
    --server-id) SERVER_ID="${2:-}"; shift 2 ;;
    --token) TOKEN="${2:-}"; shift 2 ;;
    --protocol) PROTOCOL="${2:-}"; shift 2 ;;
    *) die "Неизвестный параметр: $1" ;;
  esac
done

[[ "$SERVER_ID" =~ ^[1-9][0-9]*$ ]] || die "Некорректный server-id"
[[ "$TOKEN" =~ ^awn_[0-9a-f]{64}$ ]] || die "Некорректный или повреждённый токен"
case "$PROTOCOL" in modern|legacy) ;; *) die "Протокол должен быть modern или legacy" ;; esac
[[ "$(id -u)" = 0 ]] || die "Запустите команду через sudo"

umask 077
install -d -m 700 "$CONFIG_DIR"
printf 'SERVER_ID=%s\nPROTOCOL=%s\n' "$SERVER_ID" "$PROTOCOL" > "$CONFIG_DIR/node.conf"
printf '%s\n' "$TOKEN" > "$CONFIG_DIR/enrollment.token"
chmod 600 "$CONFIG_DIR/node.conf" "$CONFIG_DIR/enrollment.token"

printf 'OK Узел awgram подготовлен: server-id=%s, protocol=%s\n' "$SERVER_ID" "$PROTOCOL"
printf 'Токен сохранён с правами root-only и будет удалён после обмена на постоянную идентичность узла.\n'
