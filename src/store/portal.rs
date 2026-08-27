use rand::RngExt;
use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};

use crate::store::Store;

#[derive(Debug, Clone, serde::Serialize)]
pub struct PortalKey {
    pub name: String,
    pub device: String,
    pub location: String,
    pub protocol: String,
    pub server_status: String,
    pub rx: u64,
    pub tx: u64,
    pub last_handshake: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PortalOverview {
    pub user_id: i64,
    pub display_name: String,
    pub username: Option<String>,
    pub balance_kopecks: i64,
    pub keys: Vec<PortalKey>,
}

fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

impl Store {
    pub fn issue_portal_token(&self, user_id: i64, now: i64) -> Option<String> {
        self.user(user_id)?;
        let random: [u8; 32] = rand::rng().random();
        let token =
            base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, random);
        let hash = token_hash(&token);
        self.with_conn(|connection| {
            connection.execute(
                "UPDATE web_sessions SET revoked_at=?2
                 WHERE user_id=?1 AND revoked_at IS NULL",
                rusqlite::params![user_id, now],
            )?;
            connection.execute(
                "INSERT INTO web_sessions(token_hash,user_id,created_at,expires_at)
                 VALUES(?1,?2,?3,?4)",
                rusqlite::params![hash, user_id, now, now + 15 * 60],
            )?;
            Ok(())
        })
        .ok()?;
        Some(token)
    }

    pub fn activate_portal_token(&self, token: &str, now: i64) -> Option<String> {
        let login_hash = token_hash(token);
        let user_id = self
            .with_conn(|connection| {
                connection
                    .query_row(
                        "SELECT user_id FROM web_sessions
                 WHERE token_hash=?1 AND expires_at>=?2
                   AND activated_at IS NULL AND revoked_at IS NULL",
                        rusqlite::params![login_hash, now],
                        |row| row.get::<_, i64>(0),
                    )
                    .optional()
            })
            .ok()
            .flatten()?;
        let random: [u8; 32] = rand::rng().random();
        let session =
            base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, random);
        let session_hash = token_hash(&session);
        self.with_conn(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let changed = transaction.execute(
                "UPDATE web_sessions SET activated_at=?2,revoked_at=?2
                 WHERE token_hash=?1 AND activated_at IS NULL AND revoked_at IS NULL",
                rusqlite::params![login_hash, now],
            )?;
            if changed != 1 {
                return Ok(false);
            }
            transaction.execute(
                "INSERT INTO web_sessions(token_hash,user_id,created_at,expires_at,activated_at)
                 VALUES(?1,?2,?3,?4,?3)",
                rusqlite::params![session_hash, user_id, now, now + 30 * 86_400],
            )?;
            transaction.commit()?;
            Ok(true)
        })
        .ok()
        .filter(|changed| *changed)
        .map(|_| session)
    }

    pub fn portal_user_id(&self, session: &str, now: i64) -> Option<i64> {
        self.with_conn(|connection| {
            connection
                .query_row(
                    "SELECT user_id FROM web_sessions
                 WHERE token_hash=?1 AND expires_at>=?2
                   AND activated_at IS NOT NULL AND revoked_at IS NULL",
                    rusqlite::params![token_hash(session), now],
                    |row| row.get(0),
                )
                .optional()
        })
        .ok()
        .flatten()
    }

    pub fn portal_logout(&self, session: &str, now: i64) -> bool {
        self.with_conn(|connection| {
            connection.execute(
                "UPDATE web_sessions SET revoked_at=?2 WHERE token_hash=?1 AND revoked_at IS NULL",
                rusqlite::params![token_hash(session), now],
            )
        })
        .map(|changed| changed == 1)
        .unwrap_or(false)
    }

    pub fn portal_overview(&self, user_id: i64) -> Option<PortalOverview> {
        let user = self.user(user_id)?;
        let keys = self.with_conn(|connection| {
            let mut statement = connection.prepare(
                "SELECT c.name,COALESCE(c.device_label,'Не указано'),
                        COALESCE(s.location,'Не определён'),c.protocol,
                        COALESCE(s.status,'unknown'),
                        COALESCE((SELECT rx FROM traffic_samples t WHERE t.client_id=c.id ORDER BY ts DESC LIMIT 1),0),
                        COALESCE((SELECT tx FROM traffic_samples t WHERE t.client_id=c.id ORDER BY ts DESC LIMIT 1),0),
                        (SELECT ts FROM traffic_samples t WHERE t.client_id=c.id AND t.online=1 ORDER BY ts DESC LIMIT 1)
                 FROM clients c LEFT JOIN vpn_servers s ON s.id=c.server_id
                 WHERE c.owner_user_id=?1 AND c.removed_at IS NULL ORDER BY c.name",
            )?;
            let rows = statement.query_map([user_id], |row| Ok(PortalKey {
                name: row.get(0)?, device: row.get(1)?, location: row.get(2)?,
                protocol: row.get(3)?, server_status: row.get(4)?,
                rx: row.get::<_, i64>(5)?.max(0) as u64,
                tx: row.get::<_, i64>(6)?.max(0) as u64,
                last_handshake: row.get(7)?,
            }))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        }).unwrap_or_default();
        Some(PortalOverview {
            user_id,
            display_name: user.display_name,
            username: user.username,
            balance_kopecks: self.balance_kopecks(user_id),
            keys,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_link_is_one_time_and_session_can_be_revoked() {
        let store = Store::open_in_memory();
        store.upsert_user(7, Some("alice"), "Alice", None, 100);
        let login = store.issue_portal_token(7, 101).unwrap();
        let session = store.activate_portal_token(&login, 102).unwrap();
        assert!(store.activate_portal_token(&login, 103).is_none());
        assert_eq!(store.portal_user_id(&session, 104), Some(7));
        assert!(store.portal_logout(&session, 105));
        assert_eq!(store.portal_user_id(&session, 106), None);
    }
}
