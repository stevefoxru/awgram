use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use teloxide::{
    prelude::*,
    types::{InputFile, KeyboardButton, KeyboardMarkup},
};
use tokio::task::JoinHandle;

use crate::{
    config::Config,
    store::{Partner, Store},
};

fn menu(owner: bool) -> KeyboardMarkup {
    let mut rows = vec![
        vec![
            KeyboardButton::new("🏠 Кабинет"),
            KeyboardButton::new("💳 Тарифы"),
        ],
        vec![
            KeyboardButton::new("🛒 Купить"),
            KeyboardButton::new("🧾 Мои заказы"),
        ],
        vec![
            KeyboardButton::new("1 месяц"),
            KeyboardButton::new("3 месяца"),
        ],
        vec![
            KeyboardButton::new("6 месяцев"),
            KeyboardButton::new("12 месяцев"),
        ],
        vec![
            KeyboardButton::new("❌ Отменить заявку"),
            KeyboardButton::new("🆘 Поддержка"),
        ],
    ];
    if owner {
        rows.insert(
            0,
            vec![
                KeyboardButton::new("📊 Продажи"),
                KeyboardButton::new("💸 Вывод"),
            ],
        );
    }
    KeyboardMarkup::new(rows).resize_keyboard()
}

fn order_status(order: &crate::store::PartnerOrder) -> &'static str {
    match order.status.as_str() {
        "pending" => "🟠 ожидает обработки",
        "fulfilled" if order.delivered_at.is_some() => "✅ ключ доставлен",
        "fulfilled" => "📤 ключ готовится к отправке",
        "rejected" => "❌ отклонён",
        "cancelled" => "⚪ отменён",
        _ => "⏳ обрабатывается",
    }
}

fn term(text: &str) -> Option<i64> {
    match text.trim() {
        "1 месяц" => Some(1),
        "3 месяца" => Some(3),
        "6 месяцев" => Some(6),
        "12 месяцев" => Some(12),
        _ => None,
    }
}

fn epoch_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

