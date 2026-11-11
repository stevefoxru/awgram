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

/// Legacy-тариф оплачивается раз в год, поэтому предупреждаем заметно раньше
/// обычной помесячной подписки. Возвращается ближайший актуальный порог: после
/// простоя пользователь получит одно сообщение, а не всю пропущенную серию.
fn legacy_threshold_days(left: i64) -> Option<i64> {
    match left {
        i64::MIN..=0 => Some(0),
        1..=86_400 => Some(1),
        86_401..=259_200 => Some(3),
        259_201..=604_800 => Some(7),
        604_801..=1_209_600 => Some(14),
        1_209_601..=2_592_000 => Some(30),
        2_592_001..=5_184_000 => Some(60),
        _ => None,
    }
}

fn days_left(left: i64) -> i64 {
    left.max(0).saturating_add(86_399) / 86_400
}

async fn set_managed_expiry(
    vpn: &Vpn,
    store: &Store,
    name: &str,
    expires_at: i64,
) -> crate::error::Result<()> {
    match store.client_vpn_server(name) {
        Some(server) if !server.is_local && server.protocol == "amneziawg-panel" => {
            let secret = store
                .panel_password(server.id)
                .ok_or_else(|| crate::error::Error::Parse("пароль панели не настроен".into()))?;
            vpn.panel_set_expiry(&server, &secret, name, expires_at)
                .await
        }
        Some(server) if !server.is_local => {
            if let (Some(node), Some(secret)) = (
                store.vpn_node_for_server(server.id),
                store.node_secret(server.id),
            ) {
                vpn.agent_set_expiry(&server, &node, &secret, name, expires_at)
                    .await
            } else {
                vpn.remote_set_expiry(&server, name, expires_at).await
            }
        }
        _ => vpn.set_client_expiry(name, Some(expires_at)).await,
    }
}

