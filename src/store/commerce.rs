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
    pub server_id: Option<i64>,
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
    pub category: String,
    pub priority: String,
}

#[derive(Debug, Default, Clone)]
pub struct FinanceSummary {
    pub approved_sales: i64,
    pub revenue_kopecks: i64,
    pub topups_kopecks: i64,
    pub refunds_kopecks: i64,
    pub pending: i64,
}

#[derive(Debug, Default, Clone)]
pub struct AdminUserStats {
    pub total: i64,
    pub new_today: i64,
    pub new_30d: i64,
    pub paying: i64,
    pub blocked: i64,
    pub referred: i64,
}

#[derive(Debug, Clone)]
pub struct AdminUserProfile {
    pub user: UserRow,
    pub blocked: bool,
    pub admin_note: Option<String>,
    pub last_seen: i64,
    pub balance_kopecks: i64,
    pub key_count: i64,
    pub payment_count: i64,
    pub spent_kopecks: i64,
    pub referral_count: i64,
    pub ticket_count: i64,
}

#[derive(Debug, Clone)]
pub struct PromoCode {
    pub code: String,
    pub discount_percent: i64,
    pub max_uses: Option<i64>,
    pub used_count: i64,
    pub expires_at: Option<i64>,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct LegacyRequest {
    pub id: i64,
    pub user_id: i64,
    pub requested_name: String,
    pub comment: Option<String>,
    pub status: String,
    pub created_at: i64,
    pub client_name: Option<String>,
}

impl Store {
    pub fn create_key_replacement(
        &self,
        user_id: i64,
        old: &str,
        new: &str,
        server_id: i64,
        now: i64,
    ) -> Option<i64> {
        self.with_conn(|c| {
            c.execute("INSERT INTO key_replacements(user_id,old_client,new_client,target_server_id,created_at) VALUES(?1,?2,?3,?4,?5)",rusqlite::params![user_id,old,new,server_id,now])?;
            Ok(c.last_insert_rowid())
        }).ok()
    }

