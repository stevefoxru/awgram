use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use teloxide::prelude::*;

use crate::{config::Config, store::Store, vpn::Vpn};

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

async fn notify_admins(bot: &Bot, cfg: &Config, text: String) {
    for id in &cfg.admin_ids {
        let _ = bot.send_message(ChatId(*id), &text).await;
    }
}

fn vpn_problem(report: &crate::vpn::wire::CheckReport) -> Option<String> {
    if report.ok {
        return None;
    }
    Some(format!(
        "service={} interface={} port={} module={} firewall={}",
        report.service.active,
        report.interface.present,
        report.port.listening,
        report.module.loaded,
        !report.firewall.ufw_active || report.firewall.port_allowed
    ))
}

async fn tick(bot: &Bot, cfg: &Config, vpn: &Vpn, store: &Store, now: i64) {
    match vpn.check().await {
        Ok(report) => {
            if let Some(details) = vpn_problem(&report) {
                if store.update_monitor_state("vpn", "error", Some(&details), now) {
                    notify_admins(
                        bot,
                        cfg,
                        format!("🚨 Мониторинг: AmneziaWG работает с ошибками\n{details}"),
                    )
                    .await;
                }
            } else if store.update_monitor_state("vpn", "ok", None, now) {
                notify_admins(
                    bot,
                    cfg,
                    "✅ Мониторинг: AmneziaWG снова работает штатно.".into(),
                )
                .await;
            }
        }
        Err(error) => {
            let details = error.to_string();
            if store.update_monitor_state("vpn", "error", Some(&details), now) {
                notify_admins(
                    bot,
                    cfg,
                    format!("🚨 Мониторинг: ошибка AmneziaWG\n{details}"),
                )
                .await;
            }
        }
    }

    for server in store.vpn_servers() {
        let Some(paid_until) = server.paid_until else {
            continue;
        };
        let seconds = paid_until - now;
        let days = seconds.div_euclid(86_400);
        let threshold = match days {
            i64::MIN..=-1 => Some(-1),
            0 => Some(0),
            1 => Some(1),
            2..=3 => Some(3),
            4..=7 => Some(7),
            8..=14 => Some(14),
            _ => None,
        };
        let Some(threshold) = threshold else {
            continue;
        };
        if !store.mark_server_billing_notification(server.id, paid_until, threshold, now) {
            continue;
        }
        let cost = server
            .cost_minor
            .map(|v| {
                format!(
                    "{:.2} {}",
                    v as f64 / 100.0,
                    server.currency.as_deref().unwrap_or("")
                )
            })
            .unwrap_or_else(|| "не указана".into());
        let urgency = if days < 0 {
            format!("просрочено на {} дн.", -days)
        } else if days == 0 {
            "сегодня".into()
        } else {
            format!("через {days} дн.")
        };
        notify_admins(bot,cfg,format!("💳 Оплата VPN-сервера {urgency}\n\n🖥 {}\n🏢 {}\n🌐 {}\n📅 Оплачен до: {}\n💰 Сумма: {}",server.name,server.provider,server.public_ip,crate::calendar::format_date(paid_until),cost)).await;
    }

    let Some(parent) = cfg.db_path.parent() else {
        return;
    };
    let dir = parent.join("backups");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join(format!("awgram-{}.db", now / 86_400));
    if !path.exists() {
        match store.backup_database(&path) {
            Ok(()) => {
                if store.update_monitor_state("database_backup", "ok", None, now) {
                    notify_admins(
                        bot,
                        cfg,
                        "✅ Автоматическое резервное копирование БД работает.".into(),
                    )
                    .await;
                }
            }
            Err(error) => {
                let details = error.to_string();
                if store.update_monitor_state("database_backup", "error", Some(&details), now) {
                    notify_admins(
                        bot,
                        cfg,
                        format!("🚨 Не удалось создать резервную копию БД: {details}"),
                    )
                    .await;
                }
            }
        }
    }
}

pub async fn run(bot: Bot, cfg: Arc<Config>, vpn: Arc<Vpn>, store: Arc<Store>) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
    loop {
        interval.tick().await;
        tick(&bot, &cfg, &vpn, &store, now_epoch()).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_with_internal_failure_is_not_healthy() {
        let mut report = crate::vpn::wire::CheckReport {
            ok: false,
            ..Default::default()
        };
        report.service.active = true;
        report.interface.present = true;
        report.module.loaded = true;
        assert!(vpn_problem(&report).is_some());
        report.ok = true;
        assert!(vpn_problem(&report).is_none());
    }
}
