use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use teloxide::prelude::*;

use crate::{store::Store, vpn::Vpn};

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Выбирает только ближайший актуальный порог, чтобы после простоя бот не
/// присылал пользователю сразу три устаревших предупреждения.
fn threshold_days(left: i64) -> Option<i64> {
    match left {
        i64::MIN..=0 => Some(0),
        1..=86_400 => Some(1),
        86_401..=259_200 => Some(3),
        259_201..=604_800 => Some(7),
        _ => None,
    }
}

pub async fn tick(bot: &Bot, vpn: &Vpn, store: &Store, now: i64) {
    for user_id in store.all_user_ids() {
        for name in store.user_client_names(user_id) {
            let Some(expires_at) = vpn.client_expiry(&name) else {
                continue;
            };
            let Some(threshold) = threshold_days(expires_at.saturating_sub(now)) else {
                continue;
            };
            if !store.mark_expiry_notification(&name, user_id, expires_at, threshold, now) {
                continue;
            }
            let text = if threshold == 0 {
                format!("⛔ Срок действия ключа «{name}» истёк. Ключ отключён. Для продолжения работы продлите подписку в меню покупки.")
            } else {
                format!("⚠️ Срок действия ключа «{name}» закончится через {threshold} дн. Продлите подписку заранее, чтобы не потерять доступ.")
            };
            match bot.send_message(ChatId(user_id), text).await {
                Ok(_) => {}
                Err(error) => {
                    store.unmark_expiry_notification(&name, expires_at, threshold);
                    tracing::warn!(%error, user_id, client = %name, "не удалось отправить напоминание");
                }
            }
        }
    }
}

pub async fn run(bot: Bot, vpn: Arc<Vpn>, store: Arc<Store>) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
    loop {
        interval.tick().await;
        tick(&bot, &vpn, &store, now_epoch()).await;
    }
}

#[cfg(test)]
mod tests {
    use super::threshold_days;

    #[test]
    fn selects_current_expiry_bucket() {
        assert_eq!(threshold_days(-1), Some(0));
        assert_eq!(threshold_days(1), Some(1));
        assert_eq!(threshold_days(2 * 86_400), Some(3));
        assert_eq!(threshold_days(6 * 86_400), Some(7));
        assert_eq!(threshold_days(8 * 86_400), None);
    }
}
