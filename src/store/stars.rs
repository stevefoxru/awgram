use rusqlite::OptionalExtension;

use crate::store::Store;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StarOrder {
    pub id: i64,
    pub user_id: i64,
    pub kind: String,
    pub months: i64,
    pub stars: i64,
    pub client_name: Option<String>,
    pub server_id: Option<i64>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StarPaymentClaim {
    New(StarOrder),
    Duplicate,
    Invalid,
}

pub struct NewStarOrder<'a> {
    pub user_id: i64,
    pub kind: &'a str,
    pub months: i64,
    pub stars: i64,
    pub client_name: Option<&'a str>,
    pub server_id: Option<i64>,
    pub created_at: i64,
}

fn order_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StarOrder> {
    Ok(StarOrder {
        id: row.get(0)?,
        user_id: row.get(1)?,
        kind: row.get(2)?,
        months: row.get(3)?,
        stars: row.get(4)?,
        client_name: row.get(5)?,
        server_id: row.get(6)?,
        status: row.get(7)?,
    })
}

impl Store {
    pub fn create_star_order(&self, order: NewStarOrder<'_>) -> Option<StarOrder> {
        if !matches!(order.kind, "purchase" | "renew") || order.stars <= 0 {
            return None;
        }
        self.with_conn(|connection| {
            connection.execute(
                "INSERT INTO star_orders(user_id,kind,months,stars,client_name,server_id,created_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7)",
                rusqlite::params![
                    order.user_id,
                    order.kind,
                    order.months,
                    order.stars,
                    order.client_name,
                    order.server_id,
                    order.created_at
                ],
            )?;
            let id = connection.last_insert_rowid();
            connection.query_row(
                "SELECT id,user_id,kind,months,stars,client_name,server_id,status FROM star_orders WHERE id=?1",
                [id],
                order_from_row,
            )
        })
        .ok()
    }

    pub fn star_order(&self, id: i64) -> Option<StarOrder> {
        self.with_conn(|connection| {
            connection
                .query_row(
                    "SELECT id,user_id,kind,months,stars,client_name,server_id,status FROM star_orders WHERE id=?1",
                    [id],
                    order_from_row,
                )
                .optional()
        })
        .ok()
        .flatten()
    }

    pub fn claim_star_payment(
        &self,
        id: i64,
        user_id: i64,
        stars: i64,
        charge_id: &str,
        now: i64,
    ) -> StarPaymentClaim {
        let result = self.with_conn(|connection| {
            let changed = connection.execute(
                "UPDATE star_orders SET status='paid',telegram_charge_id=?4,paid_at=?5
                 WHERE id=?1 AND user_id=?2 AND stars=?3 AND status='pending'",
                rusqlite::params![id, user_id, stars, charge_id, now],
            )?;
            if changed == 1 {
                return connection.query_row(
                    "SELECT id,user_id,kind,months,stars,client_name,server_id,status FROM star_orders WHERE id=?1",
                    [id],
                    order_from_row,
                ).map(Some);
            }
            Ok(None)
        });
        match result {
            Ok(Some(order)) => StarPaymentClaim::New(order),
            _ if self
                .star_order(id)
                .is_some_and(|order| order.status != "pending") =>
            {
                StarPaymentClaim::Duplicate
            }
            _ => StarPaymentClaim::Invalid,
        }
    }

    pub fn finish_star_order(&self, id: i64, failure: Option<&str>, now: i64) {
        let status = if failure.is_some() {
            "failed"
        } else {
            "fulfilled"
        };
        let _ = self.with_conn(|connection| {
            connection.execute(
                "UPDATE star_orders SET status=?2,fulfilled_at=?3,failure=?4 WHERE id=?1 AND status='paid'",
                rusqlite::params![id, status, now, failure],
            )
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payment_claim_is_idempotent() {
        let store = Store::open_in_memory();
        store.upsert_user(7, None, "User", None, 1);
        let order = store
            .create_star_order(NewStarOrder {
                user_id: 7,
                kind: "renew",
                months: 1,
                stars: 50,
                client_name: Some("phone"),
                server_id: None,
                created_at: 2,
            })
            .unwrap();
        assert!(matches!(
            store.claim_star_payment(order.id, 7, 50, "charge", 3),
            StarPaymentClaim::New(_)
        ));
        assert_eq!(
            store.claim_star_payment(order.id, 7, 50, "charge", 4),
            StarPaymentClaim::Duplicate
        );
    }
}
