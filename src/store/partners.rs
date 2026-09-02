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
            statement.query_map([], row)?.collect()
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
}
