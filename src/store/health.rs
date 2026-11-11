use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::Serialize;

use super::MIGRATIONS;

#[derive(Debug, Serialize)]
pub struct ForeignKeyViolation {
    pub table: String,
    pub row_id: Option<i64>,
    pub parent: String,
    pub constraint_index: i64,
}

#[derive(Debug, Serialize)]
pub struct DatabaseAudit {
    pub path: String,
    pub schema_version: Option<i64>,
    pub supported_schema_version: i64,
    pub schema_supported: bool,
    pub quick_check_ok: bool,
    pub quick_check_messages: Vec<String>,
    pub foreign_key_violations: Vec<ForeignKeyViolation>,
    pub anomaly_counts: BTreeMap<String, i64>,
    pub operation_counts: BTreeMap<String, i64>,
}

impl DatabaseAudit {
    pub fn is_healthy(&self) -> bool {
        self.schema_supported
            && self.quick_check_ok
            && self.foreign_key_violations.is_empty()
            && self.anomaly_counts.values().all(|count| *count == 0)
    }
}

fn table_exists(connection: &Connection, table: &str) -> rusqlite::Result<bool> {
    connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |row| row.get(0),
    )
}

fn column_exists(connection: &Connection, table: &str, column: &str) -> rusqlite::Result<bool> {
    if !table_exists(connection, table)? {
        return Ok(false);
    }
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for value in columns {
        if value? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn count_if_supported(
    connection: &Connection,
    output: &mut BTreeMap<String, i64>,
    name: &str,
    required_tables: &[&str],
    required_columns: &[(&str, &str)],
    sql: &str,
) -> rusqlite::Result<()> {
    for table in required_tables {
        if !table_exists(connection, table)? {
            return Ok(());
        }
    }
    for (table, column) in required_columns {
        if !column_exists(connection, table, column)? {
            return Ok(());
        }
    }
    let count = connection.query_row(sql, [], |row| row.get(0))?;
    output.insert(name.into(), count);
    Ok(())
}

pub fn audit_database(path: &Path) -> rusqlite::Result<DatabaseAudit> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;

    let schema_version = if table_exists(&connection, "meta")? {
        connection
            .query_row(
                "SELECT value FROM meta WHERE key='schema_version'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|value| {
                value.parse::<i64>().map_err(|_| {
                    rusqlite::Error::InvalidParameterName(format!(
                        "invalid schema_version value {value:?}"
                    ))
                })
            })
            .transpose()?
    } else {
        None
    };
    let supported_schema_version = MIGRATIONS.len() as i64;
    let schema_supported = schema_version == Some(supported_schema_version);

    let mut statement = connection.prepare("PRAGMA quick_check")?;
    let quick_check_messages = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let quick_check_ok = quick_check_messages.as_slice() == ["ok"];

    let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
    let foreign_key_violations = statement
        .query_map([], |row| {
            Ok(ForeignKeyViolation {
                table: row.get(0)?,
                row_id: row.get(1)?,
                parent: row.get(2)?,
                constraint_index: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut anomaly_counts = BTreeMap::new();
    count_if_supported(
        &connection,
        &mut anomaly_counts,
        "clients_without_user",
        &["clients", "users"],
        &[("clients", "owner_user_id")],
        "SELECT COUNT(*) FROM clients c
         WHERE c.owner_user_id IS NOT NULL
           AND NOT EXISTS(SELECT 1 FROM users u WHERE u.user_id=c.owner_user_id)",
    )?;
    count_if_supported(
        &connection,
        &mut anomaly_counts,
        "clients_without_server",
        &["clients", "vpn_servers"],
        &[("clients", "server_id")],
        "SELECT COUNT(*) FROM clients c
         WHERE c.server_id IS NOT NULL
           AND NOT EXISTS(SELECT 1 FROM vpn_servers s WHERE s.id=c.server_id)",
    )?;
    count_if_supported(
        &connection,
        &mut anomaly_counts,
        "subscriptions_without_client",
        &["client_subscriptions", "clients"],
        &[],
        "SELECT COUNT(*) FROM client_subscriptions s
         WHERE NOT EXISTS(SELECT 1 FROM clients c WHERE c.name=s.client_name)",
    )?;
    count_if_supported(
        &connection,
        &mut anomaly_counts,
        "subscriptions_without_user",
        &["client_subscriptions", "users"],
        &[],
        "SELECT COUNT(*) FROM client_subscriptions s
         WHERE NOT EXISTS(SELECT 1 FROM users u WHERE u.user_id=s.user_id)",
    )?;
    count_if_supported(
        &connection,
        &mut anomaly_counts,
        "payments_without_user",
        &["payment_requests", "users"],
        &[],
        "SELECT COUNT(*) FROM payment_requests p
         WHERE NOT EXISTS(SELECT 1 FROM users u WHERE u.user_id=p.user_id)",
    )?;
    count_if_supported(
        &connection,
        &mut anomaly_counts,
        "ledger_without_user",
        &["balance_ledger", "users"],
        &[],
        "SELECT COUNT(*) FROM balance_ledger l
         WHERE NOT EXISTS(SELECT 1 FROM users u WHERE u.user_id=l.user_id)",
    )?;
    count_if_supported(
        &connection,
        &mut anomaly_counts,
        "payments_with_unknown_status",
        &["payment_requests"],
        &[],
        "SELECT COUNT(*) FROM payment_requests
         WHERE status NOT IN ('pending','processing','approved','rejected')",
    )?;
    count_if_supported(
        &connection,
        &mut anomaly_counts,
        "approved_topups_without_ledger",
        &["payment_requests", "balance_ledger"],
        &[],
        "SELECT COUNT(*) FROM payment_requests p
         WHERE p.method='topup' AND p.status='approved'
           AND NOT EXISTS(
             SELECT 1 FROM balance_ledger l
             WHERE l.reference='payment:' || p.id
               AND l.user_id=p.user_id
               AND l.amount_kopecks=p.amount_kopecks
               AND l.kind='topup'
           )",
    )?;
    count_if_supported(
        &connection,
        &mut anomaly_counts,
        "nonapproved_topups_with_ledger",
        &["payment_requests", "balance_ledger"],
        &[],
        "SELECT COUNT(*) FROM payment_requests p
         WHERE p.method='topup' AND p.status<>'approved'
           AND EXISTS(
             SELECT 1 FROM balance_ledger l
             WHERE l.reference='payment:' || p.id
           )",
    )?;

    let mut operation_counts = BTreeMap::new();
    for (name, table, status) in [
        ("payments_pending", "payment_requests", "pending"),
        ("payments_processing", "payment_requests", "processing"),
        (
            "legacy_requests_processing",
            "legacy_requests",
            "processing",
        ),
        ("star_orders_paid", "star_orders", "paid"),
        ("key_replacements_pending", "key_replacements", "pending"),
        ("key_transfers_pending", "key_transfers", "pending"),
    ] {
        count_if_supported(
            &connection,
            &mut operation_counts,
            name,
            &[table],
            &[],
            &format!("SELECT COUNT(*) FROM {table} WHERE status='{status}'"),
        )?;
    }

    Ok(DatabaseAudit {
        path: path.display().to_string(),
        schema_version,
        supported_schema_version,
        schema_supported,
        quick_check_ok,
        quick_check_messages,
        foreign_key_violations,
        anomaly_counts,
        operation_counts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;

    #[test]
    fn healthy_database_passes_read_only_audit() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("awgram.db");
        drop(Store::open_migrating_for_test(&path));

        let report = audit_database(&path).unwrap();
        assert!(report.is_healthy());
        assert_eq!(report.schema_version, Some(MIGRATIONS.len() as i64));
    }

    #[test]
    fn audit_does_not_create_missing_database() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("missing.db");
        assert!(audit_database(&path).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn audit_finds_foreign_key_and_domain_orphans() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("awgram.db");
        drop(Store::open_migrating_for_test(&path));
        let connection = Connection::open(&path).unwrap();
        connection
            .pragma_update(None, "foreign_keys", false)
            .unwrap();
        connection
            .execute(
                "INSERT INTO clients(name,ip,first_seen,last_seen,owner_user_id)
                 VALUES('orphan','10.0.0.2',1,1,999)",
                [],
            )
            .unwrap();
        drop(connection);

        let report = audit_database(&path).unwrap();
        assert!(!report.is_healthy());
        assert_eq!(report.anomaly_counts["clients_without_user"], 1);
        assert!(report
            .foreign_key_violations
            .iter()
            .any(|violation| violation.table == "clients"));
    }

    #[test]
    fn audit_reports_unknown_payment_status_as_unhealthy() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("awgram.db");
        let store = Store::open_migrating_for_test(&path);
        store.upsert_user(1, Some("alice"), "Alice", None, 1);
        drop(store);
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "INSERT INTO payment_requests(
                    user_id,months,amount_kopecks,method,status,created_at
                 ) VALUES(1,1,10000,'manual','unexpected',1)",
                [],
            )
            .unwrap();
        drop(connection);

        let report = audit_database(&path).unwrap();
        assert!(!report.is_healthy());
        assert_eq!(report.anomaly_counts["payments_with_unknown_status"], 1);
    }

    #[test]
    fn audit_rejects_malformed_schema_version() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("malformed.db");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
                 INSERT INTO meta(key,value) VALUES('schema_version','broken');",
            )
            .unwrap();
        drop(connection);

        let error = audit_database(&path).unwrap_err();
        assert!(error.to_string().contains("invalid schema_version"));
    }

    #[test]
    fn audit_does_not_change_database_journal_mode() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("legacy.db");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
                 INSERT INTO meta(key,value) VALUES('schema_version','1');",
            )
            .unwrap();
        drop(connection);

        let report = audit_database(&path).unwrap();
        assert!(!report.schema_supported);
        let connection = Connection::open(&path).unwrap();
        let journal_mode: String = connection
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!(journal_mode, "delete");
    }
}
