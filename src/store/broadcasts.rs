use rusqlite::OptionalExtension;

use crate::store::Store;

#[derive(Debug, Clone)]
pub struct BroadcastRun {
    pub id: i64,
    pub source_chat_id: i64,
    pub source_message_id: i32,
    pub audience: String,
    pub delivered: i64,
    pub failed: i64,
}

impl Store {
    pub fn create_broadcast_run(
        &self,
        admin_id: i64,
        source_chat_id: i64,
        source_message_id: i32,
        audience: &str,
        recipients: &[i64],
        now: i64,
    ) -> Option<i64> {
        self.with_conn(|connection| {
            let transaction = connection.unchecked_transaction()?;
            transaction.execute(
                "INSERT INTO broadcasts(admin_id,source_chat_id,source_message_id,audience,created_at)
                 VALUES(?1,?2,?3,?4,?5)",
                rusqlite::params![admin_id,source_chat_id,source_message_id,audience,now],
            )?;
            let id = transaction.last_insert_rowid();
            for user_id in recipients {
                transaction.execute(
                    "INSERT OR IGNORE INTO broadcast_deliveries(broadcast_id,user_id,status,updated_at)
                     VALUES(?1,?2,'pending',?3)",
                    rusqlite::params![id,user_id,now],
                )?;
            }
            transaction.commit()?;
            Ok(id)
        }).ok()
    }

    pub fn record_broadcast_delivery(
        &self,
        broadcast_id: i64,
        user_id: i64,
        delivered: bool,
        error: Option<&str>,
        now: i64,
    ) {
        let status = if delivered { "delivered" } else { "failed" };
        let details = error.map(|value| value.chars().take(500).collect::<String>());
        let _ = self.with_conn(|connection| {
            let transaction = connection.unchecked_transaction()?;
            transaction.execute(
                "UPDATE broadcast_deliveries SET status=?3,attempts=attempts+1,last_error=?4,updated_at=?5
                 WHERE broadcast_id=?1 AND user_id=?2",
                rusqlite::params![broadcast_id,user_id,status,details,now],
            )?;
            transaction.execute(
                "UPDATE broadcasts SET
                   delivered=(SELECT COUNT(*) FROM broadcast_deliveries WHERE broadcast_id=?1 AND status='delivered'),
                   failed=(SELECT COUNT(*) FROM broadcast_deliveries WHERE broadcast_id=?1 AND status='failed')
                 WHERE id=?1",
                [broadcast_id],
            )?;
            transaction.commit()
        });
    }

    pub fn failed_broadcast_recipients(&self, id: i64) -> Vec<i64> {
        self.with_conn(|connection| {
            let mut statement = connection.prepare(
                "SELECT user_id FROM broadcast_deliveries WHERE broadcast_id=?1 AND status='failed' ORDER BY user_id",
            )?;
            let rows = statement.query_map([id], |row| row.get(0))?;
            rows.collect()
        }).unwrap_or_default()
    }

    pub fn broadcast_run(&self, id: i64) -> Option<BroadcastRun> {
        self.with_conn(|connection| connection.query_row(
            "SELECT id,source_chat_id,source_message_id,audience,delivered,failed FROM broadcasts WHERE id=?1",
            [id],
            |row| Ok(BroadcastRun { id:row.get(0)?,source_chat_id:row.get(1)?,source_message_id:row.get(2)?,audience:row.get(3)?,delivered:row.get(4)?,failed:row.get(5)? }),
        ).optional()).ok().flatten()
    }
}