    pub fn pending_key_replacement(&self, user_id: i64, old: &str) -> Option<(i64, String, i64)> {
        self.with_conn(|connection| {
            connection
                .query_row(
                    "SELECT id,new_client,target_server_id FROM key_replacements
                     WHERE user_id=?1 AND old_client=?2 AND status='pending'",
                    rusqlite::params![user_id, old],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()
        })
        .ok()
        .flatten()
    }

    pub fn decide_key_replacement(
        &self,
        id: i64,
        user_id: i64,
        status: &str,
        now: i64,
    ) -> Option<(String, String)> {
        if !matches!(status, "confirmed" | "cancelled") {
            return None;
        }
        self.with_conn(|c| {
            let tx=c.unchecked_transaction()?;
            let pair=tx.query_row("SELECT old_client,new_client FROM key_replacements WHERE id=?1 AND user_id=?2 AND status='pending'",rusqlite::params![id,user_id],|r|Ok((r.get(0)?,r.get(1)?)))?;
            tx.execute("UPDATE key_replacements SET status=?3,decided_at=?4 WHERE id=?1 AND user_id=?2 AND status='pending'",rusqlite::params![id,user_id,status,now])?;
            tx.commit()?;
            Ok(pair)
        }).ok()
    }
    /// Резервирует самостоятельное обновление не чаще одного раза в 10 минут.
    pub fn claim_client_self_refresh(&self, name: &str, user_id: i64, now: i64) -> bool {
        if self.client_owner(name) != Some(user_id) {
            return false;
        }
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO client_self_refreshes(client_name,user_id,last_requested_at)
                 VALUES(?1,?2,?3)
                 ON CONFLICT(client_name) DO UPDATE SET
                   user_id=?2,last_requested_at=?3,request_count=request_count+1
                 WHERE client_self_refreshes.user_id=?2
                   AND client_self_refreshes.last_requested_at<=?3-600",
                rusqlite::params![name, user_id, now],
            )
        })
        .is_ok_and(|changed| changed == 1)
    }

    pub fn release_client_self_refresh(&self, name: &str, user_id: i64, claimed_at: i64) {
        let _ = self.with_conn(|c| {
            c.execute(
                "DELETE FROM client_self_refreshes
                 WHERE client_name=?1 AND user_id=?2 AND last_requested_at=?3",
                rusqlite::params![name, user_id, claimed_at],
            )
        });
    }

    pub fn admin_user_stats(&self, now: i64) -> AdminUserStats {
        self.with_conn(|c| c.query_row(
            "SELECT COUNT(*),
                    SUM(CASE WHEN created_at>=?1 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN created_at>=?2 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN blocked<>0 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN referrer_id IS NOT NULL THEN 1 ELSE 0 END)
             FROM users",
            rusqlite::params![now-86_400,now-30*86_400],
            |r| Ok(AdminUserStats{total:r.get(0)?,new_today:r.get(1)?,new_30d:r.get(2)?,blocked:r.get(3)?,referred:r.get(4)?,paying:0})
        )).map(|mut s| { s.paying=self.with_conn(|c|c.query_row("SELECT COUNT(DISTINCT user_id) FROM payment_requests WHERE status='approved' AND months>0",[],|r|r.get(0))).unwrap_or(0); s }).unwrap_or_default()
    }

    pub fn admin_user_profile(&self, user_id: i64) -> Option<AdminUserProfile> {
        let user = self.user(user_id)?;
        self.with_conn(|c| c.query_row(
            "SELECT blocked,admin_note,last_seen,
                    (SELECT COUNT(*) FROM clients WHERE owner_user_id=?1 AND removed_at IS NULL),
                    (SELECT COUNT(*) FROM payment_requests WHERE user_id=?1),
                    (SELECT COALESCE(SUM(amount_kopecks),0) FROM payment_requests WHERE user_id=?1 AND status='approved' AND months>0),
                    (SELECT COUNT(*) FROM users WHERE referrer_id=?1),
                    (SELECT COUNT(*) FROM support_tickets WHERE user_id=?1)
             FROM users WHERE user_id=?1", [user_id], |r| Ok((r.get::<_,i64>(0)?!=0,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?))
        )).ok().map(|(blocked,admin_note,last_seen,key_count,payment_count,spent_kopecks,referral_count,ticket_count)| AdminUserProfile{
            user,blocked,admin_note,last_seen,balance_kopecks:self.balance_kopecks(user_id),key_count,payment_count,spent_kopecks,referral_count,ticket_count
        })
    }

    pub fn set_user_blocked(&self, user_id: i64, blocked: bool) -> bool {
        self.with_conn(|c| {
            c.execute(
                "UPDATE users SET blocked=?2 WHERE user_id=?1",
                rusqlite::params![user_id, if blocked { 1 } else { 0 }],
            )
        })
        .map(|n| n == 1)
        .unwrap_or(false)
    }

    pub fn user_blocked(&self, user_id: i64) -> bool {
        self.with_conn(|c| {
            c.query_row(
                "SELECT blocked FROM users WHERE user_id=?1",
                [user_id],
                |r| r.get::<_, i64>(0),
            )
        })
        .is_ok_and(|v| v != 0)
    }

    pub fn set_user_note(&self, user_id: i64, note: Option<&str>) -> bool {
        self.with_conn(|c| {
            c.execute(
                "UPDATE users SET admin_note=?2 WHERE user_id=?1",
                rusqlite::params![user_id, note],
            )
        })
        .map(|n| n == 1)
        .unwrap_or(false)
    }

    pub fn set_client_note(&self, name: &str, note: Option<&str>) -> bool {
        self.with_conn(|c| {
            c.execute(
                "UPDATE clients SET admin_note=?2 WHERE name=?1 AND removed_at IS NULL",
                rusqlite::params![name, note],
            )
        })
        .map(|n| n == 1)
        .unwrap_or(false)
    }

    pub fn client_note(&self, name: &str) -> Option<String> {
        self.with_conn(|c| {
            c.query_row(
                "SELECT admin_note FROM clients WHERE name=?1 AND removed_at IS NULL",
                [name],
                |r| r.get(0),
            )
            .optional()
        })
        .ok()
        .flatten()
        .flatten()
    }

    pub fn user_payments(&self, user_id: i64, limit: usize) -> Vec<PaymentRequest> {
        self.with_conn(|c| { let mut s=c.prepare("SELECT id,user_id,months,amount_kopecks,method,status,proof,client_name,created_at,server_id FROM payment_requests WHERE user_id=?1 ORDER BY created_at DESC LIMIT ?2")?; let rows=s.query_map(rusqlite::params![user_id,limit as i64],payment_from_row)?; rows.collect() }).unwrap_or_default()
    }

    pub fn create_promo(
        &self,
        code: &str,
        percent: i64,
        max_uses: Option<i64>,
        expires_at: Option<i64>,
        actor: i64,
        now: i64,
    ) -> bool {
        let code = code.trim().to_uppercase();
        if code.is_empty() || code.len() > 24 || !(1..=100).contains(&percent) {
            return false;
        }
        self.with_conn(|c|c.execute("INSERT INTO promo_codes(code,discount_percent,max_uses,expires_at,created_by,created_at) VALUES(?1,?2,?3,?4,?5,?6)",rusqlite::params![code,percent,max_uses,expires_at,actor,now])).map(|n|n==1).unwrap_or(false)
    }

    pub fn create_legacy_promo(
        &self,
        code: &str,
        max_uses: Option<i64>,
        actor: i64,
        now: i64,
    ) -> bool {
        let code = code.trim().to_uppercase();
        if code.is_empty() || code.len() > 24 || max_uses.is_some_and(|v| v <= 0) {
            return false;
        }
        self.with_conn(|c|c.execute("INSERT INTO promo_codes(code,discount_percent,max_uses,expires_at,created_by,created_at,kind) VALUES(?1,100,?2,?3,?4,?5,'legacy')",rusqlite::params![code,max_uses,crate::calendar::LEGACY_REQUEST_DEADLINE-1,actor,now])).map(|n|n==1).unwrap_or(false)
    }

    pub fn activate_legacy_promo(&self, user_id: i64, code: &str, now: i64) -> bool {
        if !crate::calendar::legacy_requests_open(now) {
            return false;
        }
        let code = code.trim().to_uppercase();
        self.with_conn(|c| {
            c.execute_batch("BEGIN IMMEDIATE")?;
            let result = (|| {
                c.query_row(
                    "SELECT 1 FROM promo_codes WHERE code=?1 COLLATE NOCASE AND kind='legacy' AND active=1 AND (expires_at IS NULL OR expires_at>=?2) AND (max_uses IS NULL OR used_count<max_uses)",
                    rusqlite::params![code,now],
                    |_| Ok(()),
                )?;
                c.execute("INSERT INTO promo_uses(code,user_id,used_at) VALUES(?1,?2,?3)",rusqlite::params![code,user_id,now])?;
                c.execute("INSERT INTO legacy_entitlements(user_id,promo_code,activated_at,expires_at) VALUES(?1,?2,?3,?4)",rusqlite::params![user_id,code,now,crate::calendar::LEGACY_RESTORE_DEADLINE])?;
                c.execute("UPDATE promo_codes SET used_count=used_count+1 WHERE code=?1 COLLATE NOCASE",[&code])?;
                Ok(())
            })();
            if result.is_ok(){c.execute_batch("COMMIT")?;}else{let _=c.execute_batch("ROLLBACK");}
            result
        }).is_ok()
    }

    pub fn consume_legacy_entitlement(&self, user_id: i64, name: &str, now: i64) -> bool {
        if now > crate::calendar::LEGACY_RESTORE_DEADLINE {
            return false;
        }
        self.with_conn(|c| {
            c.execute(
                "UPDATE legacy_entitlements SET used_client=?2,used_at=?3
             WHERE id=(SELECT e.id FROM legacy_entitlements e
                       WHERE e.user_id=?1 AND e.used_at IS NULL AND e.expires_at>=?3
                         AND EXISTS(SELECT 1 FROM clients c WHERE c.name=?2 AND c.owner_user_id=?1)
                       ORDER BY e.activated_at LIMIT 1)",
                rusqlite::params![user_id, name, now],
            )
        })
        .map(|n| n == 1)
        .unwrap_or(false)
    }

    pub fn can_restore_legacy_client(&self, user_id: i64, name: &str, now: i64) -> bool {
        now <= crate::calendar::LEGACY_RESTORE_DEADLINE
            && self
                .with_conn(|c| {
                    c.query_row(
                        "SELECT EXISTS(SELECT 1 FROM legacy_entitlements e
             WHERE e.user_id=?1 AND e.used_at IS NULL AND e.expires_at>=?3)
             AND EXISTS(SELECT 1 FROM clients c WHERE c.name=?2 AND c.owner_user_id=?1)",
                        rusqlite::params![user_id, name, now],
                        |r| r.get::<_, i64>(0),
                    )
                })
                .is_ok_and(|v| v != 0)
    }

    pub fn has_pending_legacy_entitlement(&self, user_id: i64, code: &str, now: i64) -> bool {
        self.with_conn(|c| {
            c.query_row(
                "SELECT EXISTS(SELECT 1 FROM legacy_entitlements
                 WHERE user_id=?1 AND promo_code=?2 COLLATE NOCASE
                   AND used_at IS NULL AND expires_at>=?3)",
                rusqlite::params![user_id, code.trim(), now],
                |r| r.get::<_, i64>(0),
            )
        })
        .is_ok_and(|v| v != 0)
    }

    pub fn legacy_user_eligible(&self, user_id: i64, now: i64) -> bool {
        crate::calendar::legacy_requests_open(now)
            && self
                .with_conn(|c| {
                    c.query_row(
            "SELECT EXISTS(SELECT 1 FROM legacy_entitlements WHERE user_id=?1 AND expires_at>=?2)",
            rusqlite::params![user_id,now],|r|r.get::<_,i64>(0)
        )
                })
                .is_ok_and(|v| v != 0)
    }

    pub fn create_legacy_request(
        &self,
        user_id: i64,
        requested_name: &str,
        comment: Option<&str>,
        now: i64,
    ) -> Option<i64> {
        let requested_name = requested_name.trim();
        if !self.legacy_user_eligible(user_id, now)
            || requested_name.is_empty()
            || requested_name.chars().count() > 64
            || comment.is_some_and(|v| v.chars().count() > 500)
        {
            return None;
        }
        self.with_conn(|c|{c.execute("INSERT INTO legacy_requests(user_id,requested_name,comment,created_at) VALUES(?1,?2,?3,?4)",rusqlite::params![user_id,requested_name,comment,now])?;Ok(c.last_insert_rowid())}).ok()
    }

    pub fn legacy_requests(&self, status: &str, limit: usize) -> Vec<LegacyRequest> {
        self.with_conn(|c|{let mut s=c.prepare("SELECT id,user_id,requested_name,comment,status,created_at,client_name FROM legacy_requests WHERE status=?1 ORDER BY created_at LIMIT ?2")?;let rows=s.query_map(rusqlite::params![status,limit as i64],legacy_request_from_row)?;rows.collect()}).unwrap_or_default()
    }

    pub fn legacy_request(&self, id: i64) -> Option<LegacyRequest> {
        self.with_conn(|c|c.query_row("SELECT id,user_id,requested_name,comment,status,created_at,client_name FROM legacy_requests WHERE id=?1",[id],legacy_request_from_row).optional()).ok().flatten()
    }

    pub fn claim_legacy_request(&self, id: i64, admin_id: i64, now: i64) -> bool {
        self.with_conn(|connection| {
            connection.execute(
                "UPDATE legacy_requests SET status='processing',decided_by=?2,decided_at=?3
                 WHERE id=?1 AND status='pending'",
                rusqlite::params![id, admin_id, now],
            )
        })
        .is_ok_and(|changed| changed == 1)
    }

    pub fn release_legacy_request_claim(&self, id: i64) {
        let _ = self.with_conn(|connection| {
            connection.execute(
                "UPDATE legacy_requests SET status='pending',decided_by=NULL,decided_at=NULL
                 WHERE id=?1 AND status='processing'",
                [id],
            )
        });
    }

    pub fn decide_legacy_request(
        &self,
        id: i64,
        admin_id: i64,
        client_name: Option<&str>,
        reason: Option<&str>,
        now: i64,
    ) -> bool {
        let status = if client_name.is_some() {
            "approved"
        } else {
            "rejected"
        };
        self.with_conn(|c|c.execute("UPDATE legacy_requests SET status=?2,decided_at=?3,decided_by=?4,client_name=?5,reject_reason=?6 WHERE id=?1 AND status IN ('pending','processing')",rusqlite::params![id,status,now,admin_id,client_name,reason])).map(|n|n==1).unwrap_or(false)
    }

    pub fn mark_legacy_subscription(&self, name: &str, user_id: i64, now: i64) -> bool {
        self.with_conn(|c|c.execute(
            "INSERT INTO client_subscriptions(client_name,user_id,months,auto_renew,updated_at,legacy)
             VALUES(?1,?2,12,0,?3,1)
             ON CONFLICT(client_name) DO UPDATE SET user_id=?2,months=12,auto_renew=0,updated_at=?3,legacy=1",
            rusqlite::params![name,user_id,now]
        )).map(|n|n==1).unwrap_or(false)
    }

    pub fn is_legacy_client(&self, name: &str, user_id: i64) -> bool {
        self.with_conn(|c| {
            c.query_row(
                "SELECT legacy FROM client_subscriptions WHERE client_name=?1 AND user_id=?2",
                rusqlite::params![name, user_id],
                |r| r.get::<_, i64>(0),
            )
        })
        .is_ok_and(|v| v != 0)
    }

    pub fn legacy_clients(&self) -> Vec<(String, i64)> {
        self.with_conn(|c| {
            let mut s =
                c.prepare("SELECT client_name,user_id FROM client_subscriptions WHERE legacy=1")?;
            let rows = s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect()
        })
        .unwrap_or_default()
    }

    pub fn promo(&self, code: &str, now: i64) -> Option<PromoCode> {
        self.with_conn(|c|c.query_row("SELECT code,discount_percent,max_uses,used_count,expires_at,active FROM promo_codes WHERE code=?1 COLLATE NOCASE AND kind='discount' AND active=1 AND (expires_at IS NULL OR expires_at>?2) AND (max_uses IS NULL OR used_count<max_uses)",rusqlite::params![code.trim(),now],|r|Ok(PromoCode{code:r.get(0)?,discount_percent:r.get(1)?,max_uses:r.get(2)?,used_count:r.get(3)?,expires_at:r.get(4)?,active:r.get::<_,i64>(5)?!=0})).optional()).ok().flatten()
    }

    pub fn activate_promo(&self, user_id: i64, code: &str, now: i64) -> Option<i64> {
        let code = code.trim().to_uppercase();
        self.with_conn(|c|{
            c.execute_batch("BEGIN IMMEDIATE")?;
            let result=(||{
                let discount:i64=c.query_row("SELECT discount_percent FROM promo_codes WHERE code=?1 COLLATE NOCASE AND kind='discount' AND active=1 AND (expires_at IS NULL OR expires_at>?2) AND (max_uses IS NULL OR used_count<max_uses)",rusqlite::params![code,now],|r|r.get(0))?;
                c.execute("INSERT INTO promo_uses(code,user_id,used_at) VALUES(?1,?2,?3)",rusqlite::params![code,user_id,now])?;
                c.execute("UPDATE promo_codes SET used_count=used_count+1 WHERE code=?1 COLLATE NOCASE",[&code])?;
                c.execute("UPDATE users SET promo_discount=?2 WHERE user_id=?1",rusqlite::params![user_id,discount])?;
                Ok(discount)
            })();
            if result.is_ok(){c.execute_batch("COMMIT")?;}else{let _=c.execute_batch("ROLLBACK");}
            result
        }).ok()
    }

    pub fn take_promo_discount(&self, user_id: i64) -> i64 {
        self.with_conn(|c| {
            let value: Option<i64> = c.query_row(
                "SELECT promo_discount FROM users WHERE user_id=?1",
                [user_id],
                |r| r.get(0),
            )?;
            if value.is_some() {
                c.execute(
                    "UPDATE users SET promo_discount=NULL WHERE user_id=?1",
                    [user_id],
                )?;
            }
            Ok(value.unwrap_or(0))
        })
        .unwrap_or(0)
    }

    /// Наибольшая применимая скидка: постоянная/срочная персональная либо
    /// одноразовый промокод. Одноразовая скидка расходуется только здесь.
    pub fn purchase_discount(&self, user_id: i64, now: i64) -> i64 {
        let personal = self
            .with_conn(|c| {
                c.query_row(
                    "SELECT personal_discount,personal_discount_until FROM users WHERE user_id=?1",
                    [user_id],
                    |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)),
                )
            })
            .ok()
            .and_then(|(discount, until)| {
                discount.filter(|_| until.is_none_or(|expires| expires > now))
            })
            .unwrap_or(0);
        personal
            .max(self.take_promo_discount(user_id))
            .clamp(0, 100)
    }

    pub fn set_personal_discount(
        &self,
        user_id: i64,
        discount: Option<i64>,
        until: Option<i64>,
    ) -> bool {
        if discount.is_some_and(|value| !(0..=100).contains(&value)) {
            return false;
        }
        self.with_conn(|c| {
            c.execute(
                "UPDATE users SET personal_discount=?2,personal_discount_until=?3 WHERE user_id=?1",
                rusqlite::params![user_id, discount, until],
            )
        })
        .is_ok_and(|changed| changed == 1)
    }

    pub fn personal_discount(&self, user_id: i64, now: i64) -> Option<(i64, Option<i64>)> {
        self.with_conn(|c| {
            c.query_row(
                "SELECT personal_discount,personal_discount_until FROM users WHERE user_id=?1",
                [user_id],
                |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)),
            )
        })
        .ok()
        .and_then(|(discount, until)| {
            discount
                .filter(|_| until.is_none_or(|expires| expires > now))
                .map(|value| (value, until))
        })
    }

    pub fn set_purchase_server(&self, user_id: i64, server_id: i64, now: i64) -> bool {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO purchase_preferences(user_id,server_id,updated_at) VALUES(?1,?2,?3)
                 ON CONFLICT(user_id) DO UPDATE SET server_id=?2,updated_at=?3",
                rusqlite::params![user_id, server_id, now],
            )
        })
        .is_ok()
    }

    pub fn purchase_server(&self, user_id: i64) -> Option<i64> {
        self.with_conn(|c| {
            c.query_row(
                "SELECT server_id FROM purchase_preferences WHERE user_id=?1",
                [user_id],
                |row| row.get(0),
            )
            .optional()
        })
        .ok()
        .flatten()
    }

    pub fn legacy_renewal_price_for_user(&self, user_id: i64, base: i64) -> i64 {
        let count = self
            .legacy_clients()
            .into_iter()
            .filter(|(_, owner)| *owner == user_id)
            .count();
        match count {
            10.. => 50_000,
            6..=9 => 75_000,
            _ => base,
        }
    }

    pub fn set_subscription_pause(
        &self,
        name: &str,
        grace_until: Option<i64>,
        frozen_until: Option<i64>,
        now: i64,
    ) -> bool {
        self.with_conn(|c|c.execute("UPDATE client_subscriptions SET grace_until=?2,frozen_until=?3,updated_at=?4 WHERE client_name=?1",rusqlite::params![name,grace_until,frozen_until,now])).map(|n|n==1).unwrap_or(false)
    }
    pub fn search_clients(
        &self,
        query: &str,
        limit: usize,
    ) -> Vec<(String, Option<i64>, Option<String>)> {
        let like = format!("%{}%", query.trim().replace('%', "\\%").replace('_', "\\_"));
        self.with_conn(|c| { let mut s=c.prepare(
            "SELECT c.name,c.owner_user_id,c.device_label FROM clients c LEFT JOIN users u ON u.user_id=c.owner_user_id
             WHERE c.removed_at IS NULL AND (c.name LIKE ?1 ESCAPE '\\' OR c.device_label LIKE ?1 ESCAPE '\\' OR u.username LIKE ?1 ESCAPE '\\' OR CAST(c.owner_user_id AS TEXT)=?2)
             ORDER BY c.name LIMIT ?3")?; let rows=s.query_map(rusqlite::params![like,query.trim(),limit as i64],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?; rows.collect() }).unwrap_or_default()
    }

    /// Возвращает true, только если состояние компонента изменилось.
    pub fn update_monitor_state(
        &self,
        component: &str,
        status: &str,
        details: Option<&str>,
        now: i64,
    ) -> bool {
        self.with_conn(|c| { let old:Option<String>=c.query_row("SELECT status FROM monitor_state WHERE component=?1",[component],|r|r.get(0)).optional()?; c.execute("INSERT INTO monitor_state(component,status,details,changed_at,checked_at) VALUES(?1,?2,?3,?4,?4) ON CONFLICT(component) DO UPDATE SET status=?2,details=?3,changed_at=CASE WHEN status<>?2 THEN ?4 ELSE changed_at END,checked_at=?4",rusqlite::params![component,status,details,now])?; Ok(old.as_deref()!=Some(status) && (old.is_some() || status!="ok")) }).unwrap_or(false)
    }

    pub fn backup_database(&self, path: &std::path::Path) -> rusqlite::Result<()> {
        let value = path.to_string_lossy().into_owned();
        self.with_conn(|c| {
            c.execute("VACUUM INTO ?1", [value])?;
            Ok(())
        })
    }
    pub fn staff_role(&self, user_id: i64) -> Option<String> {
        self.with_conn(|c| {
            c.query_row(
                "SELECT role FROM staff_roles WHERE user_id=?1",
                [user_id],
                |r| r.get(0),
            )
            .optional()
        })
        .ok()
        .flatten()
    }

    pub fn set_staff_role(
        &self,
        user_id: i64,
        role: Option<&str>,
        granted_by: i64,
        now: i64,
    ) -> bool {
        if let Some(role) = role {
            if !matches!(role, "technical" | "support" | "finance") {
                return false;
            }
            self.with_conn(|c|c.execute("INSERT INTO staff_roles(user_id,role,granted_by,granted_at) VALUES(?1,?2,?3,?4) ON CONFLICT(user_id) DO UPDATE SET role=?2,granted_by=?3,granted_at=?4",rusqlite::params![user_id,role,granted_by,now])).map(|n|n==1).unwrap_or(false)
        } else {
            self.with_conn(|c| c.execute("DELETE FROM staff_roles WHERE user_id=?1", [user_id]))
                .map(|n| n == 1)
                .unwrap_or(false)
        }
    }
    pub fn finance_summary(&self, since: i64) -> FinanceSummary {
        self.with_conn(|c| {
            let (approved_sales,revenue,pending):(i64,i64,i64)=c.query_row(
                "SELECT COALESCE(SUM(CASE WHEN status='approved' AND months>0 THEN 1 ELSE 0 END),0),
                        COALESCE(SUM(CASE WHEN status='approved' AND months>0 THEN amount_kopecks ELSE 0 END),0),
                        COALESCE(SUM(CASE WHEN status='pending' THEN 1 ELSE 0 END),0)
                 FROM payment_requests WHERE created_at>=?1",[since],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
            let (topups,refunds):(i64,i64)=c.query_row(
                "SELECT COALESCE(SUM(CASE WHEN kind='topup' THEN amount_kopecks ELSE 0 END),0),
                        COALESCE(SUM(CASE WHEN kind='refund' THEN amount_kopecks ELSE 0 END),0)
                 FROM balance_ledger WHERE created_at>=?1",[since],|r|Ok((r.get(0)?,r.get(1)?)))?;
            Ok(FinanceSummary{approved_sales,revenue_kopecks:revenue,topups_kopecks:topups,refunds_kopecks:refunds,pending})
        }).unwrap_or_default()
    }

    pub fn payments_csv(&self) -> String {
        let rows = self.recent_payments(100_000);
        let mut out =
            "id,user_id,months,amount_rub,method,status,client_name,created_at\n".to_string();
        for p in rows {
            let client = p.client_name.unwrap_or_default().replace('"', "\"\"");
            out.push_str(&format!(
                "{},{},{},{:.2},\"{}\",{:?},\"{}\",{}\n",
                p.id,
                p.user_id,
                p.months,
                p.amount_kopecks as f64 / 100.0,
                p.method.replace('"', "\"\""),
                p.status,
                client,
                p.created_at
            ));
        }
        out
    }
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
        self.with_conn(|c| { let mut s=c.prepare("SELECT id,user_id,status,subject,assigned_to,created_at,updated_at,category,priority FROM support_tickets WHERE status=?1 ORDER BY CASE priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 ELSE 2 END,updated_at DESC LIMIT ?2")?; let rows=s.query_map(rusqlite::params![status,limit as i64], ticket_from_row)?; rows.collect() }).unwrap_or_default()
    }

    pub fn support_ticket(&self, id: i64) -> Option<SupportTicket> {
        self.with_conn(|c| c.query_row("SELECT id,user_id,status,subject,assigned_to,created_at,updated_at,category,priority FROM support_tickets WHERE id=?1",[id],ticket_from_row).optional()).ok().flatten()
    }

    pub fn assign_support_ticket(&self, id: i64, admin_id: i64, now: i64) -> bool {
        self.with_conn(|c| c.execute("UPDATE support_tickets SET status='in_progress',assigned_to=?2,updated_at=?3 WHERE id=?1 AND status!='closed'",rusqlite::params![id,admin_id,now])).map(|n|n==1).unwrap_or(false)
    }

    pub fn close_support_ticket(&self, id: i64, admin_id: i64, now: i64) -> bool {
        self.with_conn(|c| c.execute("UPDATE support_tickets SET status='closed',closed_at=?3,closed_by=?2,updated_at=?3 WHERE id=?1 AND status!='closed'",rusqlite::params![id,admin_id,now])).map(|n|n==1).unwrap_or(false)
    }

    pub fn set_support_priority(&self, id: i64, priority: &str, now: i64) -> bool {
        if !matches!(priority, "normal" | "high" | "urgent") {
            return false;
        }
        self.with_conn(|c|c.execute("UPDATE support_tickets SET priority=?2,updated_at=?3 WHERE id=?1 AND status!='closed'",rusqlite::params![id,priority,now])).map(|n|n==1).unwrap_or(false)
    }

    pub fn rate_support_ticket(&self, id: i64, user_id: i64, rating: i64) -> bool {
        if !(1..=5).contains(&rating) {
            return false;
        }
        self.with_conn(|c|c.execute("UPDATE support_tickets SET rating=?3 WHERE id=?1 AND user_id=?2 AND status='closed' AND rating IS NULL",rusqlite::params![id,user_id,rating])).map(|n|n==1).unwrap_or(false)
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
        self.open_support_ticket_in_category(user_id, "general", subject, now)
    }

    pub fn open_support_ticket_in_category(
        &self,
        user_id: i64,
        category: &str,
        subject: &str,
        now: i64,
    ) -> Option<i64> {
        let category = if matches!(category, "connection" | "payment" | "bug" | "general") {
            category
        } else {
            "general"
        };
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO support_tickets(user_id,subject,category,created_at,updated_at)
                 VALUES(?1,?2,?3,?4,?4)",
                rusqlite::params![user_id, subject, category, now],
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
                "SELECT id,user_id,months,amount_kopecks,method,status,proof,client_name,created_at,server_id
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

    pub fn create_payment_request_on_server(
        &self,
        user_id: i64,
        months: i64,
        amount_kopecks: i64,
        method: &str,
        server_id: i64,
        now: i64,
    ) -> Option<i64> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO payment_requests(user_id,months,amount_kopecks,method,server_id,created_at)
                 VALUES(?1,?2,?3,?4,?5,?6)",
                rusqlite::params![user_id, months, amount_kopecks, method, server_id, now],
            )?;
            Ok(c.last_insert_rowid())
        }).ok()
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

    pub fn create_legacy_renewal_request(
        &self,
        user_id: i64,
        client_name: &str,
        amount_kopecks: i64,
        now: i64,
    ) -> Option<i64> {
        if !self.is_legacy_client(client_name, user_id) {
            return None;
        }
        self.with_conn(|c| {
            let changed=c.execute("INSERT INTO payment_requests(user_id,months,amount_kopecks,method,client_name,created_at)
                SELECT ?1,12,?2,'legacy_manual',?3,?4
                WHERE NOT EXISTS(SELECT 1 FROM payment_requests WHERE user_id=?1 AND client_name=?3 AND method='legacy_manual' AND status='pending')",rusqlite::params![user_id,amount_kopecks,client_name,now])?;
            Ok((changed == 1).then(|| c.last_insert_rowid()))
        }).ok().flatten()
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
                "SELECT id,user_id,months,amount_kopecks,method,status,proof,client_name,created_at,server_id
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
                "SELECT id,user_id,months,amount_kopecks,method,status,proof,client_name,created_at,server_id
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

    pub fn reject_payment(&self, id: i64, admin_id: i64, reason: &str, now: i64) -> bool {
        let reason = reason.trim();
        if reason.is_empty() || reason.chars().count() > 500 {
            return false;
        }
        self.with_conn(|c|c.execute("UPDATE payment_requests SET status='rejected',reject_reason=?2,decided_at=?3,decided_by=?4 WHERE id=?1 AND status='pending'",rusqlite::params![id,reason,now,admin_id])).map(|n|n==1).unwrap_or(false)
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

    pub fn active_client_names(&self) -> Vec<String> {
        self.with_conn(|connection| {
            let mut statement = connection
                .prepare("SELECT name FROM clients WHERE removed_at IS NULL ORDER BY name")?;
            let rows = statement.query_map([], |row| row.get(0))?;
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
        category: r.get(7)?,
        priority: r.get(8)?,
    })
}

fn legacy_request_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<LegacyRequest> {
    Ok(LegacyRequest {
        id: r.get(0)?,
        user_id: r.get(1)?,
        requested_name: r.get(2)?,
        comment: r.get(3)?,
        status: r.get(4)?,
        created_at: r.get(5)?,
        client_name: r.get(6)?,
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
        server_id: r.get(9)?,
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

    #[test]
    fn staff_roles_and_monitor_transitions_are_persistent() {
        let s = Store::open_in_memory();
        assert!(s.set_staff_role(42, Some("support"), 1, 10));
        assert_eq!(s.staff_role(42).as_deref(), Some("support"));
        assert!(!s.update_monitor_state("vpn", "ok", None, 11));
        assert!(s.update_monitor_state("vpn", "error", Some("down"), 12));
        assert!(!s.update_monitor_state("vpn", "error", Some("still down"), 13));
        assert!(s.update_monitor_state("vpn", "ok", None, 14));
        assert!(s.set_staff_role(42, None, 1, 15));
        assert_eq!(s.staff_role(42), None);
    }

    #[test]
    fn database_backup_creates_readable_copy() {
        let s = Store::open_in_memory();
        s.upsert_user(7, None, "User", None, 1);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("backup.db");
        s.backup_database(&path).unwrap();
        let backup = Store::open(&path).unwrap();
        assert!(backup.user(7).is_some());
    }

    #[test]
    fn crm_promo_and_support_category_are_persistent() {
        let s = Store::open_in_memory();
        s.upsert_user(7, Some("alice"), "Alice", None, 10);
        assert!(s.set_user_note(7, Some("VIP")));
        assert!(s.set_user_blocked(7, true));
        let profile = s.admin_user_profile(7).unwrap();
        assert!(profile.blocked);
        assert_eq!(profile.admin_note.as_deref(), Some("VIP"));
        assert!(s.create_promo("FRIEND25", 25, Some(1), None, 1, 20));
        assert_eq!(s.activate_promo(7, "friend25", 21), Some(25));
        assert_eq!(s.take_promo_discount(7), 25);
        assert_eq!(s.take_promo_discount(7), 0);
        assert_eq!(s.activate_promo(7, "FRIEND25", 22), None);
        let id = s
            .open_support_ticket_in_category(7, "payment", "Не прошла оплата", 30)
            .unwrap();
        let ticket = s.support_ticket(id).unwrap();
        assert_eq!(ticket.category, "payment");
        assert_eq!(ticket.priority, "normal");
    }

    #[test]
    fn legacy_promo_enables_unlimited_manually_reviewed_requests() {
        let s = Store::open_in_memory();
        s.upsert_user(7, Some("alice"), "Alice", None, 10);
        s.upsert_user(8, Some("bob"), "Bob", None, 10);
        s.assign_client_group("old_alice", None, 11);
        assert!(s.assign_client_owner("old_alice", Some(7)));
        assert!(s.create_legacy_promo("RESTORE2026", Some(1), 1, 20));
        assert!(s.activate_legacy_promo(7, "restore2026", 21));
        assert!(s.legacy_user_eligible(7, 21));
        assert!(!s.legacy_user_eligible(8, 21));
        let first = s
            .create_legacy_request(7, "phone", Some("первый из двух"), 22)
            .unwrap();
        let second = s.create_legacy_request(7, "laptop", None, 23).unwrap();
        assert_ne!(first, second);
        assert_eq!(s.legacy_requests("pending", 10).len(), 2);
        assert!(s.claim_legacy_request(first, 1, 24));
        assert!(!s.claim_legacy_request(first, 1, 24));
        assert!(s.decide_legacy_request(first, 1, Some("phone_01"), None, 24));
        assert_eq!(s.legacy_request(first).unwrap().status, "approved");
        assert_eq!(s.activate_promo(8, "RESTORE2026", 22), None);
        assert!(s.mark_legacy_subscription("old_alice", 7, 24));
        assert!(s.is_legacy_client("old_alice", 7));
        assert_eq!(s.legacy_clients(), vec![("old_alice".into(), 7)]);
        assert!(s
            .create_legacy_renewal_request(7, "old_alice", 100_000, 25)
            .is_some());
        assert!(s
            .create_legacy_renewal_request(7, "old_alice", 100_000, 26)
            .is_none());
    }

    #[test]
    fn client_self_refresh_has_cooldown_and_owner_check() {
        let s = Store::open_in_memory();
        s.upsert_user(7, Some("alice"), "Alice", None, 1);
        s.upsert_user(8, Some("bob"), "Bob", None, 1);
        s.assign_client_group("phone", None, 1);
        assert!(s.assign_client_owner("phone", Some(7)));
        assert!(s.claim_client_self_refresh("phone", 7, 1_000));
        assert!(!s.claim_client_self_refresh("phone", 7, 1_599));
        assert!(!s.claim_client_self_refresh("phone", 8, 1_600));
        assert!(s.claim_client_self_refresh("phone", 7, 1_600));
        s.release_client_self_refresh("phone", 7, 1_600);
        assert!(s.claim_client_self_refresh("phone", 7, 1_601));
    }
}
