use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use teloxide::{
    prelude::*,
    types::{KeyboardButton, KeyboardMarkup},
};
use tokio::task::JoinHandle;

use crate::{
    config::Config,
    store::{Partner, Store},
};

fn menu() -> KeyboardMarkup {
    KeyboardMarkup::new(vec![
        vec![
            KeyboardButton::new("🏠 Кабинет"),
            KeyboardButton::new("💳 Тарифы"),
        ],
        vec![
            KeyboardButton::new("🛒 Купить"),
            KeyboardButton::new("🆘 Поддержка"),
        ],
        vec![
            KeyboardButton::new("1 месяц"),
            KeyboardButton::new("3 месяца"),
        ],
        vec![
            KeyboardButton::new("6 месяцев"),
            KeyboardButton::new("12 месяцев"),
        ],
    ])
    .resize_keyboard()
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

async fn run_bot(partner: Partner, token: String, db_path: PathBuf) {
    let bot = Bot::new(token);
    let partner_id = partner.id;
    let partner = Arc::new(partner);
    teloxide::repl(bot, move |bot: Bot, msg: Message| {
        let partner = Arc::clone(&partner);
        let db_path = db_path.clone();
        async move {
            let Some(from) = msg.from.as_ref() else { return Ok(()); };
            let Ok(user_id) = i64::try_from(from.id.0) else { return Ok(()); };
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            let store = match Store::open(&db_path) {
                Ok(store) => store,
                Err(error) => { tracing::error!(partner_id=partner.id, %error, "partner database unavailable"); return Ok(()); }
            };
            let display_name = [from.first_name.as_str(), from.last_name.as_deref().unwrap_or("")].join(" ").trim().to_string();
            store.upsert_user(user_id, from.username.as_deref(), &display_name, None, now);
            store.assign_partner_customer(partner.id, user_id, now);
            let text = msg.text().unwrap_or("");
            let answer = if let Some(months) = term(text) {
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
                    let price = i128::from(base) * i128::from(100 + partner.retail_markup_percent) / 100;
                    format!("• {months} мес. — {:.2} ₽", price as f64 / 100.0)
                })).collect::<Vec<_>>().join("\n");
                format!("💳 Тарифы «{}»\n\n{}\n\nВыберите срок кнопкой ниже.", partner.display_name, lines)
            } else if text == "🆘 Поддержка" {
                "🆘 Напишите владельцу этого бота ответным сообщением. Автоматическая поддержка появится в следующем обновлении.".into()
            } else {
                format!("🏠 {}\n\nЗдесь можно выбрать тариф и оформить заявку на VPN. После оформления менеджер подтвердит оплату и выдаст ключ.", partner.display_name)
            };
            bot.send_message(msg.chat.id, answer).reply_markup(menu()).await?;
            Ok(())
        }
    }).await;
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
