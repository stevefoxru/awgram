use rusqlite::OptionalExtension;

use super::Store;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Partner {
    pub id: i64,
    pub owner_user_id: i64,
    pub slug: String,
    pub display_name: String,
    pub status: String,
    pub wholesale_discount_percent: i64,
    pub retail_markup_percent: i64,
    pub bot_username: Option<String>,
    pub bot_secret_ref: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartnerOrder {
    pub id: i64,
    pub partner_id: i64,
    pub user_id: i64,
    pub months: i64,
    pub retail_price_kopecks: i64,
    pub wholesale_price_kopecks: i64,
    pub status: String,
    pub fulfilled_client_name: Option<String>,
    pub conf_path: Option<String>,
    pub qr_path: Option<String>,
    pub import_uri: Option<String>,
    pub delivered_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PartnerSalesSummary {
    pub total: i64,
    pub pending: i64,
    pub fulfilled: i64,
    pub delivered: i64,
    pub retail_kopecks: i64,
    pub wholesale_kopecks: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartnerWithdrawal {
    pub id: i64,
    pub partner_id: i64,
    pub amount_kopecks: i64,
    pub requisites: String,
    pub status: String,
    pub created_at: i64,
}

fn withdrawal_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PartnerWithdrawal> {
    Ok(PartnerWithdrawal {
        id: row.get(0)?,
        partner_id: row.get(1)?,
        amount_kopecks: row.get(2)?,
        requisites: row.get(3)?,
        status: row.get(4)?,
        created_at: row.get(5)?,
    })
}

const ORDER_COLUMNS: &str = "id,partner_id,user_id,months,retail_price_kopecks,wholesale_price_kopecks,status,fulfilled_client_name,conf_path,qr_path,import_uri,delivered_at,created_at";

fn order_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PartnerOrder> {
    Ok(PartnerOrder {
        id: row.get(0)?,
        partner_id: row.get(1)?,
        user_id: row.get(2)?,
        months: row.get(3)?,
        retail_price_kopecks: row.get(4)?,
        wholesale_price_kopecks: row.get(5)?,
        status: row.get(6)?,
        fulfilled_client_name: row.get(7)?,
        conf_path: row.get(8)?,
        qr_path: row.get(9)?,
        import_uri: row.get(10)?,
        delivered_at: row.get(11)?,
        created_at: row.get(12)?,
    })
}

const COLUMNS: &str = "id,owner_user_id,slug,display_name,status,wholesale_discount_percent,retail_markup_percent,bot_username,bot_secret_ref,created_at,updated_at";

fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Partner> {
    Ok(Partner {
        id: row.get(0)?,
        owner_user_id: row.get(1)?,
        slug: row.get(2)?,
        display_name: row.get(3)?,
        status: row.get(4)?,
        wholesale_discount_percent: row.get(5)?,
        retail_markup_percent: row.get(6)?,
        bot_username: row.get(7)?,
        bot_secret_ref: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn valid_slug(value: &str) -> bool {
    (3..=32).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
}

impl Store {
    pub fn create_partner_order(
        &self,
        partner_id: i64,
        user_id: i64,
        months: i64,
        now: i64,
    ) -> Result<PartnerOrder, String> {
        if !matches!(months, 1 | 3 | 6 | 12) {
            return Err("доступны сроки 1, 3, 6 или 12 месяцев".into());
        }
        let partner = self.partner(partner_id).ok_or("партнёр не найден")?;
        if partner.status != "active" {
            return Err("партнёрский бот временно приостановлен".into());
        }
        let already_pending = self
            .with_conn(|connection| {
                connection.query_row(
                    "SELECT EXISTS(SELECT 1 FROM partner_orders WHERE partner_id=?1 AND user_id=?2 AND status='pending')",
                    rusqlite::params![partner_id, user_id],
                    |row| row.get::<_, bool>(0),
                )
            })
            .unwrap_or(false);
        if already_pending {
            return Err("у вас уже есть заявка, ожидающая обработки".into());
        }
        let base = self
            .tariff_price_kopecks(months)
            .ok_or("тариф для выбранного срока не настроен")?;
        let percentage = |value: i64, percent: i64| -> Result<i64, String> {
            let result = i128::from(value) * i128::from(percent) / 100;
            i64::try_from(result).map_err(|_| "цена выходит за допустимый диапазон".into())
        };
        let retail = base;
        let commission = self.partner_commission_percent(partner_id, now);
        let wholesale = retail
            .checked_sub(percentage(retail, commission)?)
            .ok_or("цена выходит за допустимый диапазон")?;
        self.with_conn(|connection| {
            connection.execute(
                "INSERT INTO partner_orders(partner_id,user_id,months,retail_price_kopecks,wholesale_price_kopecks,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?6)",
                rusqlite::params![partner_id, user_id, months, retail, wholesale, now],
            )?;
            Ok(PartnerOrder {
                id: connection.last_insert_rowid(), partner_id, user_id, months,
                retail_price_kopecks: retail, wholesale_price_kopecks: wholesale,
                status: "pending".into(), fulfilled_client_name: None, conf_path: None,
                qr_path: None, import_uri: None, delivered_at: None, created_at: now,
            })
        }).map_err(|error| format!("не удалось оформить заказ: {error}"))
    }

    pub fn partner_commission_percent(&self, partner_id: i64, now: i64) -> i64 {
        let sales = self.with_conn(|connection| connection.query_row(
            "SELECT COUNT(*) FROM partner_orders WHERE partner_id=?1 AND status='fulfilled' AND created_at>=?2",
            rusqlite::params![partner_id, now.saturating_sub(30 * 86_400)], |row| row.get::<_, i64>(0))).unwrap_or(0);
        if sales >= 30 {
            30
        } else if sales >= 10 {
            25
        } else {
            20
        }
    }

    pub fn partner_order(&self, id: i64) -> Option<PartnerOrder> {
        self.with_conn(|connection| {
            connection
                .query_row(
                    &format!("SELECT {ORDER_COLUMNS} FROM partner_orders WHERE id=?1"),
                    [id],
                    order_row,
                )
                .optional()
        })
        .ok()
        .flatten()
    }

    pub fn partner_orders(&self, partner_id: i64, limit: usize) -> Vec<PartnerOrder> {
        self.with_conn(|connection| {
            let mut statement = connection.prepare(&format!("SELECT {ORDER_COLUMNS} FROM partner_orders WHERE partner_id=?1 ORDER BY CASE status WHEN 'pending' THEN 0 ELSE 1 END,created_at DESC LIMIT ?2"))?;
            let orders = statement.query_map(rusqlite::params![partner_id, limit as i64], order_row)?.collect();
            orders
        }).unwrap_or_default()
    }

    pub fn partner_customer_orders(
        &self,
        partner_id: i64,
        user_id: i64,
        limit: usize,
    ) -> Vec<PartnerOrder> {
        self.with_conn(|connection| {
            let mut statement = connection.prepare(&format!("SELECT {ORDER_COLUMNS} FROM partner_orders WHERE partner_id=?1 AND user_id=?2 ORDER BY created_at DESC,id DESC LIMIT ?3"))?;
            let orders = statement.query_map(rusqlite::params![partner_id, user_id, limit as i64], order_row)?.collect();
            orders
        }).unwrap_or_default()
    }

    pub fn partner_sales_summary(&self, partner_id: i64) -> PartnerSalesSummary {
        self.with_conn(|connection| connection.query_row(
            "SELECT COUNT(*),COALESCE(SUM(status='pending'),0),COALESCE(SUM(status='fulfilled'),0),COALESCE(SUM(status='fulfilled' AND delivered_at IS NOT NULL),0),COALESCE(SUM(CASE WHEN status='fulfilled' THEN retail_price_kopecks ELSE 0 END),0),COALESCE(SUM(CASE WHEN status='fulfilled' THEN wholesale_price_kopecks ELSE 0 END),0) FROM partner_orders WHERE partner_id=?1",
            [partner_id], |row| Ok(PartnerSalesSummary { total: row.get(0)?, pending: row.get(1)?, fulfilled: row.get(2)?, delivered: row.get(3)?, retail_kopecks: row.get(4)?, wholesale_kopecks: row.get(5)? })))
            .unwrap_or_default()
    }

    pub fn cancel_partner_order(&self, partner_id: i64, user_id: i64, id: i64, now: i64) -> bool {
        self.with_conn(|connection| connection.execute(
            "UPDATE partner_orders SET status='cancelled',updated_at=?4 WHERE id=?1 AND partner_id=?2 AND user_id=?3 AND status='pending'",
            rusqlite::params![id, partner_id, user_id, now]))
            .is_ok_and(|changed| changed == 1)
    }

    pub fn reject_partner_order(&self, id: i64, now: i64) -> bool {
        self.with_conn(|connection| connection.execute(
            "UPDATE partner_orders SET status='rejected',updated_at=?2 WHERE id=?1 AND status='pending'", rusqlite::params![id, now]))
            .is_ok_and(|changed| changed == 1)
    }

    pub fn fulfill_partner_order(
        &self,
        id: i64,
        client: &str,
        conf: &str,
        qr: &str,
        uri: &str,
        now: i64,
    ) -> bool {
        self.with_conn(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let changed = transaction.execute(
                "UPDATE partner_orders SET status='fulfilled',fulfilled_client_name=?2,conf_path=?3,qr_path=?4,import_uri=?5,updated_at=?6 WHERE id=?1 AND status='pending'",
                rusqlite::params![id, client, conf, qr, uri, now])?;
            if changed != 1 { return Ok(false); }
            transaction.execute(
                "INSERT INTO partner_ledger(partner_id,amount_kopecks,kind,reference,available_at,details,created_at) SELECT partner_id,retail_price_kopecks-wholesale_price_kopecks,'sale','partner-order:'||id,?2+604800,'Заказ #'||id,?2 FROM partner_orders WHERE id=?1",
                rusqlite::params![id, now])?;
            transaction.commit()?;
            Ok(true)
        }).unwrap_or(false)
    }

    pub fn partner_balance_kopecks(&self, partner_id: i64, now: i64) -> i64 {
        self.with_conn(|connection| connection.query_row(
            "SELECT COALESCE(SUM(amount_kopecks),0) FROM partner_ledger WHERE partner_id=?1 AND available_at<=?2",
            rusqlite::params![partner_id, now], |row| row.get(0))).unwrap_or(0)
    }

    pub fn partner_hold_kopecks(&self, partner_id: i64, now: i64) -> i64 {
        self.with_conn(|connection| connection.query_row(
            "SELECT COALESCE(SUM(amount_kopecks),0) FROM partner_ledger WHERE partner_id=?1 AND kind='sale' AND available_at>?2",
            rusqlite::params![partner_id, now], |row| row.get(0))).unwrap_or(0)
    }

    pub fn create_partner_withdrawal(
        &self,
        partner_id: i64,
        amount: i64,
        requisites: &str,
        now: i64,
    ) -> Result<i64, String> {
        if amount < 100_000 {
            return Err("минимальная сумма вывода — 1000 ₽".into());
        }
        let requisites = requisites.trim();
        if requisites.len() < 5 || requisites.len() > 300 {
            return Err("укажите корректные реквизиты".into());
        }
        self.with_conn(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let balance: i64 = transaction.query_row("SELECT COALESCE(SUM(amount_kopecks),0) FROM partner_ledger WHERE partner_id=?1 AND available_at<=?2", rusqlite::params![partner_id, now], |row| row.get(0))?;
            if balance < amount { return Err(rusqlite::Error::InvalidQuery); }
            transaction.execute("INSERT INTO partner_withdrawals(partner_id,amount_kopecks,requisites,created_at) VALUES(?1,?2,?3,?4)", rusqlite::params![partner_id, amount, requisites, now])?;
            let id = transaction.last_insert_rowid();
            transaction.execute("INSERT INTO partner_ledger(partner_id,amount_kopecks,kind,reference,available_at,details,created_at) VALUES(?1,?2,'withdrawal_reserve',?3,?4,?5,?4)", rusqlite::params![partner_id, -amount, format!("withdrawal:{id}"), now, format!("Вывод #{id}")])?;
            transaction.commit()?; Ok(id)
        }).map_err(|error| if matches!(error, rusqlite::Error::InvalidQuery) { "недостаточно доступных средств".into() } else { format!("не удалось создать заявку: {error}") })
    }

    pub fn partner_withdrawals(&self, partner_id: i64, limit: usize) -> Vec<PartnerWithdrawal> {
        self.with_conn(|connection| { let mut statement=connection.prepare("SELECT id,partner_id,amount_kopecks,requisites,status,created_at FROM partner_withdrawals WHERE partner_id=?1 ORDER BY created_at DESC LIMIT ?2")?; let rows=statement.query_map(rusqlite::params![partner_id,limit as i64],withdrawal_row)?.collect(); rows }).unwrap_or_default()
    }

    pub fn decide_partner_withdrawal(
        &self,
        id: i64,
        paid: bool,
        actor: i64,
        reason: Option<&str>,
        now: i64,
    ) -> bool {
        self.with_conn(|connection| {
            let transaction=connection.unchecked_transaction()?;
            let partner_id: i64=transaction.query_row("SELECT partner_id FROM partner_withdrawals WHERE id=?1 AND status='pending'",[id],|row|row.get(0))?;
            let changed=transaction.execute("UPDATE partner_withdrawals SET status=?2,decided_at=?3,decided_by=?4,reject_reason=?5 WHERE id=?1 AND status='pending'",rusqlite::params![id,if paid{"paid"}else{"rejected"},now,actor,reason])?;
            if changed!=1{return Ok(false)}
            if !paid { let amount:i64=transaction.query_row("SELECT amount_kopecks FROM partner_withdrawals WHERE id=?1",[id],|row|row.get(0))?; transaction.execute("INSERT INTO partner_ledger(partner_id,amount_kopecks,kind,reference,available_at,details,created_at) VALUES(?1,?2,'withdrawal_refund',?3,?4,?5,?4)",rusqlite::params![partner_id,amount,format!("withdrawal-refund:{id}"),now,format!("Возврат вывода #{id}")])?; }
            transaction.commit()?; Ok(true)
        }).unwrap_or(false)
    }

    pub fn undelivered_partner_orders(&self, partner_id: i64) -> Vec<PartnerOrder> {
        self.with_conn(|connection| {
            let mut statement = connection.prepare(&format!("SELECT {ORDER_COLUMNS} FROM partner_orders WHERE partner_id=?1 AND status='fulfilled' AND delivered_at IS NULL ORDER BY id"))?;
            let orders = statement.query_map([partner_id], order_row)?.collect();
            orders
        }).unwrap_or_default()
    }

    pub fn mark_partner_order_delivered(&self, id: i64, now: i64) -> bool {
        self.with_conn(|connection| connection.execute(
            "UPDATE partner_orders SET delivered_at=?2,updated_at=?2 WHERE id=?1 AND status='fulfilled' AND delivered_at IS NULL", rusqlite::params![id, now]))
            .is_ok_and(|changed| changed == 1)
    }

    pub fn partner_pending_order_count(&self, partner_id: i64) -> i64 {
        self.with_conn(|connection| {
            connection.query_row(
                "SELECT COUNT(*) FROM partner_orders WHERE partner_id=?1 AND status='pending'",
                [partner_id],
                |row| row.get(0),
            )
        })
        .unwrap_or(0)
    }

    pub fn create_partner(
        &self,
        owner_user_id: i64,
        slug: &str,
        display_name: &str,
        wholesale_discount_percent: i64,
        retail_markup_percent: i64,
        now: i64,
    ) -> Result<i64, String> {
        let slug = slug.trim().to_ascii_lowercase();
        let display_name = display_name.trim();
        if !valid_slug(&slug) {
            return Err(
                "slug должен содержать 3–32 строчные латинские буквы, цифры или дефисы".into(),
            );
        }
        if display_name.is_empty() || display_name.chars().count() > 64 {
            return Err("название партнёра должно содержать от 1 до 64 символов".into());
        }
        if !(0..=100).contains(&wholesale_discount_percent)
            || !(0..=500).contains(&retail_markup_percent)
        {
            return Err("некорректная скидка или наценка партнёра".into());
        }
        self.with_conn(|connection| {
            connection.execute(
                "INSERT INTO partners(owner_user_id,slug,display_name,wholesale_discount_percent,retail_markup_percent,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?6)",
                rusqlite::params![owner_user_id, slug, display_name, wholesale_discount_percent, retail_markup_percent, now],
            )?;
            Ok(connection.last_insert_rowid())
        })
        .map_err(|error| format!("не удалось создать партнёра: {error}"))
    }

    pub fn partner(&self, id: i64) -> Option<Partner> {
        self.with_conn(|connection| {
            connection
                .query_row(
                    &format!("SELECT {COLUMNS} FROM partners WHERE id=?1"),
                    [id],
                    row,
                )
                .optional()
        })
        .ok()
        .flatten()
    }

    pub fn partner_by_owner(&self, owner_user_id: i64) -> Option<Partner> {
        self.with_conn(|connection| {
            connection
                .query_row(
                    &format!("SELECT {COLUMNS} FROM partners WHERE owner_user_id=?1"),
                    [owner_user_id],
                    row,
                )
                .optional()
        })
        .ok()
        .flatten()
    }

    pub fn partners(&self) -> Vec<Partner> {
        self.with_conn(|connection| {
            let mut statement = connection.prepare(&format!(
                "SELECT {COLUMNS} FROM partners ORDER BY created_at DESC,id DESC"
            ))?;
            let partners = statement.query_map([], row)?.collect();
            partners
        })
        .unwrap_or_default()
    }

    pub fn set_partner_status(&self, id: i64, status: &str, now: i64) -> bool {
        if !matches!(status, "draft" | "active" | "suspended") {
            return false;
        }
        self.with_conn(|connection| {
            connection.execute(
                "UPDATE partners SET status=?2,updated_at=?3 WHERE id=?1",
                rusqlite::params![id, status, now],
            )
        })
        .is_ok_and(|changed| changed == 1)
    }

    pub fn set_partner_bot_identity(
        &self,
        id: i64,
        bot_username: &str,
        bot_secret_ref: &str,
        now: i64,
    ) -> bool {
        let username = bot_username.trim().trim_start_matches('@');
        let secret_ref = bot_secret_ref.trim();
        if username.len() < 5
            || !username
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            || secret_ref.is_empty()
        {
            return false;
        }
        self.with_conn(|connection| {
            connection.execute(
                "UPDATE partners SET bot_username=?2,bot_secret_ref=?3,updated_at=?4 WHERE id=?1",
                rusqlite::params![id, username, secret_ref, now],
            )
        })
        .is_ok_and(|changed| changed == 1)
    }

    pub fn assign_partner_customer(&self, partner_id: i64, user_id: i64, now: i64) -> bool {
        self.with_conn(|connection| {
            connection.execute(
                "INSERT OR IGNORE INTO partner_customers(partner_id,user_id,joined_at) VALUES(?1,?2,?3)",
                rusqlite::params![partner_id, user_id, now],
            )
        })
        .is_ok_and(|changed| changed == 1)
    }

    pub fn partner_customer_count(&self, partner_id: i64) -> i64 {
        self.with_conn(|connection| {
            connection.query_row(
                "SELECT COUNT(*) FROM partner_customers WHERE partner_id=?1",
                [partner_id],
                |row| row.get(0),
            )
        })
        .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partner_lifecycle_keeps_only_secret_reference() {
        let store = Store::open_in_memory();
        store.upsert_user(7, Some("seller"), "Seller", None, 1);
        store.upsert_user(8, Some("buyer"), "Buyer", None, 1);
        let id = store
            .create_partner(7, "Seller-One", "Seller One", 20, 35, 2)
            .unwrap();
        assert!(store.set_partner_bot_identity(
            id,
            "@seller_one_bot",
            "/etc/awgram/partners/1.env",
            3
        ));
        assert!(store.set_partner_status(id, "active", 4));
        assert!(store.assign_partner_customer(id, 8, 5));
        assert!(!store.assign_partner_customer(id, 8, 6));

        let partner = store.partner_by_owner(7).unwrap();
        assert_eq!(partner.slug, "seller-one");
        assert_eq!(partner.status, "active");
        assert_eq!(partner.retail_markup_percent, 35);
        assert_eq!(partner.bot_username.as_deref(), Some("seller_one_bot"));
        assert_eq!(store.partner_customer_count(id), 1);
        assert_eq!(store.partners(), vec![partner]);
    }

    #[test]
    fn partner_validation_rejects_unsafe_values() {
        let store = Store::open_in_memory();
        store.upsert_user(7, None, "Seller", None, 1);
        assert!(store
            .create_partner(7, "../bad", "Seller", 0, 0, 2)
            .is_err());
        assert!(store.create_partner(7, "seller", "", 0, 0, 2).is_err());
        assert!(store
            .create_partner(7, "seller", "Seller", 101, 0, 2)
            .is_err());
    }

    #[test]
    fn partner_order_freezes_retail_and_wholesale_prices() {
        let store = Store::open_in_memory();
        store.upsert_user(7, None, "Seller", None, 1);
        store.upsert_user(8, None, "Buyer", None, 1);
        let id = store
            .create_partner(7, "seller", "Seller", 20, 25, 2)
            .unwrap();
        assert!(store.set_partner_status(id, "active", 3));
        let base = store.tariff_price_kopecks(1).unwrap();
        let order = store.create_partner_order(id, 8, 1, 4).unwrap();
        assert_eq!(order.retail_price_kopecks, base);
        assert_eq!(order.wholesale_price_kopecks, base * 80 / 100);
        assert_eq!(store.partner_pending_order_count(id), 1);
        assert!(store.create_partner_order(id, 8, 3, 5).is_err());
        assert!(store.fulfill_partner_order(order.id, "buyer-01", "/tmp/a.conf", "", "awg://x", 6));
        assert_eq!(store.partner_pending_order_count(id), 0);
        assert_eq!(store.undelivered_partner_orders(id).len(), 1);
        assert!(store.mark_partner_order_delivered(order.id, 7));
        assert!(store.undelivered_partner_orders(id).is_empty());
        let summary = store.partner_sales_summary(id);
        assert_eq!(summary.fulfilled, 1);
        assert_eq!(summary.delivered, 1);
        assert_eq!(summary.retail_kopecks, order.retail_price_kopecks);
        assert_eq!(
            store.partner_hold_kopecks(id, 6),
            order.retail_price_kopecks - order.wholesale_price_kopecks
        );
        assert_eq!(store.partner_balance_kopecks(id, 6), 0);
        assert_eq!(
            store.partner_balance_kopecks(id, 6 + 604_800),
            order.retail_price_kopecks - order.wholesale_price_kopecks
        );
        store.upsert_user(9, None, "Other buyer", None, 8);
        let cancellable = store.create_partner_order(id, 9, 3, 9).unwrap();
        assert_eq!(
            store.partner_customer_orders(id, 9, 10),
            vec![cancellable.clone()]
        );
        assert!(store.cancel_partner_order(id, 9, cancellable.id, 10));
        assert!(!store.cancel_partner_order(id, 9, cancellable.id, 11));
        assert!(store.set_partner_status(id, "suspended", 5));
        assert!(store.create_partner_order(id, 8, 1, 6).is_err());
    }
}
