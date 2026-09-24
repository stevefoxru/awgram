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
    pub blocked_by_rkn: bool,
    pub rkn_blocked_at: Option<i64>,
    pub operator_unavailable: bool,
    pub unavailable_at: Option<i64>,
    pub archived_at: Option<i64>,
    pub unavailable_reason: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedClientCleanup {
    pub name: String,
    pub owner_user_id: Option<i64>,
    pub server_id: i64,
    pub server_name: String,
    pub blocked_at: i64,
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
        blocked_by_rkn: row.get::<_, i64>(21)? != 0,
        rkn_blocked_at: row.get(22)?,
        operator_unavailable: row.get::<_, i64>(23)? != 0,
        unavailable_at: row.get(24)?,
        archived_at: row.get(25)?,
        unavailable_reason: row.get(26)?,
    })
}

const SERVER_COLUMNS: &str = "id,name,hostname,public_ip,provider,location,protocol,status,enabled_for_provisioning,opened_at,added_at,paid_until,billing_period_months,cost_minor,currency,auto_renew,panel_url,order_ref,note,is_local,capacity,blocked_by_rkn,rkn_blocked_at,operator_unavailable,unavailable_at,archived_at,unavailable_reason";

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
                "SELECT {SERVER_COLUMNS} FROM vpn_servers WHERE archived_at IS NULL ORDER BY name COLLATE NOCASE"
            ))?;
            let rows = statement.query_map([], server_from_row)?;
            rows.collect()
        })
        .unwrap_or_default()
    }

    pub fn archived_vpn_servers(&self) -> Vec<VpnServer> {
        self.with_conn(|c| {
            let mut statement = c.prepare(&format!(
                "SELECT {SERVER_COLUMNS} FROM vpn_servers WHERE archived_at IS NOT NULL ORDER BY archived_at DESC,name COLLATE NOCASE"
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

    pub fn update_server_field(&self, id: i64, field: &str, value: &str, now: i64) -> bool {
        let value = value.trim();
        if value.is_empty() || value.chars().count() > 200 {
            return false;
        }
        let result = match field {
            "name" | "hostname" | "public_ip" | "provider" | "location" | "note" => {
                let column = field;
                self.with_conn(|connection| {
                    connection.execute(
                        &format!("UPDATE vpn_servers SET {column}=?2,updated_at=?3 WHERE id=?1"),
                        rusqlite::params![id, value, now],
                    )
                })
            }
            "capacity" => value
                .parse::<i64>()
                .ok()
                .filter(|v| (1..=100_000).contains(v))
                .map_or_else(
                    || Err(rusqlite::Error::InvalidQuery),
                    |capacity| {
                        self.with_conn(|connection| {
                            connection.execute(
                                "UPDATE vpn_servers SET capacity=?2,updated_at=?3 WHERE id=?1",
                                rusqlite::params![id, capacity, now],
                            )
                        })
                    },
                ),
            "opened_at" => crate::calendar::parse_date(value).map_or_else(
                || Err(rusqlite::Error::InvalidQuery),
                |date| {
                    self.with_conn(|connection| {
                        connection.execute(
                            "UPDATE vpn_servers SET opened_at=?2,updated_at=?3 WHERE id=?1",
                            rusqlite::params![id, date, now],
                        )
                    })
                },
            ),
            _ => return false,
        };
        result.is_ok_and(|changed| changed == 1)
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
                "UPDATE vpn_servers SET enabled_for_provisioning=?2,updated_at=?3
                 WHERE id=?1 AND (?2=0 OR (blocked_by_rkn=0 AND operator_unavailable=0 AND archived_at IS NULL))",
                rusqlite::params![id, enabled as i64, now],
            )
        })
        .is_ok_and(|changed| changed == 1)
    }

    /// Sets the regulatory reachability flag. Marking a server blocked also
    /// removes it from every provisioning/replacement flow atomically.
    pub fn set_server_rkn_blocked(&self, id: i64, blocked: bool, now: i64) -> bool {
        self.with_conn(|connection| {
            connection.execute(
                "UPDATE vpn_servers
                 SET blocked_by_rkn=?2,
                     rkn_blocked_at=CASE WHEN ?2=1 THEN ?3 ELSE NULL END,
                     enabled_for_provisioning=CASE WHEN ?2=1 THEN 0 ELSE enabled_for_provisioning END,
                     updated_at=?3
                 WHERE id=?1 AND blocked_by_rkn<>?2",
                rusqlite::params![id, blocked as i64, now],
            )
        })
        .is_ok_and(|changed| changed == 1)
    }

    /// Marks an active server as unusable without deleting it or relying on a
    /// health-check status which may be overwritten by the next probe.
    pub fn set_server_operator_unavailable(
        &self,
        id: i64,
        unavailable: bool,
        reason: Option<&str>,
        actor: i64,
        now: i64,
    ) -> bool {
        self.with_conn(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let changed = transaction.execute(
                "UPDATE vpn_servers
                 SET operator_unavailable=?2,
                     unavailable_at=CASE WHEN ?2=1 THEN ?3 ELSE NULL END,
                     unavailable_reason=CASE WHEN ?2=1 THEN ?4 ELSE NULL END,
                     enabled_for_provisioning=CASE WHEN ?2=1 THEN 0 ELSE enabled_for_provisioning END,
                     updated_at=?3
                 WHERE id=?1 AND archived_at IS NULL AND operator_unavailable<>?2",
                rusqlite::params![id, unavailable as i64, now, reason],
            )?;
            if changed == 1 {
                transaction.execute(
                    "INSERT INTO server_lifecycle_events(server_id,action,reason,actor_id,created_at) VALUES(?1,?2,?3,?4,?5)",
                    rusqlite::params![id, if unavailable { "marked_unavailable" } else { "marked_available" }, reason, actor, now],
                )?;
            }
            transaction.commit()?;
            Ok(changed)
        })
        .is_ok_and(|changed| changed == 1)
    }

    pub fn set_server_archived(&self, id: i64, archived: bool, actor: i64, now: i64) -> bool {
        self.with_conn(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let changed = transaction.execute(
                "UPDATE vpn_servers
                 SET archived_at=CASE WHEN ?2=1 THEN ?3 ELSE NULL END,
                     operator_unavailable=CASE WHEN ?2=1 THEN 1 ELSE operator_unavailable END,
                     unavailable_at=CASE WHEN ?2=1 THEN COALESCE(unavailable_at,?3) ELSE unavailable_at END,
                     enabled_for_provisioning=CASE WHEN ?2=1 THEN 0 ELSE enabled_for_provisioning END,
                     updated_at=?3
                 WHERE id=?1 AND ((?2=1 AND archived_at IS NULL) OR (?2=0 AND archived_at IS NOT NULL))",
                rusqlite::params![id, archived as i64, now],
            )?;
            if changed == 1 {
                transaction.execute(
                    "INSERT INTO server_lifecycle_events(server_id,action,reason,actor_id,created_at) VALUES(?1,?2,NULL,?3,?4)",
                    rusqlite::params![id, if archived { "archived" } else { "restored" }, actor, now],
                )?;
            }
            transaction.commit()?;
            Ok(changed)
        })
        .is_ok_and(|changed| changed == 1)
    }

    pub fn server_lifecycle_events(
        &self,
        id: i64,
        limit: usize,
    ) -> Vec<(String, Option<String>, Option<i64>, i64)> {
        self.with_conn(|connection| {
            let mut statement = connection.prepare(
                "SELECT action,reason,actor_id,created_at FROM server_lifecycle_events WHERE server_id=?1 ORDER BY created_at DESC,id DESC LIMIT ?2",
            )?;
            let rows = statement.query_map(rusqlite::params![id, limit.min(50) as i64], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?;
            rows.collect()
        })
        .unwrap_or_default()
    }

    pub fn user_server_client_names(&self, user_id: i64, server_id: i64) -> Vec<String> {
        self.with_conn(|connection| {
            let mut statement = connection.prepare(
                "SELECT name FROM clients
                 WHERE owner_user_id=?1 AND server_id=?2 AND removed_at IS NULL
                 ORDER BY name COLLATE NOCASE",
            )?;
            let rows =
                statement.query_map(rusqlite::params![user_id, server_id], |row| row.get(0))?;
            rows.collect()
        })
        .unwrap_or_default()
    }

    pub fn server_client_owners(&self, server_id: i64) -> Vec<(i64, String)> {
        self.with_conn(|connection| {
            let mut statement = connection.prepare(
                "SELECT owner_user_id,name FROM clients
                 WHERE server_id=?1 AND owner_user_id IS NOT NULL AND removed_at IS NULL
                 ORDER BY owner_user_id,name COLLATE NOCASE",
            )?;
            let rows = statement.query_map([server_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect()
        })
        .unwrap_or_default()
    }

    pub fn server_unowned_client_count(&self, server_id: i64) -> usize {
        self.with_conn(|connection| {
            connection.query_row(
                "SELECT COUNT(*) FROM clients
                 WHERE server_id=?1 AND owner_user_id IS NULL AND removed_at IS NULL",
                [server_id],
                |row| row.get::<_, i64>(0),
            )
        })
        .unwrap_or(0)
        .max(0) as usize
    }

    pub fn begin_server_maintenance(&self, id: i64, actor_id: i64, now: i64) -> bool {
        self.with_conn(|c| {
            let tx = c.unchecked_transaction()?;
            let previous: i64 = tx.query_row(
                "SELECT enabled_for_provisioning FROM vpn_servers WHERE id=?1",
                [id],
                |row| row.get(0),
            )?;
            tx.execute(
                "INSERT INTO server_maintenance(server_id,previous_provisioning,started_at,started_by)
                 VALUES(?1,?2,?3,?4) ON CONFLICT(server_id) DO NOTHING",
                rusqlite::params![id, previous, now, actor_id],
            )?;
            let changed = tx.execute(
                "UPDATE vpn_servers SET status='maintenance',enabled_for_provisioning=0,updated_at=?2 WHERE id=?1",
                rusqlite::params![id, now],
            )?;
            tx.execute(
                "UPDATE vpn_nodes SET status='maintenance',updated_at=?2 WHERE server_id=?1",
                rusqlite::params![id, now],
            )?;
            tx.execute(
                "UPDATE vpn_instances SET status='maintenance',updated_at=?2 WHERE server_id=?1 AND is_default=1",
                rusqlite::params![id, now],
            )?;
            tx.commit()?;
            Ok(changed == 1)
        })
        .unwrap_or(false)
    }

    pub fn finish_server_maintenance(&self, id: i64, now: i64) -> bool {
        self.with_conn(|c| {
            let tx = c.unchecked_transaction()?;
            let previous = tx
                .query_row(
                    "SELECT previous_provisioning FROM server_maintenance WHERE server_id=?1",
                    [id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?
                .unwrap_or(0);
            let changed = tx.execute(
                "UPDATE vpn_servers SET status='online',enabled_for_provisioning=?2,updated_at=?3
                 WHERE id=?1 AND status='maintenance'",
                rusqlite::params![id, previous, now],
            )?;
            tx.execute(
                "UPDATE vpn_nodes SET status='online',updated_at=?2 WHERE server_id=?1",
                rusqlite::params![id, now],
            )?;
            tx.execute(
                "UPDATE vpn_instances SET status='online',updated_at=?2 WHERE server_id=?1 AND is_default=1",
                rusqlite::params![id, now],
            )?;
            tx.execute("DELETE FROM server_maintenance WHERE server_id=?1", [id])?;
            tx.commit()?;
            Ok(changed == 1)
        })
        .unwrap_or(false)
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

    pub fn server_owner_user_ids(&self, id: i64) -> Vec<i64> {
        self.with_conn(|c| {
            let mut statement = c.prepare(
                "SELECT DISTINCT owner_user_id FROM clients
                 WHERE server_id=?1 AND removed_at IS NULL AND owner_user_id IS NOT NULL
                 ORDER BY owner_user_id",
            )?;
            let rows = statement.query_map([id], |row| row.get(0))?;
            rows.collect()
        })
        .unwrap_or_default()
    }

    pub fn prepare_maintenance_notifications(
        &self,
        server_id: i64,
        started_at: i64,
        user_ids: &[i64],
    ) -> usize {
        self.with_conn(|c| {
            let tx = c.unchecked_transaction()?;
            let mut inserted = 0usize;
            for user_id in user_ids {
                inserted += tx.execute(
                    "INSERT OR IGNORE INTO maintenance_notifications(server_id,started_at,user_id,updated_at)
                     VALUES(?1,?2,?3,?2)",
                    rusqlite::params![server_id, started_at, user_id],
                )?;
            }
            tx.commit()?;
            Ok(inserted)
        })
        .unwrap_or(0)
    }

    pub fn mark_maintenance_notification(
        &self,
        server_id: i64,
        started_at: i64,
        user_id: i64,
        phase: &str,
        now: i64,
    ) -> bool {
        let column = match phase {
            "start" => "start_delivered",
            "finish" => "finish_delivered",
            _ => return false,
        };
        self.with_conn(|c| {
            c.execute(
                &format!("UPDATE maintenance_notifications SET {column}=1,updated_at=?4 WHERE server_id=?1 AND started_at=?2 AND user_id=?3"),
                rusqlite::params![server_id, started_at, user_id, now],
            )
        })
        .is_ok_and(|changed| changed == 1)
    }

    pub fn maintenance_finish_recipients(&self, server_id: i64) -> Option<(i64, Vec<i64>)> {
        self.with_conn(|c| {
            let started_at: Option<i64> = c.query_row(
                "SELECT MAX(started_at) FROM maintenance_notifications WHERE server_id=?1",
                [server_id],
                |row| row.get(0),
            )?;
            let Some(started_at) = started_at else {
                return Ok(None);
            };
            let mut statement = c.prepare(
                "SELECT user_id FROM maintenance_notifications
                 WHERE server_id=?1 AND started_at=?2 AND start_delivered=1 AND finish_delivered=0
                 ORDER BY user_id",
            )?;
            let rows =
                statement.query_map(rusqlite::params![server_id, started_at], |row| row.get(0))?;
            Ok(Some((
                started_at,
                rows.collect::<rusqlite::Result<Vec<_>>>()?,
            )))
        })
        .ok()
        .flatten()
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

    /// Stores an AmneziaVPN full-access export encrypted by the controller.
    /// The server remains in maintenance: importing a key never enables sales.
    pub fn set_amnezia_access(&self, id: i64, host: &str, encrypted_uri: &str, now: i64) -> bool {
        let key = format!("amnezia_access_{id}");
        let Ok(secret) = serde_json::to_string(encrypted_uri) else {
            return false;
        };
        self.with_conn(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let changed = transaction.execute(
                "UPDATE vpn_servers
                 SET hostname=?2,public_ip=?2,protocol='amneziawg-3',status='maintenance',
                     enabled_for_provisioning=0,updated_at=?3
                 WHERE id=?1 AND is_local=0",
                rusqlite::params![id, host, now],
            )?;
            if changed != 1 {
                return Ok(false);
            }
            transaction.execute(
                "UPDATE vpn_nodes SET transport='restricted_ssh',status='unknown',updated_at=?2
                 WHERE server_id=?1",
                rusqlite::params![id, now],
            )?;
            transaction.execute(
                "UPDATE vpn_instances SET protocol='amneziawg-3',driver='amneziawg-3',
                 status='maintenance',updated_at=?2 WHERE server_id=?1 AND is_default=1",
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

    pub fn amnezia_access(&self, id: i64) -> Option<String> {
        let key = format!("amnezia_access_{id}");
        self.with_conn(|connection| {
            connection
                .query_row("SELECT value FROM settings WHERE key=?1", [key], |row| {
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
                       ip=excluded.ip,last_seen=excluded.last_seen,
                       server_id=CASE
                         WHEN clients.removed_at IS NULL AND clients.server_id IS NULL THEN excluded.server_id
                         ELSE clients.server_id END,
                       protocol=CASE
                         WHEN clients.removed_at IS NULL AND clients.server_id IS NULL THEN 'amneziawg-panel'
                         ELSE clients.protocol END,
                       instance_id=CASE
                         WHEN clients.removed_at IS NULL AND clients.server_id IS NULL THEN excluded.instance_id
                         ELSE clients.instance_id END",
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
        let preferred = self.default_vpn_server();
        let mut servers = self
            .vpn_servers()
            .into_iter()
            .filter(|server| {
                server.enabled_for_provisioning
                    && !server.blocked_by_rkn
                    && !server.operator_unavailable
                    && server.archived_at.is_none()
                    && valid_protocol(&server.protocol)
                    && server.status != "offline"
                    && self.server_client_count(server.id) < server.capacity
            })
            .collect::<Vec<_>>();
        // The recovery/default server is always offered first in every key
        // creation flow, while preserving the configured order of the rest.
        servers.sort_by_key(|server| (Some(server.id) != preferred, server.id));
        servers
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

    pub fn client_vpn_server_including_retired(&self, name: &str) -> Option<VpnServer> {
        self.with_conn(|connection| {
            connection
                .query_row(
                    &format!("SELECT {SERVER_COLUMNS} FROM vpn_servers WHERE id=(SELECT server_id FROM clients WHERE name=?1)"),
                    [name],
                    server_from_row,
                )
                .optional()
        })
        .ok()
        .flatten()
    }

    /// Logically retires a client whose source server cannot be contacted.
    /// This keeps history intact while removing the stale key from all active
    /// customer/admin lists and capacity calculations.
    pub fn retire_client(&self, name: &str, now: i64) -> bool {
        self.with_conn(|connection| {
            let transaction = connection.unchecked_transaction()?;
            transaction.execute(
                "INSERT INTO client_archive_events(client_name,server_id,owner_user_id,reason,archived_at)
                 SELECT name,server_id,owner_user_id,'retired',?2 FROM clients
                 WHERE name=?1 AND removed_at IS NULL",
                rusqlite::params![name, now],
            )?;
            let changed = transaction.execute(
                "UPDATE clients SET removed_at=?2 WHERE name=?1 AND removed_at IS NULL",
                rusqlite::params![name, now],
            )?;
            transaction.commit()?;
            Ok(changed)
        })
        .is_ok_and(|changed| changed == 1)
    }

    pub fn blocked_clients_for_cleanup(
        &self,
        now: i64,
        minimum_days: i64,
    ) -> Vec<BlockedClientCleanup> {
        let cutoff = now.saturating_sub(minimum_days.max(0).saturating_mul(86_400));
        self.with_conn(|connection| {
            let mut statement = connection.prepare(
                "SELECT c.name,c.owner_user_id,s.id,s.name,
                        MIN(COALESCE(s.unavailable_at,9223372036854775807),COALESCE(s.rkn_blocked_at,9223372036854775807),COALESCE(s.archived_at,9223372036854775807))
                   FROM clients c JOIN vpn_servers s ON s.id=c.server_id
                  WHERE c.removed_at IS NULL
                    AND (s.operator_unavailable=1 OR s.blocked_by_rkn=1 OR s.archived_at IS NOT NULL)
                    AND MIN(COALESCE(s.unavailable_at,9223372036854775807),COALESCE(s.rkn_blocked_at,9223372036854775807),COALESCE(s.archived_at,9223372036854775807))<=?1
                    AND NOT EXISTS(SELECT 1 FROM key_replacements kr WHERE kr.old_client=c.name AND kr.status='pending')
                  ORDER BY s.id,c.name COLLATE NOCASE",
            )?;
            let rows = statement.query_map([cutoff], |row| {
                Ok(BlockedClientCleanup {
                    name: row.get(0)?,
                    owner_user_id: row.get(1)?,
                    server_id: row.get(2)?,
                    server_name: row.get(3)?,
                    blocked_at: row.get(4)?,
                })
            })?;
            rows.collect()
        })
        .unwrap_or_default()
    }

    pub fn mark_blocked_cleanup_notification(
        &self,
        name: &str,
        blocked_at: i64,
        threshold_days: i64,
        now: i64,
    ) -> bool {
        self.with_conn(|connection| {
            connection.execute(
                "INSERT OR IGNORE INTO blocked_client_cleanup_notifications(client_name,blocked_at,threshold_days,sent_at) VALUES(?1,?2,?3,?4)",
                rusqlite::params![name, blocked_at, threshold_days, now],
            )
        })
        .is_ok_and(|changed| changed == 1)
    }

    pub fn unmark_blocked_cleanup_notification(
        &self,
        name: &str,
        blocked_at: i64,
        threshold_days: i64,
    ) {
        let _ = self.with_conn(|connection| {
            connection.execute(
                "DELETE FROM blocked_client_cleanup_notifications WHERE client_name=?1 AND blocked_at=?2 AND threshold_days=?3",
                rusqlite::params![name, blocked_at, threshold_days],
            )
        });
    }

    pub fn revive_client(&self, name: &str) -> bool {
        self.with_conn(|connection| {
            connection.execute(
                "UPDATE clients SET removed_at=NULL WHERE name=?1 AND removed_at IS NOT NULL",
                [name],
            )
        })
        .is_ok_and(|changed| changed == 1)
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
    fn unavailable_and_archived_servers_are_kept_but_excluded_from_active_lists() {
        let store = Store::open_in_memory();
        let id = store
            .add_vpn_server(
                &NewVpnServer {
                    name: "Retired Netherlands",
                    hostname: "retired.example.com",
                    public_ip: "192.0.2.44",
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
        assert!(store.set_server_operator_unavailable(id, true, Some("сломался"), 1, 200));
        let server = store.vpn_server(id).unwrap();
        assert!(server.operator_unavailable);
        assert!(!server.enabled_for_provisioning);
        store.upsert_user(7, None, "Owner", None, 100);
        store.assign_client_group("old-key", None, 100);
        assert!(store.assign_client_owner("old-key", Some(7)));
        assert!(store.assign_client_server("old-key", id, "amneziawg-1"));
        assert!(store
            .blocked_clients_for_cleanup(200 + 23 * 86_400, 30)
            .is_empty());
        assert_eq!(
            store.blocked_clients_for_cleanup(200 + 23 * 86_400, 23)[0].name,
            "old-key"
        );
        assert_eq!(
            store.blocked_clients_for_cleanup(200 + 30 * 86_400, 30)[0].owner_user_id,
            Some(7)
        );
        assert!(store.mark_blocked_cleanup_notification("old-key", 200, 7, 300));
        assert!(!store.mark_blocked_cleanup_notification("old-key", 200, 7, 301));

        assert!(store.set_server_archived(id, true, 1, 300));
        assert!(store.vpn_servers().is_empty());
        assert_eq!(store.archived_vpn_servers()[0].id, id);
        assert!(store.vpn_server(id).is_some());

        assert!(store.set_server_archived(id, false, 1, 400));
        assert_eq!(store.vpn_servers()[0].id, id);
        assert!(store.vpn_server(id).unwrap().operator_unavailable);
        assert_eq!(store.server_lifecycle_events(id, 10).len(), 3);
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
    fn preferred_server_is_first_in_provisioning_lists() {
        let store = Store::open_in_memory();
        let add = |name: &str, ip: &str, now: i64| {
            store
                .add_vpn_server(
                    &NewVpnServer {
                        name,
                        hostname: name,
                        public_ip: ip,
                        provider: "Hoster",
                        location: name,
                        protocol: "amneziawg-panel",
                        opened_at: None,
                        is_local: false,
                    },
                    1,
                    now,
                )
                .unwrap()
        };
        let first = add("first", "192.0.2.1", 100);
        let preferred = add("preferred", "192.0.2.2", 101);
        for id in [first, preferred] {
            assert!(store.set_server_status(id, "online", 102));
            assert!(store.set_server_provisioning(id, true, 102));
        }
        store.set_default_vpn_server(preferred);
        assert_eq!(
            store
                .available_vpn_servers()
                .into_iter()
                .map(|server| server.id)
                .collect::<Vec<_>>(),
            vec![preferred, first]
        );
    }

    #[test]
    fn maintenance_disables_and_restores_previous_provisioning() {
        let store = Store::open_in_memory();
        let id = store.ensure_local_vpn_server("vpn", 1, 100).unwrap();
        assert!(store.set_server_status(id, "online", 101));
        assert!(store.set_server_provisioning(id, true, 101));
        assert!(store.begin_server_maintenance(id, 42, 102));
        let server = store.vpn_server(id).unwrap();
        assert_eq!(server.status, "maintenance");
        assert!(!server.enabled_for_provisioning);
        assert!(store.finish_server_maintenance(id, 103));
        let server = store.vpn_server(id).unwrap();
        assert_eq!(server.status, "online");
        assert!(server.enabled_for_provisioning);
        assert!(!store.finish_server_maintenance(id, 104));
    }

    #[test]
    fn maintenance_notifications_are_unique_and_resume_after_restart() {
        let store = Store::open_in_memory();
        let id = store.ensure_local_vpn_server("vpn", 1, 100).unwrap();
        for (user_id, name) in [(7, "alice"), (8, "bob")] {
            store.upsert_user(user_id, None, name, None, 100);
            let client = format!("{name}-phone");
            store.assign_client_group(&client, None, 100);
            assert!(store.assign_client_owner(&client, Some(user_id)));
            assert!(store.assign_client_server(&client, id, "amneziawg-1"));
        }
        let owners = store.server_owner_user_ids(id);
        assert_eq!(owners, vec![7, 8]);
        assert_eq!(store.prepare_maintenance_notifications(id, 200, &owners), 2);
        assert_eq!(store.prepare_maintenance_notifications(id, 200, &owners), 0);
        assert!(store.mark_maintenance_notification(id, 200, 7, "start", 201));
        assert_eq!(
            store.maintenance_finish_recipients(id),
            Some((200, vec![7]))
        );
        assert!(store.mark_maintenance_notification(id, 200, 7, "finish", 202));
        assert_eq!(store.maintenance_finish_recipients(id), Some((200, vec![])));
    }

    #[test]
    fn retiring_unreachable_client_removes_it_from_active_inventory() {
        let store = Store::open_in_memory();
        store.assign_client_group("broken-key", None, 100);
        assert!(store
            .active_client_names()
            .contains(&"broken-key".to_string()));
        assert!(store.retire_client("broken-key", 200));
        assert!(!store
            .active_client_names()
            .contains(&"broken-key".to_string()));
        assert!(!store.retire_client("broken-key", 201));
        assert!(store.revive_client("broken-key"));
        assert!(store
            .active_client_names()
            .contains(&"broken-key".to_string()));
        assert!(!store.revive_client("broken-key"));
    }

    #[test]
    fn pending_replacement_can_be_resumed_without_creating_a_duplicate() {
        let store = Store::open_in_memory();
        let server_id = store.ensure_local_vpn_server("vpn", 1, 100).unwrap();
        store.upsert_user(7, Some("alice"), "Alice", None, 100);
        for name in ["old-key", "new-key"] {
            store.assign_client_group(name, None, 100);
            assert!(store.assign_client_owner(name, Some(7)));
        }
        let id = store
            .create_key_replacement(7, "old-key", "new-key", server_id, 101)
            .unwrap();
        assert_eq!(
            store.pending_key_replacement(7, "old-key"),
            Some((id, "new-key".into(), server_id))
        );
        assert!(store.retire_client("old-key", 102));
        assert_eq!(store.user_client_names(7), vec!["new-key"]);
        assert_eq!(
            store.decide_key_replacement(id, 7, "confirmed", 103),
            Some(("old-key".into(), "new-key".into()))
        );
        assert_eq!(store.pending_key_replacement(7, "old-key"), None);
    }

    #[test]
    fn replacement_staging_is_atomic_and_preserves_customer_metadata() {
        let store = Store::open_in_memory();
        let server_id = store.ensure_local_vpn_server("vpn", 1, 100).unwrap();
        store.upsert_user(7, Some("alice"), "Alice", None, 100);
        store.assign_client_group("old-key", None, 100);
        assert!(store.assign_client_owner("old-key", Some(7)));
        assert!(store.assign_client_server("old-key", server_id, "amneziawg-1"));
        assert!(store.set_device_label("old-key", 7, "Телефон"));
        let id = store
            .create_key_replacement(7, "old-key", "new-key", server_id, 101)
            .unwrap();

        assert!(store.stage_key_replacement(id, 7, server_id, "amneziawg-1", 102));
        assert_eq!(store.user_client_names(7), vec!["new-key"]);
        assert_eq!(store.device_label("new-key").as_deref(), Some("Телефон"));
        assert_eq!(store.client_owner("new-key"), Some(7));
        assert_eq!(store.client_vpn_server("new-key").unwrap().id, server_id);
        assert_eq!(
            store
                .client_vpn_server_including_retired("old-key")
                .unwrap()
                .id,
            server_id
        );
    }

    #[test]
    fn interrupted_replacement_revives_old_key() {
        let store = Store::open_in_memory();
        let server_id = store.ensure_local_vpn_server("vpn", 1, 100).unwrap();
        store.upsert_user(7, Some("alice"), "Alice", None, 100);
        store.assign_client_group("old-key", None, 100);
        assert!(store.assign_client_owner("old-key", Some(7)));
        store
            .create_key_replacement(7, "old-key", "missing-new", server_id, 101)
            .unwrap();
        assert!(store.retire_client("old-key", 102));

        assert_eq!(store.repair_user_key_replacements(7, 103), 1);
        assert_eq!(store.user_client_names(7), vec!["old-key"]);
        assert_eq!(store.user_pending_key_replacement_count(7), 0);
    }

    #[test]
    fn pending_replacements_are_listed_and_counted_per_user() {
        let store = Store::open_in_memory();
        let server_id = store.ensure_local_vpn_server("vpn", 1, 100).unwrap();
        store.upsert_user(7, Some("alice"), "Alice", None, 100);
        store.upsert_user(8, Some("bob"), "Bob", None, 100);
        let first = store
            .create_key_replacement(7, "old-a", "new-a", server_id, 101)
            .unwrap();
        store
            .create_key_replacement(8, "old-b", "new-b", server_id, 102)
            .unwrap();

        let pending = store.pending_key_replacements();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].id, first);
        assert_eq!(store.user_pending_key_replacement_count(7), 1);
        assert_eq!(store.user_pending_key_replacement_count(9), 0);

        assert!(store
            .decide_key_replacement(first, 7, "confirmed", 103)
            .is_some());
        assert_eq!(store.pending_key_replacements().len(), 1);
        assert_eq!(store.user_pending_key_replacement_count(7), 0);
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

        assert!(store.retire_client("old", 103));
        assert_eq!(
            store.sync_panel_clients(id, &[("old".into(), "10.8.0.2".into())], 104),
            1
        );
        assert_eq!(store.client_owner("old"), None);
        assert!(!store.user_client_names(7).contains(&"old".to_string()));
        assert_eq!(
            store.client_vpn_server_including_retired("old").unwrap().id,
            id
        );
    }
}
