use std::collections::{HashMap, HashSet};

use crate::store::Store;
use rusqlite::OptionalExtension;

#[derive(Debug, Clone)]
pub struct InventoryItem {
    pub remote_id: String,
    pub name: String,
    pub enabled: bool,
    pub rx: u64,
    pub tx: u64,
    pub last_handshake: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyRuntimeStats {
    pub enabled: Option<bool>,
    pub rx: u64,
    pub tx: u64,
    pub last_handshake: Option<i64>,
    pub observed_at: i64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ServerRuntimeSummary {
    pub observed: usize,
    pub enabled: usize,
    pub online: usize,
    pub rx: u64,
    pub tx: u64,
    pub observed_at: Option<i64>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct InventoryReport {
    pub observed: usize,
    pub matched: usize,
    pub panel_only: Vec<String>,
    pub database_only: Vec<String>,
    pub wrong_server: Vec<String>,
    pub duplicates: Vec<String>,
}

impl Store {
    pub fn client_runtime_stats(&self, name: &str) -> Option<KeyRuntimeStats> {
        let inventory = self
            .with_conn(|connection| {
                connection
                    .query_row(
                        "SELECT k.enabled,k.rx,k.tx,k.last_handshake,k.last_seen_at
                           FROM key_inventory k
                           JOIN clients c ON c.server_id=k.server_id AND c.name=k.name
                          WHERE c.name=?1 AND c.removed_at IS NULL AND k.missing_since IS NULL
                          ORDER BY k.last_seen_at DESC LIMIT 1",
                        [name],
                        |row| {
                            Ok(KeyRuntimeStats {
                                enabled: Some(row.get::<_, i64>(0)? != 0),
                                rx: row.get::<_, i64>(1)?.max(0) as u64,
                                tx: row.get::<_, i64>(2)?.max(0) as u64,
                                last_handshake: row.get(3)?,
                                observed_at: row.get(4)?,
                            })
                        },
                    )
                    .optional()
            })
            .ok()
            .flatten();
        inventory.or_else(|| {
            self.with_conn(|connection| {
                connection
                    .query_row(
                        "SELECT t.rx,t.tx,
                                (SELECT MAX(ts) FROM traffic_samples online
                                  WHERE online.client_id=c.id AND online.online=1),t.ts
                           FROM clients c JOIN traffic_samples t ON t.client_id=c.id
                          WHERE c.name=?1 AND c.removed_at IS NULL
                          ORDER BY t.ts DESC LIMIT 1",
                        [name],
                        |row| {
                            Ok(KeyRuntimeStats {
                                enabled: None,
                                rx: row.get::<_, i64>(0)?.max(0) as u64,
                                tx: row.get::<_, i64>(1)?.max(0) as u64,
                                last_handshake: row.get(2)?,
                                observed_at: row.get(3)?,
                            })
                        },
                    )
                    .optional()
            })
            .ok()
            .flatten()
        })
    }

    pub fn server_runtime_summary(&self, server_id: i64, now: i64) -> ServerRuntimeSummary {
        self.with_conn(|connection| {
            connection.query_row(
                "SELECT COUNT(*),
                        COALESCE(SUM(CASE WHEN enabled<>0 THEN 1 ELSE 0 END),0),
                        COALESCE(SUM(CASE WHEN last_handshake>0 AND last_handshake>=?2 THEN 1 ELSE 0 END),0),
                        COALESCE(SUM(rx),0),COALESCE(SUM(tx),0),MAX(last_seen_at)
                   FROM key_inventory
                  WHERE server_id=?1 AND missing_since IS NULL",
                rusqlite::params![
                    server_id,
                    now.saturating_sub(crate::vpn::model::ONLINE_THRESHOLD_SECS)
                ],
                |row| {
                    Ok(ServerRuntimeSummary {
                        observed: row.get::<_, i64>(0)?.max(0) as usize,
                        enabled: row.get::<_, i64>(1)?.max(0) as usize,
                        online: row.get::<_, i64>(2)?.max(0) as usize,
                        rx: row.get::<_, i64>(3)?.max(0) as u64,
                        tx: row.get::<_, i64>(4)?.max(0) as u64,
                        observed_at: row.get(5)?,
                    })
                },
            )
        })
        .unwrap_or_default()
    }

    pub fn reconcile_inventory(
        &self,
        server_id: i64,
        now: i64,
        items: &[InventoryItem],
    ) -> InventoryReport {
        self.with_conn(|connection| {
            let transaction = connection.unchecked_transaction()?;
            transaction.execute(
                "UPDATE key_inventory SET missing_since=COALESCE(missing_since,?2)
                 WHERE server_id=?1",
                rusqlite::params![server_id, now],
            )?;
            for item in items {
                transaction.execute(
                    "INSERT INTO key_inventory(server_id,remote_id,name,enabled,rx,tx,last_handshake,first_seen_at,last_seen_at,missing_since)
                     VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8,NULL)
                     ON CONFLICT(server_id,remote_id) DO UPDATE SET
                       name=excluded.name,enabled=excluded.enabled,rx=excluded.rx,tx=excluded.tx,
                       last_handshake=excluded.last_handshake,last_seen_at=excluded.last_seen_at,missing_since=NULL",
                    rusqlite::params![server_id,item.remote_id,item.name,item.enabled as i64,
                        item.rx as i64,item.tx as i64,item.last_handshake,now],
                )?;
            }
            transaction.commit()
        })
        .unwrap_or_else(|error| tracing::error!(%error, server_id, "инвентарь панели не сохранён"));

        let panel_names = items
            .iter()
            .map(|item| item.name.clone())
            .collect::<HashSet<_>>();
        let assigned = self
            .with_conn(|connection| {
                let mut statement = connection.prepare(
                "SELECT name FROM clients WHERE server_id=?1 AND removed_at IS NULL ORDER BY name",
            )?;
                let rows = statement.query_map([server_id], |row| row.get::<_, String>(0))?;
                rows.collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap_or_default();
        let ownership = self
            .with_conn(|connection| {
                let mut statement = connection
                    .prepare("SELECT name,server_id FROM clients WHERE removed_at IS NULL")?;
                let rows = statement.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?))
                })?;
                rows.collect::<rusqlite::Result<HashMap<_, _>>>()
            })
            .unwrap_or_default();
        let panel_only = panel_names
            .iter()
            .filter(|name| !ownership.contains_key(*name))
            .cloned()
            .collect();
        let wrong_server = panel_names
            .iter()
            .filter(|name| {
                ownership
                    .get(*name)
                    .is_some_and(|saved| *saved != Some(server_id))
            })
            .cloned()
            .collect();
        let database_only = assigned
            .iter()
            .filter(|name| !panel_names.contains(*name))
            .cloned()
            .collect();
        let duplicates = self
            .with_conn(|connection| {
                let mut statement = connection.prepare(
                    "SELECT name FROM key_inventory WHERE missing_since IS NULL
                 GROUP BY name HAVING COUNT(DISTINCT server_id)>1 ORDER BY name",
                )?;
                let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
                rows.collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap_or_default();
        InventoryReport {
            observed: items.len(),
            matched: panel_names
                .iter()
                .filter(|name| ownership.get(*name) == Some(&Some(server_id)))
                .count(),
            panel_only,
            database_only,
            wrong_server,
            duplicates,
        }
    }

    pub fn archive_client_event(&self, name: &str, reason: &str, actor: Option<i64>, now: i64) {
        let _ = self.with_conn(|connection| connection.execute(
            "INSERT INTO client_archive_events(client_name,server_id,owner_user_id,reason,actor_id,archived_at)
             SELECT name,server_id,owner_user_id,?2,?3,?4 FROM clients WHERE name=?1",
            rusqlite::params![name,reason,actor,now],
        ));
    }

    pub fn archive_database_only_clients(
        &self,
        server_id: i64,
        names: &[String],
        actor: i64,
        now: i64,
    ) -> usize {
        self.with_conn(|connection| {
            let tx = connection.unchecked_transaction()?;
            let mut archived = 0usize;
            for name in names {
                tx.execute(
                    "INSERT INTO client_archive_events(client_name,server_id,owner_user_id,reason,actor_id,archived_at)
                     SELECT name,server_id,owner_user_id,'missing_from_panel',?3,?4 FROM clients
                     WHERE name=?1 AND server_id=?2 AND removed_at IS NULL",
                    rusqlite::params![name, server_id, actor, now],
                )?;
                archived += tx.execute(
                    "UPDATE clients SET removed_at=?3,last_seen=?3
                     WHERE name=?1 AND server_id=?2 AND removed_at IS NULL",
                    rusqlite::params![name, server_id, now],
                )?;
            }
            tx.commit()?;
            Ok(archived)
        })
        .unwrap_or(0)
    }

    pub fn rebind_inventory_clients(&self, server_id: i64, names: &[String], now: i64) -> usize {
        let Some(server) = self.vpn_server(server_id) else {
            return 0;
        };
        self.with_conn(|connection| {
            let tx = connection.unchecked_transaction()?;
            let mut moved = 0usize;
            for name in names {
                let assigned: i64 = tx.query_row(
                    "SELECT COUNT(*) FROM clients WHERE server_id=?1 AND removed_at IS NULL",
                    [server_id],
                    |row| row.get(0),
                )?;
                if assigned >= server.capacity {
                    break;
                }
                moved += tx.execute(
                    "UPDATE clients SET server_id=?2,protocol='amneziawg-panel',
                       instance_id=(SELECT id FROM vpn_instances WHERE server_id=?2 AND is_default=1),last_seen=?3
                     WHERE name=?1 AND removed_at IS NULL AND (server_id IS NULL OR server_id<>?2)",
                    rusqlite::params![name, server_id, now],
                )?;
            }
            tx.commit()?;
            Ok(moved)
        })
        .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::NewVpnServer;

    #[test]
    fn reconciliation_finds_panel_only_and_database_only_keys() {
        let store = Store::open_in_memory();
        let server = store
            .add_vpn_server(
                &NewVpnServer {
                    name: "NL",
                    hostname: "nl",
                    public_ip: "1.2.3.4",
                    provider: "x",
                    location: "NL",
                    protocol: "amneziawg-panel",
                    opened_at: None,
                    is_local: false,
                },
                1,
                1,
            )
            .unwrap();
        store.sync_panel_clients(server, &[("in-db".into(), "10.0.0.2".into())], 2);
        let report = store.reconcile_inventory(
            server,
            3,
            &[InventoryItem {
                remote_id: "1".into(),
                name: "only-panel".into(),
                enabled: true,
                rx: 0,
                tx: 0,
                last_handshake: None,
            }],
        );
        assert_eq!(report.panel_only, vec!["only-panel"]);
        assert_eq!(report.database_only, vec!["in-db"]);
    }

    #[test]
    fn remediation_archives_missing_and_rebinds_only_named_clients() {
        let store = Store::open_in_memory();
        let add = |name: &'static str, host: &'static str, ip: &'static str| {
            store
                .add_vpn_server(
                    &NewVpnServer {
                        name,
                        hostname: host,
                        public_ip: ip,
                        provider: "x",
                        location: "NL",
                        protocol: "amneziawg-panel",
                        opened_at: None,
                        is_local: false,
                    },
                    1,
                    1,
                )
                .unwrap()
        };
        let old = add("old", "old", "1.1.1.1");
        let actual = add("actual", "actual", "2.2.2.2");
        store.sync_panel_clients(
            old,
            &[
                ("missing".into(), "10.0.0.2".into()),
                ("moved".into(), "10.0.0.3".into()),
            ],
            2,
        );

        assert_eq!(
            store.archive_database_only_clients(old, &["missing".into()], 7, 3),
            1
        );
        assert!(!store.active_client_names().contains(&"missing".into()));
        assert_eq!(
            store.rebind_inventory_clients(actual, &["moved".into()], 4),
            1
        );
        assert_eq!(store.client_vpn_server("moved").unwrap().id, actual);
    }

    #[test]
    fn runtime_stats_expose_panel_handshake_traffic_and_freshness() {
        let store = Store::open_in_memory();
        let server = store
            .add_vpn_server(
                &NewVpnServer {
                    name: "NL",
                    hostname: "nl",
                    public_ip: "1.2.3.4",
                    provider: "x",
                    location: "NL",
                    protocol: "amneziawg-panel",
                    opened_at: None,
                    is_local: false,
                },
                1,
                1,
            )
            .unwrap();
        store.sync_panel_clients(server, &[("alice".into(), "10.0.0.2".into())], 2);
        store.reconcile_inventory(
            server,
            1_000,
            &[InventoryItem {
                remote_id: "42".into(),
                name: "alice".into(),
                enabled: true,
                rx: 1_024,
                tx: 2_048,
                last_handshake: Some(950),
            }],
        );

        assert_eq!(
            store.client_runtime_stats("alice"),
            Some(KeyRuntimeStats {
                enabled: Some(true),
                rx: 1_024,
                tx: 2_048,
                last_handshake: Some(950),
                observed_at: 1_000,
            })
        );
        assert_eq!(
            store.server_runtime_summary(server, 1_000),
            ServerRuntimeSummary {
                observed: 1,
                enabled: 1,
                online: 1,
                rx: 1_024,
                tx: 2_048,
                observed_at: Some(1_000),
            }
        );
        assert_eq!(store.server_runtime_summary(server, 2_000).online, 0);
    }
}
