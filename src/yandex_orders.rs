use std::collections::BTreeMap;
use std::io::Cursor;

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use serde::Serialize;

const REQUIRED_COLUMNS: &[&str] = &[
    "Номер заказа",
    "Начало, МСК",
    "Завершение, МСК",
    "Статус",
    "Стоимость заказа, ₽",
    "Источник комиссий",
    "ID заказа",
];

#[derive(Clone, Debug, Serialize)]
pub struct YandexOrder {
    pub order_id: String,
    pub order_number: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub status: String,
    pub amount_kopecks: Option<i64>,
    pub source_sheet: String,
    pub commission_source: Option<String>,
    pub data_json: String,
}

#[derive(Debug, Serialize)]
pub struct ParsedYandexReport {
    pub owner: Option<String>,
    pub orders: Vec<YandexOrder>,
    pub sheets: Vec<String>,
}

fn cell_text(cell: Option<&Data>) -> String {
    match cell {
        Some(Data::String(value)) => value.trim().to_string(),
        Some(Data::Int(value)) => value.to_string(),
        Some(Data::Float(value)) => {
            if value.fract().abs() < f64::EPSILON {
                format!("{value:.0}")
            } else {
                value.to_string()
            }
        }
        Some(Data::Bool(value)) => value.to_string(),
        Some(Data::DateTime(value)) => value.as_f64().to_string(),
        _ => String::new(),
    }
}

fn excel_time(cell: Option<&Data>) -> Result<i64, String> {
    let value = match cell {
        Some(Data::DateTime(value)) => value.as_f64(),
        Some(Data::Float(value)) => *value,
        Some(Data::Int(value)) => *value as f64,
        _ => return Err("дата заказа отсутствует или имеет неверный формат".to_string()),
    };
    Ok(((value - 25_569.0) * 86_400.0).round() as i64)
}

fn optional_excel_time(cell: Option<&Data>) -> Result<Option<i64>, String> {
    if cell.is_none() || cell.is_some_and(|value| cell_text(Some(value)).is_empty()) {
        return Ok(None);
    }
    excel_time(cell).map(Some)
}

fn money_kopecks(cell: Option<&Data>) -> Result<Option<i64>, String> {
    match cell {
        None | Some(Data::Empty) => Ok(None),
        Some(Data::Float(value)) => Ok(Some((value * 100.0).round() as i64)),
        Some(Data::Int(value)) => Ok(Some(value * 100)),
        Some(Data::String(value)) if value.trim().is_empty() => Ok(None),
        Some(Data::String(value)) => value
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .map(|number| Some((number * 100.0).round() as i64))
            .map_err(|_| format!("неверная сумма: {value}")),
        _ => Err("неверный формат суммы".to_string()),
    }
}

pub fn parse_yandex_report(bytes: &[u8]) -> Result<ParsedYandexReport, String> {
    let mut workbook = open_workbook_auto_from_rs(Cursor::new(bytes.to_vec()))
        .map_err(|error| format!("не удалось открыть Excel: {error}"))?;
    let sheet_names = workbook.sheet_names().to_vec();
    let owner = workbook
        .worksheet_range("Итоги")
        .ok()
        .and_then(|range| range.get((0, 0)).map(|cell| cell_text(Some(cell))))
        .filter(|value| !value.is_empty());
    let mut orders = Vec::new();
    let mut imported_sheets = Vec::new();
    for sheet_name in sheet_names {
        if sheet_name == "Итоги" {
            continue;
        }
        let range = workbook
            .worksheet_range(&sheet_name)
            .map_err(|error| format!("лист «{sheet_name}»: {error}"))?;
        let mut rows = range.rows();
        let Some(header_row) = rows.next() else {
            continue;
        };
        let headers: Vec<String> = header_row
            .iter()
            .map(|cell| cell_text(Some(cell)))
            .collect();
        let columns: BTreeMap<&str, usize> = headers
            .iter()
            .enumerate()
            .map(|(index, value)| (value.as_str(), index))
            .collect();
        if !REQUIRED_COLUMNS
            .iter()
            .all(|name| columns.contains_key(name))
        {
            continue;
        }
        imported_sheets.push(sheet_name.clone());
        for (offset, row) in rows.enumerate() {
            let get = |name: &str| row.get(columns[name]);
            let order_id = cell_text(get("ID заказа"));
            if order_id.is_empty() {
                return Err(format!(
                    "лист «{sheet_name}», строка {}: нет ID заказа",
                    offset + 2
                ));
            }
            let status = cell_text(get("Статус"));
            if status != "Выполнен" && status != "Отменён" {
                return Err(format!(
                    "лист «{sheet_name}», строка {}: неизвестный статус «{status}»",
                    offset + 2
                ));
            }
            let data: BTreeMap<&str, String> = headers
                .iter()
                .enumerate()
                .filter(|(_, header)| !header.is_empty())
                .map(|(index, header)| (header.as_str(), cell_text(row.get(index))))
                .collect();
            orders.push(YandexOrder {
                order_id,
                order_number: cell_text(get("Номер заказа")),
                started_at: excel_time(get("Начало, МСК"))?,
                finished_at: optional_excel_time(get("Завершение, МСК"))?,
                status,
                amount_kopecks: money_kopecks(get("Стоимость заказа, ₽"))?,
                source_sheet: sheet_name.clone(),
                commission_source: Some(cell_text(get("Источник комиссий")))
                    .filter(|value| !value.is_empty()),
                data_json: serde_json::to_string(&data)
                    .map_err(|error| format!("не удалось подготовить заказ: {error}"))?,
            });
        }
    }
    if imported_sheets.is_empty() {
        return Err("в книге нет листов заказов с ожидаемыми колонками".to_string());
    }
    Ok(ParsedYandexReport {
        owner,
        orders,
        sheets: imported_sheets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn money_preserves_empty_and_rounds_to_kopecks() {
        assert_eq!(
            money_kopecks(Some(&Data::Float(12.345))).unwrap(),
            Some(1235)
        );
        assert_eq!(money_kopecks(Some(&Data::Empty)).unwrap(), None);
    }
    #[test]
    fn excel_serial_is_converted_to_unix_time() {
        assert_eq!(excel_time(Some(&Data::Float(25_569.0))).unwrap(), 0);
    }
}
