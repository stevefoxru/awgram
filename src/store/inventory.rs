use std::collections::{HashMap, HashSet};

use crate::store::Store;

#[derive(Debug, Clone)]
pub struct InventoryItem {
    pub remote_id: String,
    pub name: String,
    pub enabled: bool,
    pub rx: u64,
    pub tx: u64,
    pub last_handshake: Option<i64>,
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
}
