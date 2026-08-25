#!/usr/bin/env bash
set -euo pipefail
SERVER_ID=""; NODE_ID=""; PROTOCOL=""; CONTROLLER_KEY_B64=""; NODE_SECRET_B64=""; AWG_PORT=39743; AWG_SUBNET="10.10.10.1/24"
COMMIT=b9c8ea0464dfa955892f0b136804822a5906963c
INSTALL_SHA=6f345dcc7553dcc8b595d1e828fc5c010c8a96f110999b0a39a8944ddc1b7566
MANAGE_SHA=4381e847d625712ac52527257069bd646c65b643a7e738588e7dfebcde0384c0
die(){ printf 'ERR %s\n' "$*" >&2; exit 1; }
while (($#)); do case "$1" in
 --server-id) SERVER_ID="${2:-}";shift 2;; --node-id) NODE_ID="${2:-}";shift 2;; --protocol) PROTOCOL="${2:-}";shift 2;;
 --controller-key-b64) CONTROLLER_KEY_B64="${2:-}";shift 2;; --port) AWG_PORT="${2:-}";shift 2;;
 --node-secret-b64) NODE_SECRET_B64="${2:-}";shift 2;;
 --subnet) AWG_SUBNET="${2:-}";shift 2;; *) die "Неизвестный параметр: $1";; esac; done
