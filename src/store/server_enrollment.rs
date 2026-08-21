use rand::Rng;
use sha2::{Digest, Sha256};

use crate::store::Store;

pub const ENROLLMENT_TTL_SECS: i64 = 30 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrollmentIssue {
    pub token: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollmentStatus {
    Accepted(i64),
    Invalid,
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(DIGITS[(byte >> 4) as usize] as char);
        result.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    result
}

fn token_hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

fn new_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    format!("awn_{}", hex(&bytes))
}

impl Store {
    pub fn create_server_enrollment(
        &self,
        server_id: i64,
        actor: i64,
        now: i64,
    ) -> Option<EnrollmentIssue> {
        let token = new_token();
        let hash = token_hash(&token);
        let expires_at = now + ENROLLMENT_TTL_SECS;
        self.with_conn(|c| {
            let tx = c.unchecked_transaction()?;
            tx.execute(
                "UPDATE server_enrollments SET revoked_at=?2 WHERE server_id=?1 AND used_at IS NULL AND revoked_at IS NULL",
                rusqlite::params![server_id, now],
            )?;
            tx.execute(
                "INSERT INTO server_enrollments(server_id,token_hash,created_by,created_at,expires_at) VALUES(?1,?2,?3,?4,?5)",
                rusqlite::params![server_id, hash, actor, now, expires_at],
            )?;
            tx.commit()
        })
        .ok()?;
        Some(EnrollmentIssue { token, expires_at })
    }

    pub fn redeem_server_enrollment(&self, token: &str, now: i64) -> EnrollmentStatus {
        let hash = token_hash(token);
        self.with_conn(|c| {
            let tx = c.unchecked_transaction()?;
            let server_id = tx.query_row(
                "SELECT server_id FROM server_enrollments WHERE token_hash=?1 AND used_at IS NULL AND revoked_at IS NULL AND expires_at>?2",
                rusqlite::params![hash, now],
                |row| row.get::<_, i64>(0),
            )?;
            let changed = tx.execute(
                "UPDATE server_enrollments SET used_at=?2 WHERE token_hash=?1 AND used_at IS NULL",
                rusqlite::params![hash, now],
            )?;
            tx.commit()?;
            Ok((server_id, changed))
        })
        .ok()
        .filter(|(_, changed)| *changed == 1)
        .map_or(EnrollmentStatus::Invalid, |(server_id, _)| {
            EnrollmentStatus::Accepted(server_id)
        })
    }

    pub fn revoke_server_enrollments(&self, server_id: i64, now: i64) -> bool {
        self.with_conn(|c| {
            c.execute(
                "UPDATE server_enrollments SET revoked_at=?2 WHERE server_id=?1 AND used_at IS NULL AND revoked_at IS NULL",
                rusqlite::params![server_id, now],
            )
        })
        .is_ok_and(|changed| changed > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::NewVpnServer;

    fn server(store: &Store) -> i64 {
        store
            .add_vpn_server(
                &NewVpnServer {
                    name: "Remote",
                    hostname: "remote.example.com",
                    public_ip: "192.0.2.20",
                    provider: "Hoster",
                    location: "Amsterdam",
                    protocol: "modern",
                    opened_at: None,
                    is_local: false,
                },
                1,
                10,
            )
            .unwrap()
    }

    #[test]
    fn enrollment_is_one_time_and_not_stored_in_plaintext() {
        let store = Store::open_in_memory();
        let server_id = server(&store);
        let issue = store.create_server_enrollment(server_id, 1, 100).unwrap();
        assert!(issue.token.starts_with("awn_"));
        assert!(!store
            .with_conn(|c| c
                .query_row("SELECT token_hash FROM server_enrollments", [], |r| r
                    .get::<_, String>(0)))
            .unwrap()
            .contains(&issue.token));
        assert_eq!(
            store.redeem_server_enrollment(&issue.token, 200),
            EnrollmentStatus::Accepted(server_id)
        );
        assert_eq!(
            store.redeem_server_enrollment(&issue.token, 201),
            EnrollmentStatus::Invalid
        );
    }

    #[test]
    fn replacement_revokes_old_token_and_expiry_is_enforced() {
        let store = Store::open_in_memory();
        let server_id = server(&store);
        let old = store.create_server_enrollment(server_id, 1, 100).unwrap();
        let new = store.create_server_enrollment(server_id, 1, 200).unwrap();
        assert_eq!(
            store.redeem_server_enrollment(&old.token, 300),
            EnrollmentStatus::Invalid
        );
        assert_eq!(
            store.redeem_server_enrollment(&new.token, new.expires_at),
            EnrollmentStatus::Invalid
        );
    }
}
