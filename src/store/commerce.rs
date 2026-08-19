use rusqlite::OptionalExtension;

use crate::store::Store;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserRow {
    pub user_id: i64,
    pub username: Option<String>,
    pub display_name: String,
    pub referrer_id: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentStatus {
    Pending,
    Approved,
    Rejected,
}

impl PaymentStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }
    fn parse(value: &str) -> Self {
        match value {
            "approved" => Self::Approved,
            "rejected" => Self::Rejected,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentRequest {
    pub id: i64,
    pub user_id: i64,
    pub months: i64,
    pub amount_kopecks: i64,
    pub method: String,
    pub status: PaymentStatus,
    pub proof: Option<String>,
    pub client_name: Option<String>,
    pub created_at: i64,
}

impl Store {
    /// Регистрирует пользователя и обновляет изменяемые поля Telegram.
    /// Referrer записывается только один раз и не может быть самим пользователем.
    pub fn upsert_user(
        &self,
        user_id: i64,
        username: Option<&str>,
        display_name: &str,
        referrer_id: Option<i64>,
        now: i64,
    ) {
        let referrer = referrer_id.filter(|id| *id != user_id);
        if let Err(e) = self.with_conn(|c| {
            c.execute(
                "INSERT INTO users(user_id,username,display_name,referrer_id,created_at,last_seen)
                 VALUES(?1,?2,?3,?4,?5,?5)
                 ON CONFLICT(user_id) DO UPDATE SET
                   username=?2, display_name=?3, last_seen=?5",
                rusqlite::params![user_id, username, display_name, referrer, now],
            )
        }) {
            tracing::error!(error = %e, user_id, "не удалось сохранить пользователя");
        }
    }

    pub fn user(&self, user_id: i64) -> Option<UserRow> {
        self.with_conn(|c| {
            c.query_row(
                "SELECT user_id,username,display_name,referrer_id,created_at
                 FROM users WHERE user_id=?1",
                [user_id],
                |r| {
                    Ok(UserRow {
                        user_id: r.get(0)?,
                        username: r.get(1)?,
                        display_name: r.get(2)?,
                        referrer_id: r.get(3)?,
                        created_at: r.get(4)?,
                    })
                },
            )
            .optional()
        })
        .ok()
        .flatten()
    }

    pub fn find_user_by_username(&self, username: &str) -> Option<UserRow> {
        let username = username.trim().trim_start_matches('@');
        self.with_conn(|c| {
            c.query_row(
                "SELECT user_id,username,display_name,referrer_id,created_at
                 FROM users WHERE username=?1 COLLATE NOCASE",
                [username],
                |r| {
                    Ok(UserRow {
                        user_id: r.get(0)?,
                        username: r.get(1)?,
                        display_name: r.get(2)?,
                        referrer_id: r.get(3)?,
                        created_at: r.get(4)?,
                    })
                },
            )
            .optional()
        })
        .ok()
        .flatten()
    }

    pub fn balance_kopecks(&self, user_id: i64) -> i64 {
        self.with_conn(|c| {
            c.query_row(
                "SELECT COALESCE(SUM(amount_kopecks),0) FROM balance_ledger WHERE user_id=?1",
                [user_id],
                |r| r.get(0),
            )
        })
        .unwrap_or(0)
    }

    /// Идемпотентная проводка: повторный reference ничего не начисляет.
    pub fn add_ledger_entry(
        &self,
        user_id: i64,
        amount_kopecks: i64,
        kind: &str,
        reference: &str,
        details: Option<&str>,
        now: i64,
    ) -> bool {
        self.with_conn(|c| {
            c.execute(
                "INSERT OR IGNORE INTO balance_ledger
                 (user_id,amount_kopecks,kind,reference,details,created_at)
                 VALUES(?1,?2,?3,?4,?5,?6)",
                rusqlite::params![user_id, amount_kopecks, kind, reference, details, now],
            )
        })
        .map(|n| n == 1)
        .unwrap_or(false)
    }

    pub fn spend_balance(
        &self,
        user_id: i64,
        amount_kopecks: i64,
        reference: &str,
        now: i64,
    ) -> bool {
        if amount_kopecks <= 0 {
            return false;
        }
        self.with_conn(|c| {
            c.execute(
                "INSERT OR IGNORE INTO balance_ledger
                 (user_id,amount_kopecks,kind,reference,created_at)
                 SELECT ?1,-?2,'purchase',?3,?4
                 WHERE (SELECT COALESCE(SUM(amount_kopecks),0)
                        FROM balance_ledger WHERE user_id=?1) >= ?2",
                rusqlite::params![user_id, amount_kopecks, reference, now],
            )
        })
        .map(|n| n == 1)
        .unwrap_or(false)
    }

    pub fn create_payment_request(
        &self,
        user_id: i64,
        months: i64,
        amount_kopecks: i64,
        method: &str,
        now: i64,
    ) -> Option<i64> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO payment_requests(user_id,months,amount_kopecks,method,created_at)
                 VALUES(?1,?2,?3,?4,?5)",
                rusqlite::params![user_id, months, amount_kopecks, method, now],
            )?;
            Ok(c.last_insert_rowid())
        })
        .ok()
    }

    pub fn set_payment_proof(&self, id: i64, user_id: i64, proof: &str) -> bool {
        self.with_conn(|c| {
            c.execute(
                "UPDATE payment_requests SET proof=?3
                 WHERE id=?1 AND user_id=?2 AND status='pending'",
                rusqlite::params![id, user_id, proof],
            )
        })
        .map(|n| n == 1)
        .unwrap_or(false)
    }

    pub fn payment_request(&self, id: i64) -> Option<PaymentRequest> {
        self.with_conn(|c| {
            c.query_row(
                "SELECT id,user_id,months,amount_kopecks,method,status,proof,client_name,created_at
                 FROM payment_requests WHERE id=?1",
                [id],
                payment_from_row,
            )
            .optional()
        })
        .ok()
        .flatten()
    }

    pub fn pending_payments(&self) -> Vec<PaymentRequest> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT id,user_id,months,amount_kopecks,method,status,proof,client_name,created_at
                 FROM payment_requests WHERE status='pending' ORDER BY created_at",
            )?;
            let rows = stmt.query_map([], payment_from_row)?;
            rows.collect()
        })
        .unwrap_or_default()
    }

    pub fn decide_payment(
        &self,
        id: i64,
        status: PaymentStatus,
        admin_id: i64,
        client_name: Option<&str>,
        now: i64,
    ) -> bool {
        if status == PaymentStatus::Pending {
            return false;
        }
        self.with_conn(|c| {
            c.execute(
                "UPDATE payment_requests
                 SET status=?2,decided_at=?3,decided_by=?4,client_name=?5
                 WHERE id=?1 AND status='pending'",
                rusqlite::params![id, status.as_str(), now, admin_id, client_name],
            )
        })
        .map(|n| n == 1)
        .unwrap_or(false)
    }

    pub fn assign_client_owner(&self, name: &str, user_id: Option<i64>) -> bool {
        self.with_conn(|c| {
            c.execute(
                "UPDATE clients SET owner_user_id=?2 WHERE name=?1",
                rusqlite::params![name, user_id],
            )
        })
        .map(|n| n == 1)
        .unwrap_or(false)
    }

    pub fn client_owner(&self, name: &str) -> Option<i64> {
        self.with_conn(|c| {
            c.query_row(
                "SELECT owner_user_id FROM clients WHERE name=?1 AND removed_at IS NULL",
                [name],
                |r| r.get(0),
            )
            .optional()
        })
        .ok()
        .flatten()
        .flatten()
    }

    pub fn user_client_names(&self, user_id: i64) -> Vec<String> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT name FROM clients
                 WHERE owner_user_id=?1 AND removed_at IS NULL ORDER BY name",
            )?;
            let rows = stmt.query_map([user_id], |r| r.get(0))?;
            rows.collect()
        })
        .unwrap_or_default()
    }
}