[[ "$(id -u)" = 0 && "$SERVER_ID" =~ ^[1-9][0-9]*$ && "$NODE_ID" =~ ^[1-9][0-9]*$ ]] || die "Некорректные параметры"
[[ "$PROTOCOL" =~ ^(amneziawg-1|amneziawg-2)$ ]] || die "Поддерживаются только AmneziaWG 1.0 и 2.0"
key="$(printf '%s' "$CONTROLLER_KEY_B64"|base64 -d 2>/dev/null)" || die "Повреждён ключ"
[[ "$key" = ssh-ed25519\ * ]] || die "Неверный ключ контроллера"
secret="$(printf '%s' "$NODE_SECRET_B64"|base64 -d 2>/dev/null)" || die "Повреждён секрет узла"
[[ ${#secret} -ge 32 ]] || die "Секрет узла слишком короткий"
install -d -m 700 /etc/awgram-node /var/lib/awgram-node/nonces /root/.ssh /root/awg /usr/local/libexec
printf '%s' "$secret">/etc/awgram-node/agent.secret
printf 'SERVER_ID=%s\nNODE_ID=%s\nPROTOCOL=%s\nSECRET_FILE=/etc/awgram-node/agent.secret\nDRIVER_COMMAND=/usr/local/libexec/awgram-driver\n' "$SERVER_ID" "$NODE_ID" "$PROTOCOL">/etc/awgram-node/node.conf
printf "export AWG_PORT=%s\nexport AWG_TUNNEL_SUBNET='%s'\nexport DISABLE_IPV6=1\nexport ALLOWED_IPS_MODE=1\nexport ALLOWED_IPS='0.0.0.0/0'\n" "$AWG_PORT" "$AWG_SUBNET">/root/awg/awgsetup_cfg.init
chmod 600 /etc/awgram-node/node.conf /root/awg/awgsetup_cfg.init
arch="$(uname -m)"; case "$arch" in x86_64|amd64) arch=amd64;; aarch64|arm64) arch=arm64;; *) die "Архитектура не поддерживается: $arch";; esac
base="https://github.com/stevefoxru/awgram/releases/latest/download"
curl -fLsS "$base/awgram-node-linux-$arch" -o "/tmp/awgram-node-linux-$arch"
curl -fLsS "$base/awgram-node-linux-$arch.sha256" -o "/tmp/awgram-node-linux-$arch.sha256"
(cd /tmp&&sha256sum -c "awgram-node-linux-$arch.sha256") || die "Контрольная сумма агента не совпала"
install -m700 "/tmp/awgram-node-linux-$arch" /usr/local/libexec/awgram-node
rm -f "/tmp/awgram-node-linux-$arch" "/tmp/awgram-node-linux-$arch.sha256"
cat >/usr/local/libexec/awgram-driver <<'NODECTL'
#!/usr/bin/env bash
set -euo pipefail
set -- ${SSH_ORIGINAL_COMMAND:-${*:-status}}; a="${1:-status}"; n="${2:-}"; e="${3:-}"
valid(){ [[ "$1" =~ ^[A-Za-z0-9_][A-Za-z0-9_-]{0,63}$ ]]; }
case "$a" in
 status) [[ -f /etc/awgram-node/ready ]]&&systemctl is-active --quiet awg-quick@awg0&&printf '{"ok":true,"status":"online"}\n'||printf '{"ok":false,"status":"installing"}\n';;
 list) bash /root/awg/manage_amneziawg.sh list --json;;
 diagnose) printf 'host=%s\n' "$(hostname -f 2>/dev/null||hostname)"; printf 'uptime=%s\n' "$(uptime -p 2>/dev/null||true)"; printf 'service=%s\n' "$(systemctl is-active awg-quick@awg0 2>/dev/null||true)"; printf 'enabled=%s\n' "$(systemctl is-enabled awg-quick@awg0 2>/dev/null||true)"; ip -brief address show awg0 2>/dev/null||true; wg show awg0 2>/dev/null||awg show awg0 2>/dev/null||true; ss -lunp 2>/dev/null|grep -E ':39743\\b' || true;;
 add) valid "$n"||exit 2; [[ -f /etc/awgram-node/ready ]]||exit 3; bash /root/awg/manage_amneziawg.sh add "$n">/dev/null; systemctl restart awg-quick@awg0; c=/root/awg/$n.conf; [[ -f "$c" ]]||exit 4; printf '{"ok":true,"name":"%s","conf_b64":"%s","qr_b64":"%s"}\n' "$n" "$(base64 -w0<"$c")" "$([[ -f /root/awg/$n.png ]]&&base64 -w0</root/awg/$n.png||true)";;
 get) valid "$n"||exit 2; c=/root/awg/$n.conf; [[ -f "$c" ]]||exit 4; printf '{"ok":true,"name":"%s","conf_b64":"%s","qr_b64":"%s"}\n' "$n" "$(base64 -w0<"$c")" "$([[ -f /root/awg/$n.png ]]&&base64 -w0</root/awg/$n.png||true)";;
 regen) valid "$n"||exit 2; bash /root/awg/manage_amneziawg.sh regen "$n" --json >/dev/null; c=/root/awg/$n.conf; [[ -f "$c" ]]||exit 4; printf '{"ok":true,"name":"%s","conf_b64":"%s","qr_b64":"%s"}\n' "$n" "$(base64 -w0<"$c")" "$([[ -f /root/awg/$n.png ]]&&base64 -w0</root/awg/$n.png||true)";;
 remove) valid "$n"||exit 2; bash /root/awg/manage_amneziawg.sh remove "$n">/dev/null||true; systemctl restart awg-quick@awg0; printf '{"ok":true}\n';;
 enable) valid "$n"||exit 2; bash /root/awg/manage_amneziawg.sh enable "$n">/dev/null; systemctl restart awg-quick@awg0; printf '{"ok":true}\n';;
 disable) valid "$n"||exit 2; bash /root/awg/manage_amneziawg.sh disable "$n">/dev/null; systemctl restart awg-quick@awg0; printf '{"ok":true}\n';;
 set-expiry) valid "$n"||exit 2; [[ "$e" =~ ^[0-9]+$ ]]||exit 2; install -d -m700 /etc/awgram-node/expiry; printf '%s\n' "$e">"/etc/awgram-node/expiry/$n"; printf '{"ok":true}\n';;
 migrate-preflight) exec /usr/local/libexec/awgram-migratectl preflight;;
 migrate-start) exec /usr/local/libexec/awgram-migratectl start;;
 migrate-status) exec /usr/local/libexec/awgram-migratectl status;;
 migrate-rollback) exec /usr/local/libexec/awgram-migratectl rollback;;
 enforce) now=$(date +%s); for f in /etc/awgram-node/expiry/*; do [[ -f "$f" ]]||continue; read -r until<"$f"; if [[ "$until" =~ ^[0-9]+$ ]]&&((until<=now)); then x=${f##*/}; bash /root/awg/manage_amneziawg.sh remove "$x">/dev/null||true; rm -f "$f"; fi; done; systemctl restart awg-quick@awg0; printf '{"ok":true}\n';;
 *) exit 2;; esac
