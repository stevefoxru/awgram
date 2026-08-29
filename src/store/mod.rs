//! Единственный владелец SQLite-БД бота. Все таблицы, миграции схемы и
//! доступ к соединению живут здесь; наружу — типизированные методы
//! (settings/stats/events в соседних файлах модуля).

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

mod broadcasts;
mod commerce;
mod events;
mod groups;
mod inventory;
mod nodes;
mod portal;
mod server_enrollment;
mod servers;
mod settings;
mod stars;
mod stats;

pub use broadcasts::BroadcastRun;
pub use commerce::{
    AdminUserProfile, AdminUserStats, FinanceSummary, KeyReplacement, LegacyRequest, MonitorEvent,
    MonitorState, PaymentRequest, PaymentStatus, PromoCode, SupportTicket, UserRow,
};
pub use events::{EventKind, EventRow};
pub use groups::{
    gen_invite_token, GroupError, GroupRow, InviteRow, InviteUse, ListScope, QuotaAssign,
    INVITE_TTL_SECS,
};
pub use inventory::{InventoryItem, InventoryReport, KeyRuntimeStats, ServerRuntimeSummary};
pub use nodes::{InstallationJob, VpnInstance, VpnNode};
pub use portal::{PortalKey, PortalOverview, PortalPayment};
pub use server_enrollment::{EnrollmentIssue, EnrollmentStatus, ENROLLMENT_TTL_SECS};
pub use servers::{NewVpnServer, ServerBillingUpdate, VpnServer};
pub use stars::{NewStarOrder, StarOrder, StarPaymentClaim};
pub use stats::{PeriodTotals, Sample, TrafficSummary};