async fn run_bot(partner: Partner, token: String, db_path: PathBuf) {
    let bot = Bot::new(token);
    let partner_id = partner.id;
    let partner = Arc::new(partner);
    let delivery_bot = bot.clone();
    let delivery_db = db_path.clone();
    let delivery = tokio::spawn(async move {
        loop {
            if let Ok(store) = Store::open(&delivery_db) {
                for order in store.undelivered_partner_orders(partner_id) {
                    let Some(conf) = order.conf_path.as_deref() else {
                        continue;
                    };
                    let chat = ChatId(order.user_id);
                    let mut sent = delivery_bot
                        .send_message(
                            chat,
                            format!(
                                "✅ Заказ #{} готов\nКлюч: {}",
                                order.id,
                                order.fulfilled_client_name.as_deref().unwrap_or("VPN")
                            ),
                        )
                        .await
                        .is_ok();
                    if sent {
                        sent = delivery_bot
                            .send_document(chat, InputFile::file(conf))
                            .await
                            .is_ok();
                    }
                    if sent {
                        if let Some(qr) = order
                            .qr_path
                            .as_deref()
                            .filter(|path| std::path::Path::new(path).exists())
                        {
                            sent = delivery_bot
                                .send_photo(chat, InputFile::file(qr))
                                .await
                                .is_ok();
                        }
                    }
                    if sent {
                        if let Some(uri) = order.import_uri.as_deref().filter(|uri| !uri.is_empty())
                        {
                            sent = delivery_bot
                                .send_message(chat, format!("Ссылка импорта:\n{uri}"))
                                .await
                                .is_ok();
                        }
                    }
                    if sent {
                        store.mark_partner_order_delivered(order.id, epoch_now());
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });
    teloxide::repl(bot, move |bot: Bot, msg: Message| {
        let partner = Arc::clone(&partner);
        let db_path = db_path.clone();
        async move {
            let Some(from) = msg.from.as_ref() else { return Ok(()); };
            let Ok(user_id) = i64::try_from(from.id.0) else { return Ok(()); };
            let now = epoch_now();
            let store = match Store::open(&db_path) {
                Ok(store) => store,
                Err(error) => { tracing::error!(partner_id=partner.id, %error, "partner database unavailable"); return Ok(()); }
            };
            let display_name = [from.first_name.as_str(), from.last_name.as_deref().unwrap_or("")].join(" ").trim().to_string();
            store.upsert_user(user_id, from.username.as_deref(), &display_name, None, now);
            store.assign_partner_customer(partner.id, user_id, now);
            let text = msg.text().unwrap_or("");
            let is_owner = user_id == partner.owner_user_id;
            let answer = if text == "📊 Продажи" && is_owner {
                let summary = store.partner_sales_summary(partner.id);
                format!("📊 Продажи «{}»\n\nКомиссия: {}%\nВсего заявок: {}\nОжидают: {}\nКлючей создано: {}\nДоставлено: {}\n\nРозничный оборот: {:.2} ₽\nОптовая сумма: {:.2} ₽\nМаржа: {:.2} ₽\nДоступно: {:.2} ₽\nНа холде 7 дней: {:.2} ₽\n\nМинимальный вывод: 1000 ₽.", partner.display_name, store.partner_commission_percent(partner.id, now), summary.total, summary.pending, summary.fulfilled, summary.delivered, summary.retail_kopecks as f64/100.0, summary.wholesale_kopecks as f64/100.0, (summary.retail_kopecks-summary.wholesale_kopecks) as f64/100.0, store.partner_balance_kopecks(partner.id, now) as f64/100.0, store.partner_hold_kopecks(partner.id, now) as f64/100.0)
            } else if text == "💸 Вывод" && is_owner {
                let history = store.partner_withdrawals(partner.id, 5);
                let list = history.iter().map(|item| format!("#{} · {:.2} ₽ · {}", item.id, item.amount_kopecks as f64 / 100.0, item.status)).collect::<Vec<_>>().join("\n");
                format!("💸 Вывод средств\n\nДоступно: {:.2} ₽\nМинимум: 1000 ₽\n\nОтправьте одной строкой:\nВывод 1000 НОМЕР_ТЕЛЕФОНА_ИЛИ_РЕКВИЗИТЫ{}", store.partner_balance_kopecks(partner.id, now) as f64 / 100.0, if list.is_empty() { String::new() } else { format!("\n\nПоследние заявки:\n{list}") })
            } else if is_owner && text.to_lowercase().starts_with("вывод ") {
                let mut parts = text.splitn(3, ' '); let _ = parts.next();
                let amount = parts.next().and_then(|value| value.replace(',', ".").parse::<f64>().ok()).map(|value| (value * 100.0).round() as i64);
                let requisites = parts.next().unwrap_or("");
                match amount { Some(amount) => match store.create_partner_withdrawal(partner.id, amount, requisites, now) { Ok(id) => format!("✅ Заявка на вывод #{id} создана. Сумма зарезервирована до решения администратора."), Err(error) => format!("❌ {error}") }, None => "❌ Не удалось разобрать сумму.".into() }
            } else if text == "🧾 Мои заказы" {
                let orders = store.partner_customer_orders(partner.id, user_id, 10);
                if orders.is_empty() { "🧾 У вас пока нет заказов.".into() } else {
                    format!("🧾 Ваши заказы\n\n{}", orders.iter().map(|order| format!("#{} · {} мес. · {:.2} ₽\n{}", order.id, order.months, order.retail_price_kopecks as f64/100.0, order_status(order))).collect::<Vec<_>>().join("\n\n"))
                }
            } else if text == "❌ Отменить заявку" {
                let pending = store.partner_customer_orders(partner.id, user_id, 20).into_iter().find(|order| order.status == "pending");
                match pending {
                    Some(order) if store.cancel_partner_order(partner.id, user_id, order.id, now) => format!("✅ Заявка #{} отменена. Новый заказ снова доступен.", order.id),
                    Some(_) => "Заявка уже начала обрабатываться и не может быть отменена.".into(),
                    None => "У вас нет заявки, которую можно отменить.".into(),
                }
            } else if let Some(months) = term(text) {
                match store.create_partner_order(partner.id, user_id, months, now) {
                    Ok(order) => {
                        let owner_notice = format!(
                            "🛒 Новый партнёрский заказ #{}\nПокупатель: {}\nСрок: {} мес.\nРозница: {:.2} ₽\nОпт: {:.2} ₽",
                            order.id,
                            user_id,
                            months,
                            order.retail_price_kopecks as f64 / 100.0,
                            order.wholesale_price_kopecks as f64 / 100.0
                        );
                        if let Err(error) = bot
                            .send_message(ChatId(partner.owner_user_id), owner_notice)
                            .await
                        {
                            tracing::debug!(partner_id=partner.id, %error, "partner owner notification unavailable");
                        }
                        format!("✅ Заказ #{} оформлен\nСрок: {} мес.\nК оплате: {:.2} ₽\n\nМенеджер свяжется с вами для оплаты и выдачи ключа.", order.id, months, order.retail_price_kopecks as f64 / 100.0)
                    }
                    Err(error) => format!("❌ {error}"),
                }
            } else if text == "💳 Тарифы" || text == "🛒 Купить" {
                let lines = [1,3,6,12].into_iter().filter_map(|months| store.tariff_price_kopecks(months).map(|base| {
                    format!("• {months} мес. — {:.2} ₽", base as f64 / 100.0)
                })).collect::<Vec<_>>().join("\n");
                format!("💳 Тарифы «{}»\n\n{}\n\nВыберите срок кнопкой ниже.", partner.display_name, lines)
            } else if text == "🆘 Поддержка" {
                "🆘 Напишите владельцу этого бота ответным сообщением. Автоматическая поддержка появится в следующем обновлении.".into()
            } else {
                format!("🏠 {}\n\nЗдесь можно выбрать тариф и оформить заявку на VPN. После оформления менеджер подтвердит оплату и выдаст ключ.", partner.display_name)
            };
            bot.send_message(msg.chat.id, answer).reply_markup(menu(is_owner)).await?;
            Ok(())
        }
    }).await;
    delivery.abort();
    tracing::warn!(partner_id, "partner bot stopped");
}

pub async fn supervise(config_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load(config_path)?;
    let mut running: HashMap<i64, (i64, JoinHandle<()>)> = HashMap::new();
    loop {
        let store = Store::open(&config.db_path)?;
        let active = store
            .partners()
            .into_iter()
            .filter(|p| p.status == "active")
            .collect::<Vec<_>>();
        running.retain(|id, (version, task)| {
            let keep = active
                .iter()
                .any(|p| p.id == *id && p.updated_at == *version)
                && !task.is_finished();
            if !keep {
                task.abort();
            }
            keep
        });
        for partner in active {
            if running.contains_key(&partner.id) {
                continue;
            }
            let Some(secret_ref) = partner.bot_secret_ref.as_deref() else {
                continue;
            };
            let token = match std::fs::read_to_string(secret_ref) {
                Ok(value) if !value.trim().is_empty() => value.trim().to_owned(),
                Ok(_) => continue,
                Err(error) => {
                    tracing::warn!(partner_id=partner.id, %error, "partner token unavailable");
                    continue;
                }
            };
            let version = partner.updated_at;
            let id = partner.id;
            let db_path = config.db_path.clone();
            running.insert(
                id,
                (version, tokio::spawn(run_bot(partner, token, db_path))),
            );
            tracing::info!(partner_id = id, "partner bot started");
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_only_supported_terms() {
        assert_eq!(term("3 месяца"), Some(3));
        assert_eq!(term("2 месяца"), None);
    }
}
