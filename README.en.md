# awgram

The bot can connect an existing AmneziaWG panel (wg-easy API v14) from a
remote VPN server card: enter `URL | PASSWORD`, then choose “Synchronize”. The
password is encrypted with a separate local key. Existing bot ownership is
preserved; newly imported keys remain unassigned until an administrator links
them manually.

Plain `http://` panels are supported, but the password travels without TLS.
Restrict the panel port to the bot server IP and enable HTTPS when possible.

A new VPS can be added through the step-by-step server wizard. After entering
its name, IP, location, and provider, choose automatic AWG 1.0 installation,
connect an existing panel, or generate a one-time bootstrap command.

[🇷🇺 Русский](README.md) · 🇬🇧 English

[![CI](https://github.com/stevefoxru/awgram/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/stevefoxru/awgram/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/stevefoxru/awgram)](https://github.com/stevefoxru/awgram/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A Rust Telegram bot for managing [AmneziaWG](https://amnezia.org/) clients
straight from your phone: add/remove a client, view the list and traffic —
no SSH required.

<p align="center">
  <img src="docs/media/awgram-tg.webp" alt="awgram in Telegram" width="420">
</p>

**awgram manages native AmneziaWG** — the Linux kernel module (set up by the
[installer](https://github.com/bivlked/amneziawg-installer)) — entirely from
Telegram: once installed, no console or terminal is ever needed. Native AWG
is noticeably faster and lighter than container-based setups — especially
tangible on budget VPS hosts.

## Features

### Clients

- ➕ **Add**: expiry (1d–365d presets or custom), PSK, duplicate guard with
  recreate; you get back a `.conf`, a QR and an import link.
- 👥 **List**: three-color status (🟢 online / 🟡 no handshake / 🔴 offline)
  with last-handshake time right in the button, ↓/↑ traffic, ⏳ expiry badge;
  status filter and "online-first" sorting; client card, deletion with
  confirmation; menu/page navigation and the 🔄 "Refresh" button edit the
  message in place — no duplicate copies.
- ⚙️ **Modify client parameters**: Keepalive, DNS, AllowedIPs, Endpoint.
- 🔄 **Config re-issue**: one client or all at once (optionally with route
  reset).
- 📊 **Detailed traffic stats**: today / 7 days / 30 days, trends, top
  clients — a dedicated SQLite store, data survives reboots.
- 📜 **History** of connections and operations for every client.
- 🟢 **Honest online status**: online only when the handshake is under
  5 minutes old.
- 📦 **Bulk generation** — create up to 10 clients at once by prefix
  (`user_01 … user_99`) in a single action, continuing existing numbering
  and delivering configs in albums.
- 🎛️ **Delivery filter** — configure which artifacts (`.conf` / QR / link)
  are automatically sent after creation.
- 🧩 **Per-artifact delivery** — from the client card you can request config,
  QR, link, or all of them separately.

### Groups & delegation

- 🗂️ **Client groups**: create, rename, delete; move clients between
  groups and re-issue all configs of a group at once.
- 🤝 **Delegation**: group admins see and manage only their own group;
  assigned via a one-time invite link (24 h TTL) or by user ID.
- 📏 **Quotas**: per-group client limit — applies to group admins (the
  owner is unlimited).

### Server

- 🩺 **Check**: card with service, interface, port, module, clients and
  firewall status (✅/⚠️/❌).
- 🔬 **Environment diagnostics**.
- 🔁 **Restart service** and 🛠 **kernel module repair** (DKMS rebuild).
- 💾 **Backup/restore** of the AmneziaWG state, archive download to chat.

### Settings & security

- ⚙️ **Settings**: RU/EN language (per admin), default PSK; everything
  survives restarts (persistent state).
- 🔒 **Security**: access restricted to owners from `admin_ids` and the
  group admins they appoint, shell-free manage-script invocation, secrets
  never reach the logs, hardened mode (dedicated user + sudoers).

### Users & subscriptions

- 👤 Open registration by Telegram ID with multiple VPN keys per account.
- 💳 1/3/6/12-month plans, manual transfer approval, and automatic key delivery.
- 💰 Internal balance with top-ups and an append-only transaction ledger.
- 🎁 Referral rewards of 25% after subscription activation.
- 🔗 Assignment of existing keys by Telegram ID or `@username`.
- 🎁 One-time 24-hour trial key for a new user.
- 📅 Manual and automatic renewal of a selected key, with an automatic refund
  on technical failures.
- ⏳ 7/3/1-day reminders. Expired peers are disabled while preserving their
  config, restored after renewal, and deleted after seven days.
- 📱 Device labels and key cards with status and expiry information.
- 🎟 One-use discount promo codes with usage limits.
- ♻️ A recovery area for keys previously purchased directly from the admin:
  a `LEGACY` promo confirms eligibility, then each free replacement-key request
  is reviewed manually. New requests close automatically on December 1, 2026.
- 🔧 A separately editable annual renewal price for legacy keys through the end
  of the following calendar year, with a December reminder.
- 🆘 Categorized support tickets with priority, assignment, replies, and ratings.

### Administration & operations

- 💳 Daily/30-day financial analytics and CSV payment export.
- 👤 User and key CRM cards with balances, payments, referrals, blocking, and notes.
- 🧑‍💼 Restricted `technical`, `support`, and `finance` staff roles.
- 🔎 Button-driven search and confirmed bulk operations with previews.
- 📊 VPN, user, subscription, tariff, and finance statistics sections.
- 📣 Confirmed broadcasts to selected user audiences.
- 💾 Daily SQLite backups, five-minute VPN health monitoring, broadcasts,
  recovery alerts, and an administrative audit trail.

## Quick start

1. Get a bot token from [@BotFather](https://t.me/BotFather) (`/newbot`)
   and your numeric ID from [@userinfobot](https://t.me/userinfobot).
2. On a VPS with the
   [AmneziaWG installer](https://github.com/bivlked/amneziawg-installer) set up, run:

   ```bash
   curl -fsSL https://github.com/stevefoxru/awgram/releases/latest/download/install.sh | bash
   ```

3. Answer the installer's questions (language, root/hardened mode, token,
   admin IDs) — done: open your bot in Telegram and press `/start`.

Fully automated install — via flags:

```bash
curl -fsSL https://github.com/stevefoxru/awgram/releases/latest/download/install.sh \
  | bash -s -- install --lang en --mode root --token 'TOKEN' --admins 111111111 --yes
```

You can skip the `--token` flag (so the token never lands in `argv` or shell
history) — `export AWGRAM_TOKEN='TOKEN'` before the same command without
`--token` instead.

Post-install management: `awgram-setup update | config | status | uninstall`.

Pre-release builds — available since **v0.7.0**: `awgram-setup update
--channel rc` (the choice is sticky, the rc channel also sees stable
releases; to return run `awgram-setup update --channel stable`). If the
server's `awgram-setup` is older than v0.7.0, it has no `--channel` flag
yet — either run a plain `awgram-setup update` (it updates the script
itself too), or install an rc right away with a one-liner from the release:

```bash
curl -fsSL https://github.com/stevefoxru/awgram/releases/download/vX.Y.Z-rc.N/install.sh \
  | bash -s -- update --channel rc
```

## How it works

`awgram` is a single static binary (Rust, `teloxide`, long polling, no
webhook) living on the same VPS as the VPN. It never touches the AmneziaWG
configuration directly — it invokes the standard `manage_amneziawg.sh`
script (shell-free, with `--json`) and renders the result as an inline
Telegram menu. Access is restricted to owners from `admin_ids` and the
group admins they appoint; the token and `.conf`/QR contents never reach
the logs.

## AmneziaWG installer compatibility

The bot is a layer on top of `manage_amneziawg.sh` from
[bivlked/amneziawg-installer](https://github.com/bivlked/amneziawg-installer)
and depends directly on its interface.

To provision a separate AWG 1.0 node, create a VPS passport with the
`amneziawg-1` protocol and choose “Install VPN on VPS” in its card. The initial
password is used in process memory only; subsequent access uses a dedicated
SSH key restricted to the node management command set.

The local server can be migrated via “Admin → Servers → VPN management →
Migrate local server to AWG 1.0”. It requires Ubuntu 24.04, at least 3 GB of
free space, and the explicit `MIGRATE AWG1` confirmation. The workflow creates
a backup, survives the required reboots, and distributes replacement client
configurations after completion. Existing client configurations stop working.

- **Supported installer version:
  [v5.25.0](https://github.com/bivlked/amneziawg-installer/releases/tag/v5.25.0)**
  (`--json` contract verified). Minimum is
  [v5.21.0](https://github.com/bivlked/amneziawg-installer/releases/tag/v5.21.0);
  older v5.20.x releases are not supported — the bot uses the extended
  `--json` interface for management commands introduced in v5.21.0.
  v5.21.1–v5.25.0 keep the JSON contract intact: v5.21.1/v5.21.2 are
  validation bugfixes, v5.22.0 added an `awgsetup_cfg.init` drift warning
  to `regen`/`check` (goes to stderr, stdout JSON untouched), v5.23.0
  only changes the installers (kernel module handling on older kernels),
  v5.24.0 added an additive `module.version` field to `check --json`,
  and v5.25.0 adds only new warnings, all of which go to stderr.
- Subcommands used: `add`, `remove`, `list`, `stats`, `regen`, `modify`,
  `backup`, `restore`, `check`, `restart`, `repair-module` — all with `--json`.

## Building from source

You need a stable Rust toolchain (1.95 or newer) and `cargo`; TLS is
rustls-based, no system `libssl` required.

```bash
cargo build --release                 # target/release/awgram
./scripts/build-musl.sh [arm64|all]   # static Linux binaries in dist/ (requires Docker)
```

Releases on a `v*` tag build amd64+arm64 binaries with `sha256` checksums
automatically.

## License

[MIT](LICENSE)