/// SQL-батчи миграций: индекс в массиве + 1 == schema_version после применения.
/// Только добавлять в конец — существующие батчи менять нельзя (уже применены
/// на живых установках).
pub(crate) const MIGRATIONS: &[&str] = &[
    // v1: базовая схема. meta — IF NOT EXISTS: её создаёт migrate() ещё до
    // применения батчей (нужна, чтобы прочитать schema_version).
    r#"
    CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE clients(
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL UNIQUE,
        ip TEXT NOT NULL DEFAULT '',
        first_seen INTEGER NOT NULL,
        last_seen INTEGER NOT NULL,
        removed_at INTEGER
    );
    CREATE TABLE traffic_samples(
        client_id INTEGER NOT NULL REFERENCES clients(id),
        ts INTEGER NOT NULL,
        rx INTEGER NOT NULL,
        tx INTEGER NOT NULL,
        rx_delta INTEGER NOT NULL,
        tx_delta INTEGER NOT NULL,
        online INTEGER NOT NULL
    );
    CREATE INDEX idx_samples_client_ts ON traffic_samples(client_id, ts);
    CREATE INDEX idx_samples_ts ON traffic_samples(ts);
    CREATE TABLE traffic_hourly(
        client_id INTEGER NOT NULL REFERENCES clients(id),
        hour_ts INTEGER NOT NULL,
        rx_bytes INTEGER NOT NULL,
        tx_bytes INTEGER NOT NULL,
        online_minutes INTEGER NOT NULL,
        PRIMARY KEY(client_id, hour_ts)
    );
    CREATE TABLE traffic_daily(
        client_id INTEGER NOT NULL REFERENCES clients(id),
        day_ts INTEGER NOT NULL,
        rx_bytes INTEGER NOT NULL,
        tx_bytes INTEGER NOT NULL,
        online_minutes INTEGER NOT NULL,
        PRIMARY KEY(client_id, day_ts)
    );
    CREATE TABLE events(
        id INTEGER PRIMARY KEY,
        ts INTEGER NOT NULL,
        kind TEXT NOT NULL,
        client TEXT,
        actor INTEGER,
        details TEXT
    );
    CREATE INDEX idx_events_ts ON events(ts);
    CREATE INDEX idx_events_client ON events(client, ts);
    "#,
    // v2: группы и делегирование (issue #20). Принадлежность клиента к группе —
    // только в БД; manage_amneziawg.sh про группы не знает.
    r#"
    CREATE TABLE groups(
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL UNIQUE,
        max_clients INTEGER,
        created_at INTEGER NOT NULL
    );
    CREATE TABLE group_admins(
        group_id INTEGER NOT NULL REFERENCES groups(id),
        user_id INTEGER NOT NULL,
        added_at INTEGER NOT NULL,
        added_by INTEGER NOT NULL,
        PRIMARY KEY(group_id, user_id)
    );
    CREATE TABLE invites(
        token TEXT PRIMARY KEY,
        group_id INTEGER NOT NULL REFERENCES groups(id),
        created_by INTEGER NOT NULL,
        created_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL,
        used_by INTEGER,
        used_at INTEGER
    );
    ALTER TABLE clients ADD COLUMN group_id INTEGER REFERENCES groups(id);
    "#,
    // v3: обычные пользователи, владение ключами и ручные платежи.
    r#"
    CREATE TABLE users(
        user_id INTEGER PRIMARY KEY,
        username TEXT,
        display_name TEXT NOT NULL DEFAULT '',
        referrer_id INTEGER REFERENCES users(user_id),
        created_at INTEGER NOT NULL,
        last_seen INTEGER NOT NULL
    );
    ALTER TABLE clients ADD COLUMN owner_user_id INTEGER REFERENCES users(user_id);
    CREATE INDEX idx_clients_owner ON clients(owner_user_id, removed_at);
    CREATE TABLE balance_ledger(
        id INTEGER PRIMARY KEY,
        user_id INTEGER NOT NULL REFERENCES users(user_id),
        amount_kopecks INTEGER NOT NULL,
        kind TEXT NOT NULL,
        reference TEXT NOT NULL UNIQUE,
        details TEXT,
        created_at INTEGER NOT NULL
    );
    CREATE INDEX idx_ledger_user ON balance_ledger(user_id, created_at);
    CREATE TABLE payment_requests(
        id INTEGER PRIMARY KEY,
        user_id INTEGER NOT NULL REFERENCES users(user_id),
        months INTEGER NOT NULL,
        amount_kopecks INTEGER NOT NULL,
        method TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'pending',
        proof TEXT,
        client_name TEXT,
        created_at INTEGER NOT NULL,
        decided_at INTEGER,
        decided_by INTEGER
    );
    CREATE INDEX idx_payments_status ON payment_requests(status, created_at);
    "#,
    // v4: пробный доступ, напоминания об окончании и обращения в поддержку.
    r#"
    ALTER TABLE users ADD COLUMN trial_claimed_at INTEGER;
    CREATE TABLE expiry_notifications(
        client_name TEXT NOT NULL,
        owner_user_id INTEGER NOT NULL REFERENCES users(user_id),
        expires_at INTEGER NOT NULL,
        threshold_days INTEGER NOT NULL,
        sent_at INTEGER NOT NULL,
        PRIMARY KEY(client_name, expires_at, threshold_days)
    );
    CREATE TABLE support_tickets(
        id INTEGER PRIMARY KEY,
        user_id INTEGER NOT NULL REFERENCES users(user_id),
        status TEXT NOT NULL DEFAULT 'open',
        subject TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        closed_at INTEGER,
        closed_by INTEGER
    );
    CREATE INDEX idx_support_status ON support_tickets(status, updated_at);
    CREATE TABLE broadcasts(
        id INTEGER PRIMARY KEY,
        admin_id INTEGER NOT NULL,
        source_chat_id INTEGER NOT NULL,
        source_message_id INTEGER NOT NULL,
        delivered INTEGER NOT NULL DEFAULT 0,
        failed INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL
    );
    "#,
    // v5: настройки подписок и защита автоматического продления от дублей.
    r#"
    CREATE TABLE client_subscriptions(
        client_name TEXT PRIMARY KEY,
        user_id INTEGER NOT NULL REFERENCES users(user_id),
        months INTEGER NOT NULL DEFAULT 1,
        auto_renew INTEGER NOT NULL DEFAULT 0,
        updated_at INTEGER NOT NULL
    );
    CREATE TABLE renewal_attempts(
        client_name TEXT NOT NULL,
        expires_at INTEGER NOT NULL,
        status TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        PRIMARY KEY(client_name, expires_at)
    );
    "#,
    // v6: понятные названия устройств и управляемые тикеты поддержки.
    r#"
    ALTER TABLE clients ADD COLUMN device_label TEXT;
    ALTER TABLE support_tickets ADD COLUMN assigned_to INTEGER;
    CREATE TABLE support_messages(
        id INTEGER PRIMARY KEY,
        ticket_id INTEGER NOT NULL REFERENCES support_tickets(id),
        sender_user_id INTEGER NOT NULL,
        is_admin INTEGER NOT NULL,
        telegram_chat_id INTEGER NOT NULL,
        telegram_message_id INTEGER NOT NULL,
        text TEXT,
        created_at INTEGER NOT NULL
    );
    CREATE INDEX idx_support_messages_ticket ON support_messages(ticket_id,created_at);
    "#,
    // v7: ограниченные роли сотрудников.
    r#"
    CREATE TABLE staff_roles(
        user_id INTEGER PRIMARY KEY,
        role TEXT NOT NULL CHECK(role IN ('technical','support','finance')),
        granted_by INTEGER NOT NULL,
        granted_at INTEGER NOT NULL
    );
    "#,
    // v8: состояние эксплуатационного мониторинга.
    r#"
    CREATE TABLE monitor_state(
        component TEXT PRIMARY KEY,
        status TEXT NOT NULL,
        details TEXT,
        changed_at INTEGER NOT NULL,
        checked_at INTEGER NOT NULL
    );
    "#,
    // v9: CRM-карточки, расширенные подписки, поддержка, промокоды и рассылки.
    r#"
    ALTER TABLE users ADD COLUMN blocked INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE users ADD COLUMN admin_note TEXT;
    ALTER TABLE users ADD COLUMN promo_discount INTEGER;
    ALTER TABLE clients ADD COLUMN admin_note TEXT;
    ALTER TABLE payment_requests ADD COLUMN reject_reason TEXT;
    ALTER TABLE client_subscriptions ADD COLUMN grace_until INTEGER;
    ALTER TABLE client_subscriptions ADD COLUMN frozen_until INTEGER;
    ALTER TABLE support_tickets ADD COLUMN category TEXT NOT NULL DEFAULT 'general';
    ALTER TABLE support_tickets ADD COLUMN priority TEXT NOT NULL DEFAULT 'normal';
    ALTER TABLE support_tickets ADD COLUMN rating INTEGER;
    ALTER TABLE broadcasts ADD COLUMN audience TEXT NOT NULL DEFAULT 'all';
    ALTER TABLE broadcasts ADD COLUMN scheduled_at INTEGER;
    ALTER TABLE broadcasts ADD COLUMN button_text TEXT;
    ALTER TABLE broadcasts ADD COLUMN button_url TEXT;
    CREATE TABLE promo_codes(
        code TEXT PRIMARY KEY COLLATE NOCASE,
        discount_percent INTEGER NOT NULL CHECK(discount_percent BETWEEN 1 AND 100),
        max_uses INTEGER,
        used_count INTEGER NOT NULL DEFAULT 0,
        expires_at INTEGER,
        active INTEGER NOT NULL DEFAULT 1,
        created_by INTEGER NOT NULL,
        created_at INTEGER NOT NULL
    );
    CREATE TABLE promo_uses(
        code TEXT NOT NULL REFERENCES promo_codes(code),
        user_id INTEGER NOT NULL REFERENCES users(user_id),
        payment_id INTEGER REFERENCES payment_requests(id),
        used_at INTEGER NOT NULL,
        PRIMARY KEY(code,user_id)
    );
    CREATE INDEX idx_users_created ON users(created_at);
    CREATE INDEX idx_subscriptions_grace ON client_subscriptions(grace_until);
    "#,
    // v10: бесплатное восстановление legacy-ключей и отдельное ежегодное
    // продление до конца следующего календарного года.
    r#"
    ALTER TABLE promo_codes ADD COLUMN kind TEXT NOT NULL DEFAULT 'discount';
    ALTER TABLE client_subscriptions ADD COLUMN legacy INTEGER NOT NULL DEFAULT 0;
    CREATE TABLE legacy_entitlements(
        id INTEGER PRIMARY KEY,
        user_id INTEGER NOT NULL REFERENCES users(user_id),
        promo_code TEXT NOT NULL REFERENCES promo_codes(code),
        activated_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL,
        used_client TEXT,
        used_at INTEGER,
        UNIQUE(user_id,promo_code)
    );
    CREATE TABLE legacy_requests(
        id INTEGER PRIMARY KEY,
        user_id INTEGER NOT NULL REFERENCES users(user_id),
        requested_name TEXT NOT NULL,
        comment TEXT,
        status TEXT NOT NULL DEFAULT 'pending',
        created_at INTEGER NOT NULL,
        decided_at INTEGER,
        decided_by INTEGER,
        client_name TEXT,
        reject_reason TEXT
    );
    CREATE INDEX idx_legacy_requests_status ON legacy_requests(status,created_at);
    CREATE INDEX idx_subscriptions_legacy ON client_subscriptions(legacy,user_id);
    "#,
    // v11: защита самостоятельного обновления клиентской конфигурации от спама.
    r#"
    CREATE TABLE client_self_refreshes(
        client_name TEXT PRIMARY KEY,
        user_id INTEGER NOT NULL REFERENCES users(user_id),
        last_requested_at INTEGER NOT NULL,
        request_count INTEGER NOT NULL DEFAULT 1
    );
    CREATE INDEX idx_client_self_refreshes_user ON client_self_refreshes(user_id,last_requested_at);
    "#,
    // v12: реестр VPN-серверов, паспорта VPS и календарь оплаты.
    r#"
    CREATE TABLE vpn_servers(
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL UNIQUE,
        hostname TEXT NOT NULL,
        public_ip TEXT NOT NULL,
        provider TEXT NOT NULL,
        location TEXT NOT NULL,
        protocol TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'unknown',
        enabled_for_provisioning INTEGER NOT NULL DEFAULT 0,
        opened_at INTEGER,
        added_at INTEGER NOT NULL,
        paid_until INTEGER,
        billing_period_months INTEGER,
        cost_minor INTEGER,
        currency TEXT,
        auto_renew INTEGER NOT NULL DEFAULT 0,
        panel_url TEXT,
        order_ref TEXT,
        note TEXT,
        is_local INTEGER NOT NULL DEFAULT 0,
        created_by INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE TABLE server_payment_events(
        id INTEGER PRIMARY KEY,
        server_id INTEGER NOT NULL REFERENCES vpn_servers(id) ON DELETE CASCADE,
        paid_at INTEGER NOT NULL,
        paid_until INTEGER NOT NULL,
        amount_minor INTEGER,
        currency TEXT,
        actor_id INTEGER NOT NULL,
        note TEXT
    );
    CREATE TABLE server_billing_notifications(
        server_id INTEGER NOT NULL REFERENCES vpn_servers(id) ON DELETE CASCADE,
        paid_until INTEGER NOT NULL,
        threshold_days INTEGER NOT NULL,
        sent_at INTEGER NOT NULL,
        PRIMARY KEY(server_id,paid_until,threshold_days)
    );
    CREATE INDEX idx_vpn_servers_paid_until ON vpn_servers(paid_until,status);
    "#,
    // v13: одноразовые bootstrap-приглашения для удалённых VPN-узлов.
    // В БД хранится только SHA-256 токена; исходный секрет показывается один раз.
    r#"
    CREATE TABLE server_enrollments(
        id INTEGER PRIMARY KEY,
        server_id INTEGER NOT NULL REFERENCES vpn_servers(id) ON DELETE CASCADE,
        token_hash TEXT NOT NULL UNIQUE,
        created_by INTEGER NOT NULL,
        created_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL,
        used_at INTEGER,
        revoked_at INTEGER
    );
    CREATE INDEX idx_server_enrollments_server ON server_enrollments(server_id,expires_at);
    "#,
    // v14: размещение ключей, ёмкость узлов, мультипротокольные адаптеры и
    // постоянные индивидуальные скидки. Старые ключи привязываются к локальному
    // узлу при его регистрации, поэтому перевыпуск конфигураций не требуется.
    r#"
    ALTER TABLE vpn_servers ADD COLUMN capacity INTEGER NOT NULL DEFAULT 150;
    ALTER TABLE clients ADD COLUMN server_id INTEGER REFERENCES vpn_servers(id);
    ALTER TABLE clients ADD COLUMN protocol TEXT NOT NULL DEFAULT 'amneziawg-2';
    ALTER TABLE payment_requests ADD COLUMN server_id INTEGER REFERENCES vpn_servers(id);
    ALTER TABLE users ADD COLUMN personal_discount INTEGER;
    ALTER TABLE users ADD COLUMN personal_discount_until INTEGER;
    CREATE TABLE purchase_preferences(
        user_id INTEGER PRIMARY KEY REFERENCES users(user_id),
        server_id INTEGER NOT NULL REFERENCES vpn_servers(id),
        updated_at INTEGER NOT NULL
    );
    CREATE INDEX idx_clients_server ON clients(server_id,removed_at);
    CREATE INDEX idx_payments_server ON payment_requests(server_id,status);
    "#,
    // v15: Telegram Stars orders. Charge id is unique, making successful
    // payment handling idempotent even if Telegram retries an update.
    r#"
    CREATE TABLE star_orders(
        id INTEGER PRIMARY KEY,
        user_id INTEGER NOT NULL REFERENCES users(user_id),
        kind TEXT NOT NULL CHECK(kind IN ('purchase','renew')),
        months INTEGER NOT NULL,
        stars INTEGER NOT NULL CHECK(stars > 0),
        client_name TEXT,
        server_id INTEGER REFERENCES vpn_servers(id),
        status TEXT NOT NULL DEFAULT 'pending',
        telegram_charge_id TEXT UNIQUE,
        created_at INTEGER NOT NULL,
        paid_at INTEGER,
        fulfilled_at INTEGER,
        failure TEXT
    );
    CREATE INDEX idx_star_orders_user ON star_orders(user_id,created_at);
    CREATE INDEX idx_star_orders_status ON star_orders(status,created_at);
    "#,
    // v16: двухфазная пользовательская замена ключа.
    r#"
    CREATE TABLE key_replacements(
        id INTEGER PRIMARY KEY,
        user_id INTEGER NOT NULL REFERENCES users(user_id),
        old_client TEXT NOT NULL,
        new_client TEXT NOT NULL,
        target_server_id INTEGER NOT NULL REFERENCES vpn_servers(id),
        status TEXT NOT NULL DEFAULT 'pending',
        created_at INTEGER NOT NULL,
        decided_at INTEGER
    );
    CREATE UNIQUE INDEX idx_key_replacements_pending_old
      ON key_replacements(old_client) WHERE status='pending';
    CREATE INDEX idx_key_replacements_expiry ON key_replacements(status,created_at);
    "#,
    // v17: физические узлы отделены от установленных VPN-инстансов. Это
    // позволяет контроллеру управлять несколькими протоколами на одном VPS и
    // больше не считать vpn_servers реализацией конкретного VPN-драйвера.
    r#"
    CREATE TABLE vpn_nodes(
        id INTEGER PRIMARY KEY,
        server_id INTEGER NOT NULL UNIQUE REFERENCES vpn_servers(id) ON DELETE CASCADE,
        transport TEXT NOT NULL CHECK(transport IN ('local','restricted_ssh','signed_ssh','https_agent','panel_api')),
        status TEXT NOT NULL DEFAULT 'unknown',
        agent_version TEXT,
        public_key_fingerprint TEXT,
        endpoint TEXT,
        last_seen_at INTEGER,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE TABLE vpn_instances(
        id INTEGER PRIMARY KEY,
        node_id INTEGER NOT NULL REFERENCES vpn_nodes(id) ON DELETE CASCADE,
        server_id INTEGER NOT NULL REFERENCES vpn_servers(id) ON DELETE CASCADE,
        protocol TEXT NOT NULL,
        driver TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'unknown',
        is_default INTEGER NOT NULL DEFAULT 0,
        config_json TEXT NOT NULL DEFAULT '{}',
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE UNIQUE INDEX idx_vpn_instances_driver ON vpn_instances(node_id,driver);
    CREATE UNIQUE INDEX idx_vpn_instances_default ON vpn_instances(server_id) WHERE is_default=1;
    CREATE TABLE installation_jobs(
        id INTEGER PRIMARY KEY,
        server_id INTEGER NOT NULL REFERENCES vpn_servers(id) ON DELETE CASCADE,
        node_id INTEGER REFERENCES vpn_nodes(id) ON DELETE SET NULL,
        protocol TEXT NOT NULL,
        operation TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'pending',
        stage TEXT NOT NULL DEFAULT 'created',
        progress INTEGER NOT NULL DEFAULT 0 CHECK(progress BETWEEN 0 AND 100),
        error_code TEXT,
        log_excerpt TEXT,
        backup_ref TEXT,
        requested_by INTEGER NOT NULL,
        created_at INTEGER NOT NULL,
        started_at INTEGER,
        finished_at INTEGER
    );
    CREATE INDEX idx_installation_jobs_server ON installation_jobs(server_id,created_at DESC);
    ALTER TABLE clients ADD COLUMN instance_id INTEGER REFERENCES vpn_instances(id);
    INSERT INTO vpn_nodes(server_id,transport,status,created_at,updated_at)
      SELECT id,
             CASE WHEN is_local=1 THEN 'local'
                  WHEN protocol='amneziawg-panel' THEN 'panel_api'
                  ELSE 'restricted_ssh' END,
             status,added_at,updated_at
      FROM vpn_servers;
    INSERT INTO vpn_instances(node_id,server_id,protocol,driver,status,is_default,created_at,updated_at)
      SELECT n.id,s.id,s.protocol,s.protocol,s.status,1,s.added_at,s.updated_at
      FROM vpn_servers s JOIN vpn_nodes n ON n.server_id=s.id;
    UPDATE clients
       SET instance_id=(SELECT i.id FROM vpn_instances i
                        WHERE i.server_id=clients.server_id AND i.is_default=1)
     WHERE server_id IS NOT NULL;
    CREATE INDEX idx_clients_instance ON clients(instance_id,removed_at);
    "#,
    // v18: продукт намеренно ограничен семейством AmneziaWG. Устаревшие
    // паспорта других протоколов сохраняются для аудита, но автоматически
    // исключаются из выдачи без удаления ключей или истории.
    r#"
    UPDATE vpn_servers
       SET enabled_for_provisioning=0,status='maintenance',updated_at=strftime('%s','now')
     WHERE protocol NOT IN ('modern','legacy','amneziawg-2','amneziawg-1','amneziawg-panel');
    UPDATE vpn_instances
       SET status='unsupported',updated_at=strftime('%s','now')
     WHERE protocol NOT IN ('modern','legacy','amneziawg-2','amneziawg-1','amneziawg-panel');
    "#,
    // v19: короткоживущие одноразовые ссылки и серверные сессии внутреннего ЛК.
    // В БД хранятся только SHA-256 отпечатки секретов.
    r#"
    CREATE TABLE web_sessions(
        token_hash TEXT PRIMARY KEY,
        user_id INTEGER NOT NULL REFERENCES users(user_id),
        created_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL,
        activated_at INTEGER,
        revoked_at INTEGER
    );
    CREATE INDEX idx_web_sessions_user ON web_sessions(user_id,expires_at);
    "#,
    // v20: фактический инвентарь ключей на каждой панели. Реестр отделён от
    // таблицы clients, чтобы видеть сироты и одинаковые имена на разных VPS.
    r#"
    CREATE TABLE key_inventory(
        server_id INTEGER NOT NULL REFERENCES vpn_servers(id) ON DELETE CASCADE,
        remote_id TEXT NOT NULL,
        name TEXT NOT NULL,
        enabled INTEGER NOT NULL DEFAULT 1,
        rx INTEGER NOT NULL DEFAULT 0,
        tx INTEGER NOT NULL DEFAULT 0,
        last_handshake INTEGER,
        first_seen_at INTEGER NOT NULL,
        last_seen_at INTEGER NOT NULL,
        missing_since INTEGER,
        PRIMARY KEY(server_id,remote_id)
    );
    CREATE INDEX idx_key_inventory_name ON key_inventory(name,missing_since);
    CREATE TABLE client_archive_events(
        id INTEGER PRIMARY KEY,
        client_name TEXT NOT NULL,
        server_id INTEGER REFERENCES vpn_servers(id),
        owner_user_id INTEGER,
        reason TEXT NOT NULL,
        actor_id INTEGER,
        archived_at INTEGER NOT NULL
    );
    CREATE INDEX idx_client_archive_name ON client_archive_events(client_name,archived_at DESC);
    "#,
    // v21: адресный журнал доставки и повтор только неуспешных сообщений.
    r#"
    CREATE TABLE broadcast_deliveries(
        broadcast_id INTEGER NOT NULL REFERENCES broadcasts(id) ON DELETE CASCADE,
        user_id INTEGER NOT NULL REFERENCES users(user_id),
        status TEXT NOT NULL CHECK(status IN ('pending','delivered','failed')),
        attempts INTEGER NOT NULL DEFAULT 0,
        last_error TEXT,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY(broadcast_id,user_id)
    );
    CREATE INDEX idx_broadcast_delivery_status ON broadcast_deliveries(broadcast_id,status);
    "#,
    // v22: неизменяемая история переходов эксплуатационного мониторинга.
    r#"
    CREATE TABLE monitor_events(
        id INTEGER PRIMARY KEY,
        component TEXT NOT NULL,
        previous_status TEXT,
        status TEXT NOT NULL,
        details TEXT,
        created_at INTEGER NOT NULL,
        acknowledged_at INTEGER
    );
    CREATE INDEX idx_monitor_events_created ON monitor_events(created_at DESC);
    CREATE INDEX idx_monitor_events_ack ON monitor_events(acknowledged_at,status);
    "#,
    // v23: управляемое плановое обслуживание без потери прежнего режима выдачи.
    r#"
    CREATE TABLE server_maintenance(
        server_id INTEGER PRIMARY KEY REFERENCES vpn_servers(id) ON DELETE CASCADE,
        previous_provisioning INTEGER NOT NULL,
        started_at INTEGER NOT NULL,
        started_by INTEGER NOT NULL
    );
    "#,
    // v24: адресная доставка начала и завершения плановых работ.
    r#"
    CREATE TABLE maintenance_notifications(
        server_id INTEGER NOT NULL REFERENCES vpn_servers(id) ON DELETE CASCADE,
        started_at INTEGER NOT NULL,
        user_id INTEGER NOT NULL REFERENCES users(user_id),
        start_delivered INTEGER NOT NULL DEFAULT 0,
        finish_delivered INTEGER NOT NULL DEFAULT 0,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY(server_id,started_at,user_id)
    );
    CREATE INDEX idx_maintenance_notifications_finish
        ON maintenance_notifications(server_id,started_at,start_delivered,finish_delivered);
    "#,
];

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store, rusqlite::Error> {
        if let Some(parent) = path.parent() {
            // В проде каталог создаёт install.sh; здесь — на случай dev-запуска.
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        migrate(&conn)?;
        Ok(Store {
            conn: Mutex::new(conn),
        })
    }

    /// БД в памяти — для тестов store и соседних модулей.
    pub fn open_in_memory() -> Store {
        let conn = Connection::open_in_memory().expect("in-memory sqlite");
        migrate(&conn).expect("миграции на пустой БД не могут падать");
        Store {
            conn: Mutex::new(conn),
        }
    }

    pub fn schema_version(&self) -> i64 {
        self.with_conn(|c| {
            c.query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |r| r.get::<_, String>(0),
            )
        })
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
    }

    pub(crate) fn with_conn<T>(
        &self,
        f: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T> {
        let conn = self.conn.lock().unwrap();
        f(&conn)
    }
}

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT NOT NULL)",
    )?;
    let current: i64 = conn
        .query_row(
            "SELECT value FROM meta WHERE key='schema_version'",
            [],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    for (i, batch) in MIGRATIONS.iter().enumerate() {
        let version = (i + 1) as i64;
        if version > current {
            conn.execute_batch(batch)?;
            conn.execute(
                "INSERT INTO meta(key,value) VALUES('schema_version',?1)
                 ON CONFLICT(key) DO UPDATE SET value=?1",
                [version.to_string()],
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_creates_schema_and_version() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("sub/awgram.db")).unwrap();
        assert_eq!(store.schema_version(), MIGRATIONS.len() as i64);
    }

    #[test]
    fn reopen_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("awgram.db");
        drop(Store::open(&path).unwrap());
        let store = Store::open(&path).unwrap();
        assert_eq!(store.schema_version(), MIGRATIONS.len() as i64);
    }

    #[test]
    fn in_memory_store_works() {
        let store = Store::open_in_memory();
        assert_eq!(store.schema_version(), MIGRATIONS.len() as i64);
    }
}
