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

#[derive(Debug, Clone)]
pub struct SupportTicket {
    pub id: i64,
    pub user_id: i64,
    pub status: String,
    pub subject: String,
    pub assigned_to: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Store {
    pub fn set_device_label(&self, name: &str, user_id: i64, label: &str) -> bool {
        let label = label.trim();
        if label.is_empty()
            || label.chars().count() > 40
            || self.client_owner(name) != Some(user_id)
        {
            return false;
        }
        self.with_conn(|c| c.execute("UPDATE clients SET device_label=?3 WHERE name=?1 AND owner_user_id=?2 AND removed_at IS NULL", rusqlite::params![name,user_id,label]))
            .map(|n| n==1).unwrap_or(false)
    }

    pub fn device_label(&self, name: &str) -> Option<String> {
        self.with_conn(|c| {
            c.query_row(
                "SELECT device_label FROM clients WHERE name=?1 AND removed_at IS NULL",
                [name],
                |r| r.get(0),
            )
            .optional()
        })
        .ok()
        .flatten()
        .flatten()
    }

    pub fn support_tickets(&self, status: &str, limit: usize) -> Vec<SupportTicket> {
        self.with_conn(|c| { let mut s=c.prepare("SELECT id,user_id,status,subject,assigned_to,created_at,updated_at FROM support_tickets WHERE status=?1 ORDER BY updated_at DESC LIMIT ?2")?; let rows=s.query_map(rusqlite::params![status,limit as i64], ticket_from_row)?; rows.collect() }).unwrap_or_default()
    }

    pub fn support_ticket(&self, id: i64) -> Option<SupportTicket> {
        self.with_conn(|c| c.query_row("SELECT id,user_id,status,subject,assigned_to,created_at,updated_at FROM support_tickets WHERE id=?1",[id],ticket_from_row).optional()).ok().flatten()
    }

    pub fn assign_support_ticket(&self, id: i64, admin_id: i64, now: i64) -> bool {
        self.with_conn(|c| c.execute("UPDATE support_tickets SET status='in_progress',assigned_to=?2,updated_at=?3 WHERE id=?1 AND status!='closed'",rusqlite::params![id,admin_id,now])).map(|n|n==1).unwrap_or(false)
    }

    pub fn close_support_ticket(&self, id: i64, admin_id: i64, now: i64) -> bool {
        self.with_conn(|c| c.execute("UPDATE support_tickets SET status='closed',closed_at=?3,closed_by=?2,updated_at=?3 WHERE id=?1 AND status!='closed'",rusqlite::params![id,admin_id,now])).map(|n|n==1).unwrap_or(false)
    }

    pub fn add_support_message(
        &self,
        ticket_id: i64,
        sender: i64,
        is_admin: bool,
        telegram_message: (i64, i32),
        text: Option<&str>,
        now: i64,
    ) {
        let (chat, message) = telegram_message;
        let _=self.with_conn(|c| c.execute("INSERT INTO support_messages(ticket_id,sender_user_id,is_admin,telegram_chat_id,telegram_message_id,text,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",rusqlite::params![ticket_id,sender,if is_admin {1}else{0},chat,message,text,now]));
    }
    pub fn set_auto_renew(
        &self,
        client_name: &str,
        user_id: i64,
        months: i64,
        enabled: bool,
        now: i64,
    ) -> bool {
        if !matches!(months, 1 | 3 | 6 | 12) || self.client_owner(client_name) != Some(user_id) {
            return false;
        }
        self.with_conn(|c| c.execute(
            "INSERT INTO client_subscriptions(client_name,user_id,months,auto_renew,updated_at)
             VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(client_name) DO UPDATE SET user_id=?2,months=?3,auto_renew=?4,updated_at=?5",
            rusqlite::params![client_name,user_id,months,if enabled { 1_i64 } else { 0_i64 },now]
        )).map(|n| n == 1).unwrap_or(false)
    }

    pub fn auto_renew(&self, client_name: &str, user_id: i64) -> Option<(i64, bool)> {
        self.with_conn(|c| c.query_row(
            "SELECT months,auto_renew FROM client_subscriptions WHERE client_name=?1 AND user_id=?2",
            rusqlite::params![client_name,user_id], |r| Ok((r.get(0)?, r.get::<_,i64>(1)? != 0)))
            .optional()).ok().flatten()
    }

    pub fn auto_renew_clients(&self) -> Vec<(String, i64, i64)> {
        self.with_conn(|c| {
            let mut s = c.prepare(
                "SELECT client_name,user_id,months FROM client_subscriptions WHERE auto_renew=1",
            )?;
            let rows = s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            rows.collect()
        })
        .unwrap_or_default()
    }

    pub fn claim_renewal_attempt(&self, client_name: &str, expires_at: i64, now: i64) -> bool {
        self.with_conn(|c| c.execute("INSERT OR IGNORE INTO renewal_attempts(client_name,expires_at,status,created_at) VALUES(?1,?2,'processing',?3)", rusqlite::params![client_name,expires_at,now]))
            .map(|n| n==1).unwrap_or(false)
    }

    pub fn finish_renewal_attempt(&self, client_name: &str, expires_at: i64, status: &str) {
        let _ = self.with_conn(|c| {
            c.execute(
                "UPDATE renewal_attempts SET status=?3 WHERE client_name=?1 AND expires_at=?2",
                rusqlite::params![client_name, expires_at, status],
            )
        });
    }
    /// Атомарно резервирует единственный пробный период для Telegram ID.
    pub fn claim_trial(&self, user_id: i64, now: i64) -> bool {
        self.with_conn(|c| {
            c.execute(
                "UPDATE users SET trial_claimed_at=?2
                 WHERE user_id=?1 AND trial_claimed_at IS NULL
                   AND NOT EXISTS(SELECT 1 FROM clients WHERE owner_user_id=?1)",
                rusqlite::params![user_id, now],
            )
        })
        .map(|n| n == 1)
        .unwrap_or(false)
    }

    pub fn release_trial_claim(&self, user_id: i64, claimed_at: i64) {
        let _ = self.with_conn(|c| {
            c.execute(
                "UPDATE users SET trial_claimed_at=NULL
                 WHERE user_id=?1 AND trial_claimed_at=?2
                   AND NOT EXISTS(SELECT 1 FROM clients WHERE owner_user_id=?1)",
                rusqlite::params![user_id, claimed_at],
            )
        });
    }

    pub fn referral_count(&self, user_id: i64) -> i64 {
        self.with_conn(|c| {
            c.query_row(
                "SELECT COUNT(*) FROM users WHERE referrer_id=?1",
                [user_id],
                |r| r.get(0),
            )
        })
        .unwrap_or(0)
    }

    pub fn all_user_ids(&self) -> Vec<i64> {
        self.with_conn(|c| {
            let mut stmt = c.prepare("SELECT user_id FROM users ORDER BY user_id")?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            rows.collect()
        })
        .unwrap_or_default()
    }

    /// Возвращает true только для ещё не отправленного порога данного срока.
    pub fn mark_expiry_notification(
        &self,
        client_name: &str,
        owner_user_id: i64,
        expires_at: i64,
        threshold_days: i64,
        sent_at: i64,
    ) -> bool {
        self.with_conn(|c| {
            c.execute(
                "INSERT OR IGNORE INTO expiry_notifications
                 (client_name,owner_user_id,expires_at,threshold_days,sent_at)
                 VALUES(?1,?2,?3,?4,?5)",
                rusqlite::params![
                    client_name,
                    owner_user_id,
                    expires_at,
                    threshold_days,
                    sent_at
                ],
            )
        })
        .map(|n| n == 1)
        .unwrap_or(false)
    }

    pub fn unmark_expiry_notification(
        &self,
        client_name: &str,
        expires_at: i64,
        threshold_days: i64,
    ) {
        let _ = self.with_conn(|c| {
            c.execute(
                "DELETE FROM expiry_notifications
                 WHERE client_name=?1 AND expires_at=?2 AND threshold_days=?3",
                rusqlite::params![client_name, expires_at, threshold_days],
            )
        });
    }

    pub fn open_support_ticket(&self, user_id: i64, subject: &str, now: i64) -> Option<i64> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO support_tickets(user_id,subject,created_at,updated_at)
                 VALUES(?1,?2,?3,?3)",
                rusqlite::params![user_id, subject, now],
            )?;
            Ok(c.last_insert_rowid())
        })
        .ok()
    }

    pub fn open_support_count(&self) -> i64 {
        self.with_conn(|c| {
            c.query_row(
                "SELECT COUNT(*) FROM support_tickets WHERE status='open'",
                [],
                |r| r.get(0),
            )
        })
        .unwrap_or(0)
    }

    pub fn recent_payments(&self, limit: usize) -> Vec<PaymentRequest> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT id,user_id,months,amount_kopecks,method,status,proof,client_name,created_at
                 FROM payment_requests ORDER BY created_at DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map([limit as i64], payment_from_row)?;
            rows.collect()
        })
        .unwrap_or_default()
    }

    pub fn approved_revenue_kopecks(&self) -> i64 {
        self.with_conn(|c| {
            c.query_row(
                "SELECT COALESCE(SUM(amount_kopecks),0) FROM payment_requests
             WHERE status='approved' AND months>0",
                [],
                |r| r.get(0),
            )
        })
        .unwrap_or(0)
    }
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

    pub fn create_renewal_request(
        &self,
        user_id: i64,
        client_name: &str,
        months: i64,
        amount_kopecks: i64,
        now: i64,
    ) -> Option<i64> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO payment_requests(user_id,months,amount_kopecks,method,client_name,created_at)
                 VALUES(?1,?2,?3,'manual',?4,?5)",
                rusqlite::params![user_id, months, amount_kopecks, client_name, now],
            )?;
            Ok(c.last_insert_rowid())
        }).ok()
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

