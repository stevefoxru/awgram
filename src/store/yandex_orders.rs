use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use super::Store;
use crate::yandex_orders::YandexOrder;

#[derive(Debug, Serialize)]
pub struct YandexImportResult {
    pub total: usize,
    pub added: usize,
    pub duplicates: usize,
    pub changed: usize,
}

#[derive(Debug, Serialize)]
pub struct YandexOrderSummary {
    pub total: i64,
    pub completed: i64,
    pub cancelled: i64,
    pub completed_kopecks: i64,
    pub first_started_at: Option<i64>,
    pub last_started_at: Option<i64>,
    pub last_import_at: Option<i64>,
}

impl Store {
    pub fn compare_yandex_orders(&self, orders: &[YandexOrder]) -> YandexImportResult {
        self.with_conn(|connection| {
            let mut result = YandexImportResult {
                total: orders.len(),
                added: 0,
                duplicates: 0,
                changed: 0,
            };
            let mut statement =
                connection.prepare("SELECT data_json FROM yandex_orders WHERE order_id=?1")?;
            for order in orders {
                match statement
                    .query_row([&order.order_id], |row| row.get::<_, String>(0))
                    .optional()?
                {
                    None => result.added += 1,
                    Some(existing) if existing == order.data_json => result.duplicates += 1,
                    Some(_) => result.changed += 1,
                }
            }
            Ok(result)
        })
        .unwrap_or(YandexImportResult {
            total: orders.len(),
            added: 0,
            duplicates: 0,
            changed: 0,
        })
    }

    pub fn import_yandex_orders(
        &self,
        owner: Option<&str>,
        file_name: &str,
        orders: &[YandexOrder],
        imported_by: i64,
        now: i64,
    ) -> Result<YandexImportResult, String> {
        self.with_conn(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut result = YandexImportResult { total: orders.len(), added: 0, duplicates: 0, changed: 0 };
            for order in orders {
                let existing = transaction.query_row("SELECT data_json FROM yandex_orders WHERE order_id=?1", [&order.order_id], |row| row.get::<_, String>(0)).optional()?;
                match existing {
                    Some(value) if value == order.data_json => result.duplicates += 1,
                    Some(_) => result.changed += 1,
                    None => {
                        transaction.execute(
                            "INSERT INTO yandex_orders(order_id,order_number,report_owner,source_sheet,started_at,finished_at,status,amount_kopecks,commission_source,data_json,imported_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                            params![order.order_id,order.order_number,owner,order.source_sheet,order.started_at,order.finished_at,order.status,order.amount_kopecks,order.commission_source,order.data_json,now],
                        )?;
                        result.added += 1;
                    }
                }
            }
            transaction.execute(
                "INSERT INTO yandex_order_imports(report_owner,file_name,total_rows,added_rows,duplicate_rows,changed_rows,imported_at,imported_by) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![owner,file_name,result.total as i64,result.added as i64,result.duplicates as i64,result.changed as i64,now,imported_by],
            )?;
            transaction.commit()?;
            Ok(result)
        }).map_err(|error| format!("не удалось сохранить заказы: {error}"))
    }

    pub fn yandex_order_summary(&self) -> YandexOrderSummary {
        self.with_conn(|connection| connection.query_row(
            "SELECT COUNT(*),COALESCE(SUM(status='Выполнен'),0),COALESCE(SUM(status='Отменён'),0),COALESCE(SUM(CASE WHEN status='Выполнен' THEN amount_kopecks ELSE 0 END),0),MIN(started_at),MAX(started_at),(SELECT MAX(imported_at) FROM yandex_order_imports) FROM yandex_orders", [],
            |row| Ok(YandexOrderSummary { total:row.get(0)?,completed:row.get(1)?,cancelled:row.get(2)?,completed_kopecks:row.get(3)?,first_started_at:row.get(4)?,last_started_at:row.get(5)?,last_import_at:row.get(6)? })
        )).unwrap_or(YandexOrderSummary { total:0,completed:0,cancelled:0,completed_kopecks:0,first_started_at:None,last_started_at:None,last_import_at:None })
    }
}
