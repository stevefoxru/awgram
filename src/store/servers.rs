use rusqlite::OptionalExtension;

use crate::store::Store;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VpnServer {
    pub id: i64,
    pub name: String,
    pub hostname: String,
    pub public_ip: String,
    pub provider: String,
    pub location: String,
    pub protocol: String,
    pub status: String,
    pub enabled_for_provisioning: bool,
    pub opened_at: Option<i64>,
    pub added_at: i64,
    pub paid_until: Option<i64>,
    pub billing_period_months: Option<i64>,
    pub cost_minor: Option<i64>,
    pub currency: Option<String>,
    pub auto_renew: bool,
    pub panel_url: Option<String>,
    pub order_ref: Option<String>,
    pub note: Option<String>,
    pub is_local: bool,
    pub capacity: i64,
}

pub struct NewVpnServer<'a> {
    pub name: &'a str,
    pub hostname: &'a str,
    pub public_ip: &'a str,
    pub provider: &'a str,
    pub location: &'a str,
    pub protocol: &'a str,
    pub opened_at: Option<i64>,
    pub is_local: bool,
}

pub struct ServerBillingUpdate<'a> {
    pub paid_until: i64,
    pub period_months: i64,
    pub cost_minor: i64,
    pub currency: &'a str,
    pub auto_renew: bool,
}

fn server_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<VpnServer> {
    Ok(VpnServer {
        id: row.get(0)?,
        name: row.get(1)?,
        hostname: row.get(2)?,
        public_ip: row.get(3)?,
        provider: row.get(4)?,
        location: row.get(5)?,
        protocol: row.get(6)?,
        status: row.get(7)?,
        enabled_for_provisioning: row.get::<_, i64>(8)? != 0,
        opened_at: row.get(9)?,
        added_at: row.get(10)?,
        paid_until: row.get(11)?,
        billing_period_months: row.get(12)?,
        cost_minor: row.get(13)?,
        currency: row.get(14)?,
        auto_renew: row.get::<_, i64>(15)? != 0,
        panel_url: row.get(16)?,
        order_ref: row.get(17)?,
        note: row.get(18)?,
        is_local: row.get::<_, i64>(19)? != 0,
        capacity: row.get(20)?,
    })
}

const SERVER_COLUMNS: &str = "id,name,hostname,public_ip,provider,location,protocol,status,enabled_for_provisioning,opened_at,added_at,paid_until,billing_period_months,cost_minor,currency,auto_renew,panel_url,order_ref,note,is_local,capacity";