fn ticket_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SupportTicket> {
    Ok(SupportTicket {
        id: r.get(0)?,
        user_id: r.get(1)?,
        status: r.get(2)?,
        subject: r.get(3)?,
        assigned_to: r.get(4)?,
        created_at: r.get(5)?,
        updated_at: r.get(6)?,
    })
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

    #[test]
    fn trial_is_one_shot_and_referrals_are_counted() {
        let s = Store::open_in_memory();
        s.upsert_user(1, Some("alice"), "Alice", None, 10);
        s.upsert_user(2, Some("bob"), "Bob", Some(1), 11);
        assert_eq!(s.referral_count(1), 1);
        assert!(s.claim_trial(2, 12));
        assert!(!s.claim_trial(2, 13));
    }

    #[test]
    fn expiry_notification_is_idempotent_per_expiry_and_threshold() {
        let s = Store::open_in_memory();
        s.upsert_user(1, None, "Alice", None, 10);
        assert!(s.mark_expiry_notification("alice_01", 1, 1000, 7, 20));
        assert!(!s.mark_expiry_notification("alice_01", 1, 1000, 7, 21));
        assert!(s.mark_expiry_notification("alice_01", 1, 1000, 3, 22));
        assert!(s.mark_expiry_notification("alice_01", 1, 2000, 7, 23));
    }
}
