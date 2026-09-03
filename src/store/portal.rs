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
    pub enabled: Option<bool>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PortalOverview {
    pub user_id: i64,
    pub display_name: String,
    pub username: Option<String>,
    pub balance_kopecks: i64,
    pub keys: Vec<PortalKey>,
    pub payments: Vec<PortalPayment>,
    pub balance_history: Vec<PortalBalanceEntry>,
    pub tickets: Vec<PortalTicket>,
    pub expiry_notifications: bool,
    pub maintenance_notifications: bool,
    pub discount_percent: i64,
    pub referral_count: i64,
    pub referral_percent: u8,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PortalBalanceEntry {
    pub amount_kopecks: i64,
    pub kind: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PortalTicket {
    pub id: i64,
    pub category: String,
    pub status: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PortalPayment {
    pub id: i64,
    pub amount_kopecks: i64,
    pub method: String,
    pub status: String,
    pub created_at: i64,
}

fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

impl Store {
    pub fn portal_email(&self, user_id: i64) -> Option<String> {
        self.with_conn(|connection| {
            connection
                .query_row(
                    "SELECT email FROM users WHERE user_id=?1 AND email_verified_at IS NOT NULL",
                    [user_id],
                    |row| row.get(0),
                )
                .optional()
        })
        .ok()
        .flatten()
    }

    pub fn request_email_code(
        &self,
        user_id: i64,
        email: &str,
        purpose: &str,
        code: &str,
        now: i64,
    ) -> bool {
        let email = email.trim().to_lowercase();
        if !matches!(purpose, "bind" | "login") || !email.contains('@') || email.len() > 254 {
            return false;
        }
        let salt_raw: [u8; 16] = rand::rng().random();
        let salt =
            base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, salt_raw);
        let hash = token_hash(&format!("{salt}:{code}"));
        self.with_conn(|connection| {
            let recent: i64 = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM email_codes WHERE user_id=?1 AND purpose=?2 AND created_at>?3)",
                rusqlite::params![user_id,purpose,now-60], |row| row.get(0)
            )?;
            if recent != 0 { return Ok(false); }
            connection.execute("UPDATE email_codes SET used_at=?2 WHERE user_id=?1 AND purpose=?3 AND used_at IS NULL", rusqlite::params![user_id,now,purpose])?;
            connection.execute("INSERT INTO email_codes(user_id,email,purpose,salt,code_hash,created_at,expires_at) VALUES(?1,?2,?3,?4,?5,?6,?7)", rusqlite::params![user_id,email,purpose,salt,hash,now,now+600])?;
            Ok(true)
        }).unwrap_or(false)
    }

    pub fn verified_user_by_email(&self, email: &str) -> Option<i64> {
        self.with_conn(|connection| connection.query_row(
            "SELECT user_id FROM users WHERE lower(email)=lower(?1) AND email_verified_at IS NOT NULL",
            [email.trim()], |row| row.get(0)
        ).optional()).ok().flatten()
    }

    pub fn confirm_email_code(
        &self,
        user_id: i64,
        email: &str,
        purpose: &str,
        code: &str,
        now: i64,
    ) -> bool {
        let result = self.with_conn(|connection| {
            let row: Option<(i64,String,String,i64)> = connection.query_row(
                "SELECT id,salt,code_hash,attempts FROM email_codes WHERE user_id=?1 AND lower(email)=lower(?2) AND purpose=?3 AND used_at IS NULL AND expires_at>=?4 ORDER BY created_at DESC LIMIT 1",
                rusqlite::params![user_id,email.trim(),purpose,now],
                |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))
            ).optional()?;
            let Some((id,salt,expected,attempts))=row else{return Ok(false)};
            if attempts >= 5 { return Ok(false); }
            let valid = token_hash(&format!("{salt}:{code}")) == expected;
            if valid {
                connection.execute("UPDATE email_codes SET used_at=?2 WHERE id=?1", rusqlite::params![id,now])?;
                if purpose == "bind" {
                    connection.execute("UPDATE users SET email=?2,email_verified_at=?3 WHERE user_id=?1", rusqlite::params![user_id,email.trim().to_lowercase(),now])?;
                }
            } else {
                connection.execute("UPDATE email_codes SET attempts=attempts+1 WHERE id=?1", [id])?;
            }
            Ok(valid)
        });
        result.unwrap_or(false)
    }

    pub fn create_portal_session(&self, user_id: i64, now: i64) -> Option<String> {
        self.user(user_id)?;
        let random: [u8; 32] = rand::rng().random();
        let session =
            base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, random);
        self.with_conn(|connection| connection.execute(
            "INSERT INTO web_sessions(token_hash,user_id,created_at,expires_at,activated_at) VALUES(?1,?2,?3,?4,?3)",
            rusqlite::params![token_hash(&session),user_id,now,now+30*86_400]
        )).ok()?;
        Some(session)
    }

    pub fn prune_portal_sessions(&self, now: i64) -> usize {
        self.with_conn(|connection| {
            connection.execute(
                "DELETE FROM web_sessions WHERE expires_at<?1 OR (revoked_at IS NOT NULL AND revoked_at<?2)",
                rusqlite::params![now,now-7*86_400],
            )
        }).unwrap_or_default()
    }

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

    pub fn portal_overview(&self, user_id: i64, now: i64) -> Option<PortalOverview> {
        let user = self.user(user_id)?;
        let mut keys = self.with_conn(|connection| {
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
                enabled: None,
                expires_at: None,
            }))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        }).unwrap_or_default();
        for key in &mut keys {
            if let Some(runtime) = self.client_runtime_stats(&key.name) {
                key.rx = runtime.rx;
                key.tx = runtime.tx;
                key.last_handshake = runtime.last_handshake;
                key.enabled = runtime.enabled;
            }
        }
        let payments = self
            .with_conn(|connection| {
                let mut statement = connection.prepare(
                    "SELECT id,amount_kopecks,method,status,created_at FROM payment_requests
                 WHERE user_id=?1 ORDER BY created_at DESC LIMIT 20",
                )?;
                let rows = statement.query_map([user_id], |row| {
                    Ok(PortalPayment {
                        id: row.get(0)?,
                        amount_kopecks: row.get(1)?,
                        method: row.get(2)?,
                        status: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap_or_default();
        let balance_history = self
            .balance_history(user_id, 10)
            .into_iter()
            .map(|entry| PortalBalanceEntry {
                amount_kopecks: entry.amount_kopecks,
                kind: entry.kind,
                created_at: entry.created_at,
            })
            .collect();
        let tickets = self
            .user_support_tickets(user_id, 10)
            .into_iter()
            .map(|ticket| PortalTicket {
                id: ticket.id,
                category: ticket.category,
                status: ticket.status,
                updated_at: ticket.updated_at,
            })
            .collect();
        let (expiry_notifications, maintenance_notifications) =
            self.notification_preferences(user_id);
        Some(PortalOverview {
            user_id,
            display_name: user.display_name,
            username: user.username,
            balance_kopecks: self.balance_kopecks(user_id),
            keys,
            payments,
            balance_history,
            tickets,
            expiry_notifications,
            maintenance_notifications,
            discount_percent: self.peek_purchase_discount(user_id, now),
            referral_count: self.referral_count(user_id),
            referral_percent: self.referral_percent(),
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
        let overview = store.portal_overview(7, 106).unwrap();
        assert_eq!(overview.balance_kopecks, 0);
        assert!(overview.expiry_notifications);
        assert!(overview.maintenance_notifications);
        assert_eq!(overview.discount_percent, 0);
    }

    #[test]
    fn verified_email_creates_independent_web_session() {
        let store = Store::open_in_memory();
        store.upsert_user(7, Some("alice"), "Alice", None, 100);
        assert!(store.request_email_code(7, "Alice@Example.ru", "bind", "123456", 101));
        assert!(store.confirm_email_code(7, "alice@example.ru", "bind", "123456", 102));
        assert_eq!(store.portal_email(7).as_deref(), Some("alice@example.ru"));
        assert_eq!(store.verified_user_by_email("ALICE@example.ru"), Some(7));
        let session = store.create_portal_session(7, 103).unwrap();
        assert_eq!(store.portal_user_id(&session, 104), Some(7));
    }

    #[test]
    fn email_code_is_rate_limited_and_attempt_limited() {
        let store = Store::open_in_memory();
        store.upsert_user(7, None, "Alice", None, 100);
        assert!(store.request_email_code(7, "a@example.ru", "bind", "123456", 101));
        assert!(!store.request_email_code(7, "a@example.ru", "bind", "654321", 120));
        for _ in 0..5 {
            assert!(!store.confirm_email_code(7, "a@example.ru", "bind", "000000", 130));
        }
        assert!(!store.confirm_email_code(7, "a@example.ru", "bind", "123456", 131));
    }
}