fn payment_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<PaymentRequest> {
    let status: String = r.get(5)?;
    Ok(PaymentRequest {
        id: r.get(0)?,
        user_id: r.get(1)?,
        months: r.get(2)?,
        amount_kopecks: r.get(3)?,
        method: r.get(4)?,
        status: PaymentStatus::parse(&status),
        proof: r.get(6)?,
        client_name: r.get(7)?,
        created_at: r.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_referrer_is_sticky_and_balance_is_ledger_sum() {
        let s = Store::open_in_memory();
        s.upsert_user(1, Some("alice"), "Alice", None, 10);
        s.upsert_user(2, Some("bob"), "Bob", Some(1), 10);
        s.upsert_user(2, Some("newbob"), "Bob B", None, 20);
        assert_eq!(s.user(2).unwrap().referrer_id, Some(1));
        assert!(s.add_ledger_entry(2, 20_000, "topup", "p:1", None, 30));
        assert!(!s.add_ledger_entry(2, 20_000, "topup", "p:1", None, 30));
        assert!(s.add_ledger_entry(2, -5_000, "purchase", "buy:1", None, 31));
        assert_eq!(s.balance_kopecks(2), 15_000);
    }

    #[test]
    fn payment_decision_is_one_shot() {
        let s = Store::open_in_memory();
        s.upsert_user(1, Some("alice"), "Alice", None, 10);
        let id = s
            .create_payment_request(1, 3, 60_000, "manual", 11)
            .unwrap();
        assert!(s.set_payment_proof(id, 1, "receipt"));
        assert!(s.decide_payment(id, PaymentStatus::Approved, 99, Some("alice_01"), 12));
        assert!(!s.decide_payment(id, PaymentStatus::Rejected, 99, None, 13));
        assert_eq!(
            s.payment_request(id).unwrap().status,
            PaymentStatus::Approved
        );
    }
}