impl Store {
    pub fn ensure_local_vpn_server(&self, hostname: &str, actor: i64, now: i64) -> Option<i64> {
        let hostname = hostname.trim();
        if hostname.is_empty() {
            return None;
        }
        self.with_conn(|c| {
            if let Some(id) = c
                .query_row(
                    "SELECT id FROM vpn_servers WHERE is_local=1 ORDER BY id LIMIT 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?
            {
                c.execute(
                    "UPDATE clients SET server_id=?1,
                     instance_id=(SELECT id FROM vpn_instances WHERE server_id=?1 AND is_default=1)
                     WHERE server_id IS NULL AND removed_at IS NULL",
                    [id],
                )?;
                return Ok(id);
            }
            if let Some(id) = c
                .query_row(
                    "SELECT id FROM vpn_servers WHERE hostname=?1 ORDER BY id LIMIT 1",
                    [hostname],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?
            {
                c.execute(
                    "UPDATE vpn_servers SET is_local=1,enabled_for_provisioning=1,updated_at=?2 WHERE id=?1",
                    rusqlite::params![id, now],
                )?;
                c.execute(
                    "UPDATE clients SET server_id=?1,
                     instance_id=(SELECT id FROM vpn_instances WHERE server_id=?1 AND is_default=1)
                     WHERE server_id IS NULL AND removed_at IS NULL",
                    [id],
                )?;
                return Ok(id);
            }
            let name = format!("Локальный · {hostname}");
            c.execute(
                "INSERT INTO vpn_servers(name,hostname,public_ip,provider,location,protocol,status,enabled_for_provisioning,is_local,created_by,added_at,updated_at)
                 VALUES(?1,?2,'не указан','не указан','не указана','modern','unknown',1,1,?3,?4,?4)",
                rusqlite::params![name, hostname, actor, now],
            )?;
            let id = c.last_insert_rowid();
            c.execute(
                "INSERT INTO vpn_nodes(server_id,transport,status,created_at,updated_at)
                 VALUES(?1,'local','unknown',?2,?2)",
                rusqlite::params![id, now],
            )?;
            let node_id = c.last_insert_rowid();
            c.execute(
                "INSERT INTO vpn_instances(node_id,server_id,protocol,driver,status,is_default,created_at,updated_at)
                 VALUES(?1,?2,'modern','modern','unknown',1,?3,?3)",
                rusqlite::params![node_id, id, now],
            )?;
            c.execute(
                "UPDATE clients SET server_id=?1,
                 instance_id=(SELECT id FROM vpn_instances WHERE server_id=?1 AND is_default=1)
                 WHERE server_id IS NULL AND removed_at IS NULL",
                [id],
            )?;
            Ok(id)
        })
        .ok()
    }

    /// Removes controller placeholders accidentally created on a host without a
    /// local VPN. Servers with assigned clients are never touched.
    pub fn remove_empty_local_vpn_servers(&self) -> usize {
        self.with_conn(|connection| {
            connection.execute(
                "DELETE FROM vpn_servers
                 WHERE is_local=1
                   AND NOT EXISTS(
                     SELECT 1 FROM clients
                     WHERE clients.server_id=vpn_servers.id AND clients.removed_at IS NULL
                   )",
                [],
            )
        })
        .unwrap_or_default()
    }

    pub fn add_vpn_server(&self, value: &NewVpnServer<'_>, actor: i64, now: i64) -> Option<i64> {
        let valid = !value.name.trim().is_empty()
            && !value.hostname.trim().is_empty()
            && !value.public_ip.trim().is_empty()
            && valid_protocol(value.protocol);
        if !valid {
            return None;
        }
        self.with_conn(|c| {
            let transaction = c.unchecked_transaction()?;
            transaction.execute(
                "INSERT INTO vpn_servers(name,hostname,public_ip,provider,location,protocol,opened_at,is_local,created_by,added_at,updated_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10)",
                rusqlite::params![value.name.trim(),value.hostname.trim(),value.public_ip.trim(),value.provider.trim(),value.location.trim(),value.protocol,value.opened_at,value.is_local as i64,actor,now],
            )?;
            let server_id = transaction.last_insert_rowid();
            let transport = if value.is_local {
                "local"
            } else if value.protocol == "amneziawg-panel" {
                "panel_api"
            } else {
                "restricted_ssh"
            };
            transaction.execute(
                "INSERT INTO vpn_nodes(server_id,transport,status,created_at,updated_at)
                 VALUES(?1,?2,'unknown',?3,?3)",
                rusqlite::params![server_id, transport, now],
            )?;
            let node_id = transaction.last_insert_rowid();
            transaction.execute(
                "INSERT INTO vpn_instances(node_id,server_id,protocol,driver,status,is_default,created_at,updated_at)
                 VALUES(?1,?2,?3,?3,'unknown',1,?4,?4)",
                rusqlite::params![node_id, server_id, value.protocol, now],
            )?;
            transaction.commit()?;
            Ok(server_id)
        }).ok()
    }

    pub fn vpn_servers(&self) -> Vec<VpnServer> {
        self.with_conn(|c| {
            let mut statement = c.prepare(&format!(
                "SELECT {SERVER_COLUMNS} FROM vpn_servers ORDER BY name COLLATE NOCASE"
            ))?;
            let rows = statement.query_map([], server_from_row)?;
            rows.collect()
        })
        .unwrap_or_default()
    }

    pub fn vpn_server(&self, id: i64) -> Option<VpnServer> {
        self.with_conn(|c| {
            c.query_row(
                &format!("SELECT {SERVER_COLUMNS} FROM vpn_servers WHERE id=?1"),
                [id],
                server_from_row,
            )
            .optional()
        })
        .ok()
        .flatten()
    }

    pub fn update_server_billing(
        &self,
        id: i64,
        value: &ServerBillingUpdate<'_>,
        now: i64,
    ) -> bool {
        if value.paid_until <= now
            || !(1..=120).contains(&value.period_months)
            || !(0..=1_000_000_000).contains(&value.cost_minor)
            || value.currency.trim().is_empty()
            || value.currency.chars().count() > 8
        {
            return false;
        }
        self.with_conn(|c| c.execute(
            "UPDATE vpn_servers SET paid_until=?2,billing_period_months=?3,cost_minor=?4,currency=?5,auto_renew=?6,updated_at=?7 WHERE id=?1",
            rusqlite::params![id,value.paid_until,value.period_months,value.cost_minor,value.currency.trim().to_uppercase(),value.auto_renew as i64,now]
        )).is_ok_and(|n|n==1)
    }

    pub fn update_server_passport(&self, id: i64, value: &NewVpnServer<'_>, now: i64) -> bool {
        if value.name.trim().is_empty()
            || value.hostname.trim().is_empty()
            || value.public_ip.trim().is_empty()
            || !valid_protocol(value.protocol)
        {
            return false;
        }
        self.with_conn(|c| {
            let transaction = c.unchecked_transaction()?;
            let changed = transaction.execute(
                "UPDATE vpn_servers SET name=?2,hostname=?3,public_ip=?4,provider=?5,location=?6,protocol=?7,opened_at=?8,updated_at=?9 WHERE id=?1",
                rusqlite::params![id,value.name.trim(),value.hostname.trim(),value.public_ip.trim(),value.provider.trim(),value.location.trim(),value.protocol,value.opened_at,now],
            )?;
            transaction.execute(
                "UPDATE vpn_instances SET protocol=?2,driver=?2,updated_at=?3
                 WHERE server_id=?1 AND is_default=1",
                rusqlite::params![id, value.protocol, now],
            )?;
            transaction.commit()?;
            Ok(changed)
        })
        .is_ok_and(|n| n == 1)
    }

    pub fn set_local_server_status(&self, status: &str, now: i64) -> bool {
        let Some(server) = self
            .vpn_servers()
            .into_iter()
            .find(|server| server.is_local)
        else {
            return false;
        };
        self.set_server_status(server.id, status, now)
    }

    pub fn set_server_status(&self, id: i64, status: &str, now: i64) -> bool {
        if !matches!(
            status,
            "unknown" | "online" | "warning" | "offline" | "maintenance"
        ) {
            return false;
        }
        self.with_conn(|c| {
            let transaction = c.unchecked_transaction()?;
            let changed = transaction.execute(
                "UPDATE vpn_servers SET status=?2,updated_at=?3 WHERE id=?1",
                rusqlite::params![id, status, now],
            )?;
            transaction.execute(
                "UPDATE vpn_nodes SET status=?2,updated_at=?3 WHERE server_id=?1",
                rusqlite::params![id, status, now],
            )?;
            transaction.execute(
                "UPDATE vpn_instances SET status=?2,updated_at=?3
                 WHERE server_id=?1 AND is_default=1",
                rusqlite::params![id, status, now],
            )?;
            transaction.commit()?;
            Ok(changed)
        })
        .is_ok_and(|n| n == 1)
    }

    pub fn set_server_capacity(&self, id: i64, capacity: i64, now: i64) -> bool {
        if !(1..=100_000).contains(&capacity) {
            return false;
        }
        self.with_conn(|c| {
            c.execute(
                "UPDATE vpn_servers SET capacity=?2,updated_at=?3 WHERE id=?1",
                rusqlite::params![id, capacity, now],
            )
        })
        .is_ok_and(|changed| changed == 1)
    }

    pub fn set_server_provisioning(&self, id: i64, enabled: bool, now: i64) -> bool {
        self.with_conn(|c| {
            c.execute(
                "UPDATE vpn_servers SET enabled_for_provisioning=?2,updated_at=?3 WHERE id=?1",
                rusqlite::params![id, enabled as i64, now],
            )
        })
        .is_ok_and(|changed| changed == 1)
    }

    pub fn server_client_count(&self, id: i64) -> i64 {
        self.with_conn(|c| {
            c.query_row(
                "SELECT COUNT(*) FROM clients WHERE server_id=?1 AND removed_at IS NULL",
                [id],
                |row| row.get(0),
            )
        })
        .unwrap_or(0)
    }

    pub fn server_client_names(&self, id: i64) -> Vec<String> {
        self.with_conn(|c| {
            let mut statement = c.prepare(
                "SELECT name FROM clients WHERE server_id=?1 AND removed_at IS NULL ORDER BY name",
            )?;
            let rows = statement.query_map([id], |row| row.get(0))?;
            rows.collect()
        })
        .unwrap_or_default()
    }

    pub fn set_panel_credentials(
        &self,
        id: i64,
        url: &str,
        encrypted_password: &str,
        now: i64,
    ) -> bool {
        let key = format!("panel_password_{id}");
        let Ok(secret) = serde_json::to_string(encrypted_password) else {
            return false;
        };
        self.with_conn(|c| {
            let transaction = c.unchecked_transaction()?;
            let changed = transaction.execute(
                "UPDATE vpn_servers
                 SET panel_url=?2,protocol='amneziawg-panel',status='online',
                     enabled_for_provisioning=1,updated_at=?3
                 WHERE id=?1 AND is_local=0",
                rusqlite::params![id, url.trim_end_matches('/'), now],
            )?;
            if changed != 1 {
                return Ok(false);
            }
            transaction.execute(
                "UPDATE vpn_nodes SET transport='panel_api',status='online',updated_at=?2
                 WHERE server_id=?1",
                rusqlite::params![id, now],
            )?;
            transaction.execute(
                "UPDATE vpn_instances SET protocol='amneziawg-panel',driver='amneziawg-panel',
                 status='online',updated_at=?2 WHERE server_id=?1 AND is_default=1",
                rusqlite::params![id, now],
            )?;
            transaction.execute(
                "INSERT INTO settings(key,value) VALUES(?1,?2)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                rusqlite::params![key, secret],
            )?;
            transaction.commit()?;
            Ok(true)
        })
        .unwrap_or(false)
    }

    pub fn panel_password(&self, id: i64) -> Option<String> {
        let key = format!("panel_password_{id}");
        self.with_conn(|c| {
            c.query_row("SELECT value FROM settings WHERE key=?1", [key], |row| {
                row.get::<_, String>(0)
            })
            .optional()
        })
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_str(&value).ok())
    }

    pub fn sync_panel_clients(
        &self,
        server_id: i64,
        clients: &[(String, String)],
        now: i64,
    ) -> usize {
        self.with_conn(|c| {
            let transaction = c.unchecked_transaction()?;
            let mut changed = 0usize;
            for (name, address) in clients {
                let valid_name = name.len() <= 64
                    && name
                        .chars()
                        .next()
                        .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                    && name
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'));
                if !valid_name {
                    continue;
                }
                changed += transaction.execute(
                    "INSERT INTO clients(name,ip,first_seen,last_seen,server_id,protocol,instance_id,removed_at)
                     VALUES(?1,?2,?3,?3,?4,'amneziawg-panel',
                       (SELECT id FROM vpn_instances WHERE server_id=?4 AND is_default=1),NULL)
                     ON CONFLICT(name) DO UPDATE SET
                       ip=excluded.ip,last_seen=excluded.last_seen,server_id=excluded.server_id,
                       protocol='amneziawg-panel',instance_id=excluded.instance_id,removed_at=NULL",
                    rusqlite::params![name, address, now, server_id],
                )?;
            }
            transaction.commit()?;
            Ok(changed)
        })
        .unwrap_or(0)
    }

    pub fn approve_server_legacy_migration(&self, id: i64, now: i64) -> bool {
        self.with_conn(|c| {
            let transaction = c.unchecked_transaction()?;
            let server_changed = transaction.execute(
                "UPDATE vpn_servers
                 SET protocol='amneziawg-1',status='online',enabled_for_provisioning=1,updated_at=?2
                 WHERE id=?1 AND is_local=0 AND status='maintenance'",
                rusqlite::params![id, now],
            )?;
            if server_changed != 1 {
                return Ok(false);
            }
            transaction.execute(
                "UPDATE clients SET protocol='amneziawg-1' WHERE server_id=?1 AND removed_at IS NULL",
                [id],
            )?;
            transaction.execute(
                "UPDATE vpn_instances SET protocol='amneziawg-1',driver='amneziawg-1',
                 status='online',updated_at=?2 WHERE server_id=?1 AND is_default=1",
                rusqlite::params![id, now],
            )?;
            transaction.commit()?;
            Ok(true)
        })
        .unwrap_or(false)
    }

    pub fn finish_server_legacy_rollback(&self, id: i64, now: i64) -> bool {
        self.with_conn(|c| {
            let transaction = c.unchecked_transaction()?;
            let server_changed = transaction.execute(
                "UPDATE vpn_servers
                 SET protocol='amneziawg-2',status='online',enabled_for_provisioning=1,updated_at=?2
                 WHERE id=?1 AND is_local=0",
                rusqlite::params![id, now],
            )?;
            transaction.execute(
                "UPDATE clients SET protocol='amneziawg-2' WHERE server_id=?1 AND removed_at IS NULL",
                [id],
            )?;
            transaction.execute(
                "UPDATE vpn_instances SET protocol='amneziawg-2',driver='amneziawg-2',
                 status='online',updated_at=?2 WHERE server_id=?1 AND is_default=1",
                rusqlite::params![id, now],
            )?;
            transaction.commit()?;
            Ok(server_changed == 1)
        })
        .unwrap_or(false)
    }

    pub fn available_vpn_servers(&self) -> Vec<VpnServer> {
        self.vpn_servers()
            .into_iter()
            .filter(|server| {
                server.enabled_for_provisioning
                    && server.status != "offline"
                    && self.server_client_count(server.id) < server.capacity
            })
            .collect()
    }

    pub fn assign_client_server(&self, name: &str, server_id: i64, protocol: &str) -> bool {
        if !valid_protocol(protocol) {
            return false;
        }
        let Some(server) = self.vpn_server(server_id) else {
            return false;
        };
        if self.server_client_count(server_id) >= server.capacity {
            return false;
        }
        self.with_conn(|c| {
            c.execute(
                "UPDATE clients SET server_id=?2,protocol=?3,
                 instance_id=(SELECT id FROM vpn_instances WHERE server_id=?2 AND is_default=1)
                 WHERE name=?1 AND removed_at IS NULL",
                rusqlite::params![name, server_id, protocol],
            )
        })
        .is_ok_and(|changed| changed == 1)
    }

    pub fn client_vpn_server(&self, name: &str) -> Option<VpnServer> {
        self.with_conn(|connection| {
            connection
                .query_row(
                    &format!("SELECT {SERVER_COLUMNS} FROM vpn_servers WHERE id=(SELECT server_id FROM clients WHERE name=?1 AND removed_at IS NULL)"),
                    [name],
                    server_from_row,
                )
                .optional()
        })
        .ok()
        .flatten()
    }

    pub fn mark_server_billing_notification(
        &self,
        id: i64,
        paid_until: i64,
        days: i64,
        now: i64,
    ) -> bool {
        self.with_conn(|c| c.execute(
            "INSERT OR IGNORE INTO server_billing_notifications(server_id,paid_until,threshold_days,sent_at) VALUES(?1,?2,?3,?4)",
            rusqlite::params![id,paid_until,days,now]
        )).is_ok_and(|n|n==1)
    }
}

pub fn valid_protocol(value: &str) -> bool {
    crate::vpn::driver::Protocol::parse(value).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_server_passport_and_billing() {
        let store = Store::open_in_memory();
        let id = store
            .add_vpn_server(
                &NewVpnServer {
                    name: "Netherlands #1",
                    hostname: "nl1.example.com",
                    public_ip: "192.0.2.10",
                    provider: "Hoster",
                    location: "Amsterdam",
                    protocol: "modern",
                    opened_at: Some(100),
                    is_local: false,
                },
                1,
                200,
            )
            .unwrap();
        assert_eq!(store.vpn_server(id).unwrap().provider, "Hoster");
        assert!(store.update_server_billing(
            id,
            &ServerBillingUpdate {
                paid_until: 10_000,
                period_months: 1,
                cost_minor: 600,
                currency: "EUR",
                auto_renew: true,
            },
            300
        ));
        let server = store.vpn_server(id).unwrap();
        assert_eq!(server.cost_minor, Some(600));
        assert!(server.auto_renew);
        assert!(store.mark_server_billing_notification(id, 10_000, 7, 400));
        assert!(!store.mark_server_billing_notification(id, 10_000, 7, 401));
    }

    #[test]
    fn local_server_is_created_once_and_can_be_completed() {
        let store = Store::open_in_memory();
        let id = store.ensure_local_vpn_server("nl26", 1, 100).unwrap();
        assert_eq!(store.ensure_local_vpn_server("nl26", 1, 200), Some(id));
        assert_eq!(store.vpn_servers().len(), 1);
        assert!(store.vpn_server(id).unwrap().is_local);
        assert!(store.update_server_passport(
            id,
            &NewVpnServer {
                name: "Netherlands main",
                hostname: "nl26",
                public_ip: "192.0.2.26",
                provider: "Hoster",
                location: "Amsterdam",
                protocol: "modern",
                opened_at: Some(50),
                is_local: true,
            },
            300
        ));
        assert_eq!(store.vpn_server(id).unwrap().provider, "Hoster");
        assert!(store.set_local_server_status("online", 400));
        assert_eq!(store.vpn_server(id).unwrap().status, "online");
    }

    #[test]
    fn controller_cleanup_removes_only_empty_local_server() {
        let store = Store::open_in_memory();
        let id = store.ensure_local_vpn_server("controller", 1, 100).unwrap();
        assert!(store.vpn_server(id).is_some());
        assert_eq!(store.remove_empty_local_vpn_servers(), 1);
        assert!(store.vpn_server(id).is_none());
    }

    #[test]
    fn approving_remote_migration_updates_server_and_clients_atomically() {
        let store = Store::open_in_memory();
        let id = store
            .add_vpn_server(
                &NewVpnServer {
                    name: "nl26",
                    hostname: "nl26",
                    public_ip: "192.0.2.26",
                    provider: "Hoster",
                    location: "Amsterdam",
                    protocol: "amneziawg-2",
                    opened_at: None,
                    is_local: false,
                },
                1,
                100,
            )
            .unwrap();
        store.upsert_user(7, Some("tester"), "Tester", None, 100);
        store
            .with_conn(|connection| {
                connection.execute(
                    "INSERT INTO clients(name,ip,first_seen,last_seen) VALUES('test_key','',100,100)",
                    [],
                )
            })
            .unwrap();
        store.assign_client_owner("test_key", Some(7));
        assert!(store.assign_client_server("test_key", id, "amneziawg-2"));
        assert!(store.set_server_status(id, "maintenance", 101));
        assert!(store.approve_server_legacy_migration(id, 102));
        let server = store.vpn_server(id).unwrap();
        assert_eq!(server.protocol, "amneziawg-1");
        assert_eq!(server.status, "online");
        assert!(server.enabled_for_provisioning);
        assert_eq!(store.server_client_names(id), vec!["test_key"]);
        assert_eq!(
            store.client_vpn_server("test_key").unwrap().protocol,
            "amneziawg-1"
        );
    }

    #[test]
    fn panel_credentials_and_sync_preserve_existing_owner() {
        let store = Store::open_in_memory();
        let id = store
            .add_vpn_server(
                &NewVpnServer {
                    name: "Panel",
                    hostname: "panel.example",
                    public_ip: "192.0.2.50",
                    provider: "Hoster",
                    location: "Amsterdam",
                    protocol: "amneziawg-panel",
                    opened_at: None,
                    is_local: false,
                },
                1,
                100,
            )
            .unwrap();
        store.upsert_user(7, Some("alice"), "Alice", None, 100);
        store.assign_client_group("old", None, 100);
        store.assign_client_owner("old", Some(7));
        assert!(store.set_panel_credentials(id, "http://panel:1240/", "ciphertext", 101));
        assert_eq!(store.panel_password(id).as_deref(), Some("ciphertext"));
        assert_eq!(
            store.sync_panel_clients(
                id,
                &[
                    ("old".into(), "10.8.0.2".into()),
                    ("new".into(), "10.8.0.3".into())
                ],
                102,
            ),
            2
        );
        let server = store.vpn_server(id).unwrap();
        assert_eq!(server.protocol, "amneziawg-panel");
        assert!(server.enabled_for_provisioning);
        assert_eq!(server.panel_url.as_deref(), Some("http://panel:1240"));
        assert_eq!(store.client_owner("old"), Some(7));
        assert_eq!(store.client_owner("new"), None);
    }
}
