use rusqlite::OptionalExtension;

use crate::store::Store;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VpnNode {
    pub id: i64,
    pub server_id: i64,
    pub transport: String,
    pub status: String,
    pub agent_version: Option<String>,
    pub last_seen_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VpnInstance {
    pub id: i64,
    pub node_id: i64,
    pub server_id: i64,
    pub protocol: String,
    pub driver: String,
    pub status: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationJob {
    pub id: i64,
    pub server_id: i64,
    pub protocol: String,
    pub operation: String,
    pub status: String,
    pub stage: String,
    pub progress: i64,
    pub error_code: Option<String>,
    pub backup_ref: Option<String>,
}

fn node_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<VpnNode> {
    Ok(VpnNode {
        id: row.get(0)?,
        server_id: row.get(1)?,
        transport: row.get(2)?,
        status: row.get(3)?,
        agent_version: row.get(4)?,
        last_seen_at: row.get(5)?,
    })
}

fn instance_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<VpnInstance> {
    Ok(VpnInstance {
        id: row.get(0)?,
        node_id: row.get(1)?,
        server_id: row.get(2)?,
        protocol: row.get(3)?,
        driver: row.get(4)?,
        status: row.get(5)?,
        is_default: row.get::<_, i64>(6)? != 0,
    })
}

fn job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<InstallationJob> {
    Ok(InstallationJob {
        id: row.get(0)?,
        server_id: row.get(1)?,
        protocol: row.get(2)?,
        operation: row.get(3)?,
        status: row.get(4)?,
        stage: row.get(5)?,
        progress: row.get(6)?,
        error_code: row.get(7)?,
        backup_ref: row.get(8)?,
    })
}

impl Store {
    pub fn set_node_secret(&self, server_id: i64, encrypted_secret: &str, now: i64) -> bool {
        let key = format!("node_secret_{server_id}");
        let Ok(value) = serde_json::to_string(encrypted_secret) else {
            return false;
        };
        self.with_conn(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let changed = transaction.execute(
                "UPDATE vpn_nodes SET transport='signed_ssh',updated_at=?2 WHERE server_id=?1",
                rusqlite::params![server_id, now],
            )?;
            transaction.execute(
                "INSERT INTO settings(key,value) VALUES(?1,?2)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                rusqlite::params![key, value],
            )?;
            transaction.commit()?;
            Ok(changed)
        })
        .is_ok_and(|changed| changed == 1)
    }

    pub fn node_secret(&self, server_id: i64) -> Option<String> {
        let key = format!("node_secret_{server_id}");
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

    pub fn vpn_node_for_server(&self, server_id: i64) -> Option<VpnNode> {
        self.with_conn(|connection| {
            connection
                .query_row(
                    "SELECT id,server_id,transport,status,agent_version,last_seen_at
                     FROM vpn_nodes WHERE server_id=?1",
                    [server_id],
                    node_from_row,
                )
                .optional()
        })
        .ok()
        .flatten()
    }

    pub fn vpn_instances_for_server(&self, server_id: i64) -> Vec<VpnInstance> {
        self.with_conn(|connection| {
            let mut statement = connection.prepare(
                "SELECT id,node_id,server_id,protocol,driver,status,is_default
                 FROM vpn_instances WHERE server_id=?1 ORDER BY is_default DESC,id",
            )?;
            let instances = statement
                .query_map([server_id], instance_from_row)?
                .collect();
            instances
        })
        .unwrap_or_default()
    }

    pub fn set_node_seen(
        &self,
        server_id: i64,
        status: &str,
        agent_version: Option<&str>,
        now: i64,
    ) -> bool {
        self.with_conn(|connection| {
            connection.execute(
                "UPDATE vpn_nodes SET status=?2,agent_version=COALESCE(?3,agent_version),
                 last_seen_at=?4,updated_at=?4 WHERE server_id=?1",
                rusqlite::params![server_id, status, agent_version, now],
            )
        })
        .is_ok_and(|changed| changed == 1)
    }

    pub fn create_installation_job(
        &self,
        server_id: i64,
        protocol: &str,
        operation: &str,
        actor: i64,
        now: i64,
    ) -> Option<i64> {
        if crate::vpn::driver::Protocol::parse(protocol).is_none()
            || !matches!(operation, "install" | "upgrade" | "migrate" | "rollback")
        {
            return None;
        }
        self.with_conn(|connection| {
            connection.execute(
                "INSERT INTO installation_jobs(server_id,node_id,protocol,operation,requested_by,created_at)
                 VALUES(?1,(SELECT id FROM vpn_nodes WHERE server_id=?1),?2,?3,?4,?5)",
                rusqlite::params![server_id, protocol, operation, actor, now],
            )?;
            Ok(connection.last_insert_rowid())
        })
        .ok()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_installation_job(
        &self,
        id: i64,
        status: &str,
        stage: &str,
        progress: i64,
        error_code: Option<&str>,
        log_excerpt: Option<&str>,
        backup_ref: Option<&str>,
        now: i64,
    ) -> bool {
        if !matches!(
            status,
            "pending" | "running" | "complete" | "failed" | "rolled_back"
        ) || !(0..=100).contains(&progress)
            || stage.is_empty()
            || stage.len() > 64
        {
            return false;
        }
        let started_at = (status == "running").then_some(now);
        let finished_at = matches!(status, "complete" | "failed" | "rolled_back").then_some(now);
        self.with_conn(|connection| {
            connection.execute(
                "UPDATE installation_jobs SET status=?2,stage=?3,progress=?4,error_code=?5,
                 log_excerpt=?6,backup_ref=COALESCE(?7,backup_ref),
                 started_at=COALESCE(started_at,?8),finished_at=COALESCE(finished_at,?9)
                 WHERE id=?1",
                rusqlite::params![
                    id,
                    status,
                    stage,
                    progress,
                    error_code,
                    log_excerpt,
                    backup_ref,
                    started_at,
                    finished_at
                ],
            )
        })
        .is_ok_and(|changed| changed == 1)
    }

    pub fn installation_job(&self, id: i64) -> Option<InstallationJob> {
        self.with_conn(|connection| {
            connection
                .query_row(
                    "SELECT id,server_id,protocol,operation,status,stage,progress,error_code,backup_ref
                     FROM installation_jobs WHERE id=?1",
                    [id],
                    job_from_row,
                )
                .optional()
        })
        .ok()
        .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::NewVpnServer;

    #[test]
    fn new_server_gets_node_and_default_instance() {
        let store = Store::open_in_memory();
        let server_id = store
            .add_vpn_server(
                &NewVpnServer {
                    name: "Remote",
                    hostname: "192.0.2.8",
                    public_ip: "192.0.2.8",
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
        let node = store.vpn_node_for_server(server_id).unwrap();
        assert_eq!(node.transport, "restricted_ssh");
        let instances = store.vpn_instances_for_server(server_id);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].protocol, "amneziawg-2");
        assert!(instances[0].is_default);
        assert!(store.set_node_secret(server_id, "encrypted", 105));
        assert_eq!(store.node_secret(server_id).as_deref(), Some("encrypted"));
        let job = store
            .create_installation_job(server_id, "amneziawg-2", "install", 1, 110)
            .unwrap();
        assert!(store.update_installation_job(
            job,
            "running",
            "preflight",
            10,
            None,
            Some("ok"),
            Some("backup-1"),
            120,
        ));
        let job = store.installation_job(job).unwrap();
        assert_eq!(job.progress, 10);
        assert_eq!(job.backup_ref.as_deref(), Some("backup-1"));
    }
}
