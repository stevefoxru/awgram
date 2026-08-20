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

async fn tick(bot: &Bot, cfg: &Config, vpn: &Vpn, store: &Store, now: i64) {
    match vpn.check().await {
        Ok(_) => {
            if store.update_monitor_state("vpn", "ok", None, now) {
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