pub async fn tick(bot: &Bot, vpn: &Vpn, store: &Store, now: i64) {
    for (name, user_id, months) in store.auto_renew_clients() {
        let Some(expires_at) = vpn.client_expiry(&name) else {
            continue;
        };
        let left = expires_at.saturating_sub(now);
        if left <= 0 || left > 86_400 || !store.claim_renewal_attempt(&name, expires_at, now) {
            continue;
        }
        let seconds = match months {
            1 => 30 * 86_400,
            3 => 90 * 86_400,
            6 => 180 * 86_400,
            12 => 365 * 86_400,
            _ => {
                store.finish_renewal_attempt(&name, expires_at, "invalid_tariff");
                continue;
            }
        };
        let Some(amount) = store.tariff_price_kopecks(months) else {
            store.finish_renewal_attempt(&name, expires_at, "invalid_tariff");
            continue;
        };
        let reference = format!("autorenew:{name}:{expires_at}");
        if !store.spend_balance(user_id, amount, &reference, now) {
            store.finish_renewal_attempt(&name, expires_at, "insufficient_balance");
            let _ = bot.send_message(ChatId(user_id), format!("⚠️ Автопродление ключа «{name}» не выполнено: недостаточно средств. Пополните баланс до истечения срока.")).await;
            continue;
        }
        let new_expiry = expires_at.max(now).saturating_add(seconds);
        match set_managed_expiry(vpn, store, &name, new_expiry).await {
            Ok(()) => {
                store.finish_renewal_attempt(&name, expires_at, "done");
                if let Some(referrer) = store.user(user_id).and_then(|u| u.referrer_id) {
                    let reward = amount * i64::from(store.referral_percent()) / 100;
                    store.add_ledger_entry(
                        referrer,
                        reward,
                        "referral",
                        &format!("referral:{reference}"),
                        Some(&format!("autorenew user={user_id}")),
                        now,
                    );
                }
                let _ = bot.send_message(ChatId(user_id), format!("✅ Автопродление выполнено\n\n🔑 {name}\n📅 Новый срок: {}\n💰 Списано: {:.2} ₽\n💼 Остаток: {:.2} ₽", crate::calendar::format_date(new_expiry), amount as f64 / 100.0, store.balance_kopecks(user_id) as f64 / 100.0)).await;
            }
            Err(error) => {
                store.add_ledger_entry(
                    user_id,
                    amount,
                    "refund",
                    &format!("refund:{reference}"),
                    Some("autorenew failed"),
                    now,
                );
                store.finish_renewal_attempt(&name, expires_at, "failed");
                tracing::error!(%error, client=%name, "ошибка автопродления");
                let _ = bot.send_message(ChatId(user_id), format!("⚠️ Автопродление ключа «{name}» не выполнено из-за ошибки сервера. Списанная сумма полностью возвращена на внутренний баланс. Попробуйте продлить ключ вручную позже.")).await;
            }
        }
    }
    let legacy_clients = store.legacy_clients();
    let legacy_names = legacy_clients
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<std::collections::HashSet<_>>();
    for (name, user_id) in &legacy_clients {
        if !store.notification_preferences(*user_id).0 || !store.client_ever_used(name) {
            continue;
        }
        let Some(expires_at) = vpn.client_expiry(name) else {
            continue;
        };
        let left = expires_at.saturating_sub(now);
        let Some(threshold) = legacy_threshold_days(left) else {
            continue;
        };
        if !store.mark_expiry_notification(name, *user_id, expires_at, threshold, now) {
            continue;
        }
        let target = crate::calendar::legacy_renewal_target(now, Some(expires_at));
        let price =
            store.legacy_renewal_price_for_user(*user_id, store.legacy_renewal_price_kopecks());
        let timing = if threshold == 0 {
            "срок уже истёк".to_string()
        } else {
            format!("осталось {} дн.", days_left(left))
        };
        let text = format!(
            "🔔 Пора продлить Legacy-ключ\n\n🔑 {name}\n📅 Действует до: {} ({timing})\n💰 Продление до {}: {:.2} ₽\n\nОформите продление заранее, чтобы подключение не остановилось в конце срока.",
            crate::calendar::format_date(expires_at),
            crate::calendar::format_date(target),
            price as f64 / 100.0
        );
        let sent = bot
            .send_message(ChatId(*user_id), text.clone())
            .reply_markup(crate::bot::menu::legacy_renew_menu(name, price))
            .await;
        if let Err(error) = sent {
            store.unmark_expiry_notification(name, expires_at, threshold);
            tracing::warn!(%error,user_id,client=%name,"не удалось отправить legacy-напоминание");
        } else {
            store.add_portal_notification(
                *user_id,
                "renewal",
                "Пора продлить Legacy-ключ",
                &text,
                Some("/lk/keys"),
                now,
            );
        }
    }
    for user_id in store.all_user_ids() {
        if !store.notification_preferences(user_id).0 {
            continue;
        }
        for name in store.user_client_names(user_id) {
            if legacy_names.contains(name.as_str()) {
                continue;
            }
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
            let mut request = bot.send_message(ChatId(user_id), text);
            if threshold == 0 {
                request = request.reply_markup(crate::bot::menu::expired_subscription_menu(&name));
            }
            match request.await {
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
    use super::{days_left, legacy_threshold_days, threshold_days};

    #[test]
    fn selects_current_expiry_bucket() {
        assert_eq!(threshold_days(-1), Some(0));
        assert_eq!(threshold_days(1), Some(1));
        assert_eq!(threshold_days(2 * 86_400), Some(3));
        assert_eq!(threshold_days(6 * 86_400), Some(7));
        assert_eq!(threshold_days(8 * 86_400), None);
    }

    #[test]
    fn selects_early_legacy_expiry_bucket_without_duplicate_backlog() {
        assert_eq!(legacy_threshold_days(61 * 86_400), None);
        assert_eq!(legacy_threshold_days(50 * 86_400), Some(60));
        assert_eq!(legacy_threshold_days(30 * 86_400), Some(30));
        assert_eq!(legacy_threshold_days(14 * 86_400), Some(14));
        assert_eq!(legacy_threshold_days(7 * 86_400), Some(7));
        assert_eq!(legacy_threshold_days(3 * 86_400), Some(3));
        assert_eq!(legacy_threshold_days(86_400), Some(1));
        assert_eq!(legacy_threshold_days(0), Some(0));
        assert_eq!(days_left(49 * 86_400 + 1), 50);
    }
}