NODECTL
chmod 700 /usr/local/libexec/awgram-driver
forced="command=\"/usr/local/libexec/awgram-node\",no-agent-forwarding,no-port-forwarding,no-pty,no-user-rc,no-X11-forwarding $key"
touch /root/.ssh/authorized_keys;chmod 600 /root/.ssh/authorized_keys;grep -Fqx "$forced" /root/.ssh/authorized_keys||printf '%s\n' "$forced">>/root/.ssh/authorized_keys
if [[ "$PROTOCOL" != amneziawg-1 ]]; then
  printf 'OK агент установлен; установка протокола %s будет запущена контроллером\n' "$PROTOCOL"
  exit 0
fi
cat >/usr/local/libexec/awgram-node-install <<RUNNER
#!/usr/bin/env bash
set -euo pipefail
b=https://raw.githubusercontent.com/bivlked/amneziawg-installer/$COMMIT
curl -fLsS "\$b/install_amneziawg.sh" -o /root/install_amneziawg-v1.sh
echo '$INSTALL_SHA  /root/install_amneziawg-v1.sh'|sha256sum -c -
curl -fLsS "\$b/manage_amneziawg.sh" -o /root/awg/manage_amneziawg.legacy
echo '$MANAGE_SHA  /root/awg/manage_amneziawg.legacy'|sha256sum -c -
chmod 700 /root/install_amneziawg-v1.sh /root/awg/manage_amneziawg.legacy
sed -i "s#https://raw.githubusercontent.com/bivlked/amneziawg-installer/main/manage_amneziawg.sh#\$b/manage_amneziawg.sh#" /root/install_amneziawg-v1.sh
sed -i 's#read -p "Перезагрузить сейчас? \[y/N\]: " confirm < /dev/tty#confirm=y#' /root/install_amneziawg-v1.sh
bash /root/install_amneziawg-v1.sh --port=$AWG_PORT --subnet=$AWG_SUBNET --disallow-ipv6 --route-all --no-color
install -m700 /root/awg/manage_amneziawg.legacy /root/awg/manage_amneziawg.sh
systemctl restart awg-quick@awg0;touch /etc/awgram-node/ready;systemctl disable awgram-node-install.service
RUNNER
chmod 700 /usr/local/libexec/awgram-node-install
cat >/etc/systemd/system/awgram-node-install.service <<'UNIT'
[Unit]
Description=awgram automatic AmneziaWG installation
After=network-online.target
Wants=network-online.target
[Service]
Type=oneshot
ExecStart=/usr/local/libexec/awgram-node-install
TimeoutStartSec=infinity
[Install]
WantedBy=multi-user.target
UNIT
systemctl daemon-reload;systemctl enable awgram-node-install.service
printf '*/5 * * * * root /usr/local/libexec/awgram-driver enforce >/dev/null 2>&1\n'>/etc/cron.d/awgram-node-expiry
systemd-run --quiet --collect --unit=awgram-node-install-now /usr/local/libexec/awgram-node-install
printf 'OK bootstrap запущен; server-id=%s\n' "$SERVER_ID"
