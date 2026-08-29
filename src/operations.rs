use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use teloxide::prelude::*;

use crate::{config::Config, store::Store, vpn::Vpn};

static MONITOR_RUN: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

async fn notify_admins(bot: &Bot, cfg: &Config, text: String) -> bool {
    let mut delivered = false;
    for id in &cfg.admin_ids {
        if bot.send_message(ChatId(*id), &text).await.is_ok() {
            delivered = true;
        }
    }
    delivered
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

fn replacement_is_stale(created_at: i64, now: i64) -> bool {
    created_at <= now.saturating_sub(24 * 60 * 60)
}

fn capacity_status(assigned: i64, capacity: i64) -> (&'static str, i64) {
    let percent = if capacity > 0 {
        assigned.max(0).saturating_mul(100) / capacity
    } else {
        100
    };
    let status = if percent >= 100 {
        "error"
    } else if percent >= 90 {
        "warning90"
    } else if percent >= 80 {
        "warning80"
    } else {
        "ok"
    };
    (status, percent)
}

async fn tick(bot: &Bot, cfg: &Config, vpn: &Vpn, store: &Store, now: i64) {
    let removed_sessions = store.prune_portal_sessions(now);
    if removed_sessions > 0 {
        tracing::info!(removed_sessions, "устаревшие сессии ЛК удалены");
    }

    for server in store.vpn_servers() {
        let assigned = store.server_client_count(server.id);
        let (status, percent) = capacity_status(assigned, server.capacity);
        let free = server.capacity.saturating_sub(assigned).max(0);
        let details = format!(
            "использовано {assigned}/{} ({percent}%), свободно {free}",
            server.capacity
        );
        let key = format!("vpn-server-{}-capacity", server.id);
        if store.update_monitor_state(&key, status, Some(&details), now) {
            let text = match status {
                "error" => format!("🔴 Сервер «{}» заполнен\n{details}\n\nНовая выдача на этот узел автоматически заблокирована лимитом. Выберите другой сервер или увеличьте ёмкость после проверки ресурсов.", server.name),
                "warning90" => format!("🟠 Сервер «{}» заполнен на 90%\n{details}\n\nРекомендуется заранее подготовить другой узел.", server.name),
                "warning80" => format!("🟡 Сервер «{}» заполнен на 80%\n{details}", server.name),
                _ => format!("✅ Нагрузка сервера «{}» вернулась в безопасный диапазон\n{details}", server.name),
            };
            notify_admins(bot, cfg, text).await;
        }
    }
    let stale_replacements = store
        .pending_key_replacements()
        .into_iter()
        .filter(|operation| replacement_is_stale(operation.created_at, now))
        .collect::<Vec<_>>();
    if stale_replacements.is_empty() {
        store.update_monitor_state("stale-key-replacements", "ok", None, now);
    } else {
        let ids = stale_replacements
            .iter()
            .take(20)
            .map(|operation| format!("#{}", operation.id))
            .collect::<Vec<_>>()
            .join(", ");
        let details = format!("count={} ids={ids}", stale_replacements.len());
        if store.update_monitor_state("stale-key-replacements", "warning", Some(&details), now) {
            notify_admins(
                bot,
                cfg,
                format!(
                    "⚠️ Есть замены ключей без подтверждения более 24 часов: {}\nОперации: {ids}\n\nОткройте админ-панель → «Операции».",
                    stale_replacements.len()
                ),
            )
            .await;
        }
    }
    if !cfg.controller_only {
        match vpn.check().await {
            Ok(report) => {
                if let Some(details) = vpn_problem(&report) {
                    store.set_local_server_status("warning", now);
                    if store.update_monitor_state("vpn", "error", Some(&details), now) {
                        notify_admins(
                            bot,
                            cfg,
                            format!("🚨 Мониторинг: AmneziaWG работает с ошибками\n{details}"),
                        )
                        .await;
                    }
                } else {
                    store.set_local_server_status("online", now);
                    if store.update_monitor_state("vpn", "ok", None, now) {
                        notify_admins(
                            bot,
                            cfg,
                            "✅ Мониторинг: AmneziaWG снова работает штатно.".into(),
                        )
                        .await;
                    }
                }
            }
            Err(error) => {
                store.set_local_server_status("offline", now);
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
    }

    for server in store
        .vpn_servers()
        .into_iter()
        .filter(|server| !server.is_local)
    {
        let remote_health = if server.protocol == "amneziawg-panel" {
            match store.panel_password(server.id) {
                Some(secret) => match vpn.panel_clients(&server, &secret).await {
                    Ok(clients) => {
                        store.update_monitor_state(
                            &format!("vpn-server-{}-panel-format", server.id),
                            "ok",
                            None,
                            now,
                        );
                        let samples = clients
                            .iter()
                            .map(|client| crate::store::Sample {
                                name: client.name.clone(),
                                ip: client.address.clone(),
                                rx: client.transfer_rx,
                                tx: client.transfer_tx,
                                last_handshake: client.last_handshake_epoch(),
                            })
                            .collect::<Vec<_>>();
                        store.ingest_panel(server.id, now, &samples);
                        let inventory = clients
                            .iter()
                            .map(|client| crate::store::InventoryItem {
                                remote_id: client.id.clone(),
                                name: client.name.clone(),
                                enabled: client.enabled,
                                rx: client.transfer_rx,
                                tx: client.transfer_tx,
                                last_handshake: client.last_handshake_epoch(),
                            })
                            .collect::<Vec<_>>();
                        let report = store.reconcile_inventory(server.id, now, &inventory);
                        if !report.panel_only.is_empty()
                            || !report.database_only.is_empty()
                            || !report.wrong_server.is_empty()
                            || !report.duplicates.is_empty()
                        {
                            let details = format!(
                                "panel_only={} database_only={} wrong_server={} duplicates={}",
                                report.panel_only.len(),
                                report.database_only.len(),
                                report.wrong_server.len(),
                                report.duplicates.len()
                            );
                            let key = format!("vpn-server-{}-inventory", server.id);
                            if store.update_monitor_state(&key, "warning", Some(&details), now) {
                                notify_admins(bot,cfg,format!("🧾 Сверка ключей «{}» обнаружила расхождения\n{}\n\nОткройте карточку сервера → «Сверить ключи с базой».",server.name,details)).await;
                            }
                        } else {
                            store.update_monitor_state(
                                &format!("vpn-server-{}-inventory", server.id),
                                "ok",
                                None,
                                now,
                            );
                        }
                        Ok(true)
                    }
                    Err(list_error) => match vpn.panel_probe(&server, &secret).await {
                        Ok(probe) => {
                            let details =
                                format!("list_error={list_error}; probe={}", probe.response_format);
                            let key = format!("vpn-server-{}-panel-format", server.id);
                            if store.update_monitor_state(&key, "warning", Some(&details), now) {
                                notify_admins(
                                    bot,
                                    cfg,
                                    format!(
                                        "⚠️ Панель «{}» доступна, но формат списка клиентов изменился\n{}\n\nVPN продолжает считаться доступным; SSH-мост для панели не требуется.",
                                        server.name, probe.response_format
                                    ),
                                )
                                .await;
                            }
                            Ok(true)
                        }
                        Err(probe_error) => Err(crate::error::Error::Parse(format!(
                            "список клиентов: {list_error}; проверка панели: {probe_error}"
                        ))),
                    },
                },
                None => Err(crate::error::Error::Parse(
                    "пароль панели не настроен".into(),
                )),
            }
        } else if let (Some(node), Some(secret)) = (
            store.vpn_node_for_server(server.id),
            store.node_secret(server.id),
        ) {
            vpn.agent_status(&server, &node, &secret).await
        } else {
            vpn.remote_status(&server).await
        };
        match remote_health {
            Ok(true) => {
                if server.status == "maintenance" {
                    // Миграция/установка требует ручной проверки тестового
                    // конфига. Health-check не должен сам включать выдачу или
                    // назначать непроверенный узел основным.
                    store.update_monitor_state(
                        &format!("vpn-server-{}-maintenance", server.id),
                        "maintenance",
                        Some("VPN отвечает; ожидается ручная проверка"),
                        now,
                    );
                    continue;
                }
                let became_ready = server.status != "online";
                store.set_server_status(server.id, "online", now);
                let monitor_key = format!("vpn-server-{}", server.id);
                let recovered = store.update_monitor_state(&monitor_key, "ok", None, now);
                if became_ready && recovered {
                    notify_admins(
                        bot,
                        cfg,
                        format!("✅ VPS «{}» снова доступен.", server.name),
                    )
                    .await;
                }
            }
            Ok(false) => {
                if server.status != "maintenance" {
                    store.set_server_status(server.id, "warning", now);
                    let monitor_key = format!("vpn-server-{}", server.id);
                    if store.update_monitor_state(
                        &monitor_key,
                        "error",
                        Some("SSH доступен, VPN-служба неактивна"),
                        now,
                    ) {
                        notify_admins(
                            bot,
                            cfg,
                            format!(
                                "🚨 VPS «{}» доступен, но VPN-служба не работает.",
                                server.name
                            ),
                        )
                        .await;
                    }
                }
            }
            Err(error) => {
                if server.status != "maintenance" {
                    store.set_server_status(server.id, "offline", now);
                    let monitor_key = format!("vpn-server-{}", server.id);
                    let details = error.to_string();
                    if store.update_monitor_state(&monitor_key, "error", Some(&details), now) {
                        notify_admins(
                            bot,
                            cfg,
                            format!("🚨 VPS «{}» недоступен: {error}", server.name),
                        )
                        .await;
                    }
                }
            }
        }
    }

    if !cfg.controller_only
        && !store.local_migration_notice_sent()
        && vpn
            .local_legacy_migration("status")
            .await
            .is_ok_and(|value| value.contains("\"status\":\"complete\""))
    {
        let mut users = 0usize;
        let mut files = 0usize;
        for user_id in store.all_user_ids() {
            let names = store.user_client_names(user_id);
            if names.is_empty() {
                continue;
            }
            let chat = ChatId(user_id);
            if bot.send_message(chat, "🔄 Сервер переведён на AmneziaWG 1.0\n\nСтарые подключения больше не работают. Удалите старый профиль из приложения и импортируйте новые конфигурации ниже.").await.is_ok() {
                users += 1;
            }
            for name in names {
                if let Ok(result) = vpn.existing_files(&name) {
                    if crate::bot::render::send_client_files(
                        bot,
                        chat,
                        store.lang(user_id),
                        &result,
                    )
                    .await
                    .is_ok()
                    {
                        files += 1;
                    }
                }
            }
        }
        store.set_local_migration_notice_sent(true);
        notify_admins(
            bot,
            cfg,
            format!("✅ Локальная миграция на AWG 1.0 завершена\nНовые конфигурации отправлены: пользователей {users}, ключей {files}."),
        )
        .await;
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
        let urgency = match days.cmp(&0) {
            std::cmp::Ordering::Less => format!("просрочено на {} дн.", -days),
            std::cmp::Ordering::Equal => "сегодня".into(),
            std::cmp::Ordering::Greater => format!("через {days} дн."),
        };
        notify_admins(bot,cfg,format!("💳 Оплата VPN-сервера {urgency}\n\n🖥 {}\n🏢 {}\n🌐 {}\n📅 Оплачен до: {}\n💰 Сумма: {}",server.name,server.provider,server.public_ip,crate::calendar::format_date(paid_until),cost)).await;
    }

    let Some(parent) = cfg.db_path.parent() else {
        store.update_monitor_state(
            "database_backup",
            "error",
            Some("у пути базы нет родительского каталога"),
            now,
        );
        return;
    };
    let dir = parent.join("backups");
    if let Err(error) = std::fs::create_dir_all(&dir) {
        let details = format!("не удалось подготовить каталог резервных копий: {error}");
        if store.update_monitor_state("database_backup", "error", Some(&details), now) {
            notify_admins(bot, cfg, format!("🚨 {details}")).await;
        }
        return;
    }
    let path = dir.join(format!("awgram-{}.db", now / 86_400));
    let result = if path.exists() {
        Ok(())
    } else {
        store
            .backup_database(&path)
            .map_err(|error| error.to_string())
    }
    .and_then(|()| {
        Store::verify_database_backup(&path).map(|size| {
            (
                size,
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("backup.db")
                    .to_string(),
            )
        })
    });
    match result {
        Ok((size, name)) => {
            let details = format!("{name} · {size} байт · SQLite quick_check=ok");
            if store.update_monitor_state("database_backup", "ok", Some(&details), now) {
                notify_admins(
                    bot,
                    cfg,
                    format!("✅ Резервное копирование БД восстановлено.\n{details}"),
                )
                .await;
            }
        }
        Err(details) => {
            if store.update_monitor_state("database_backup", "error", Some(&details), now) {
                notify_admins(
                    bot,
                    cfg,
                    format!("🚨 Резервная копия БД не прошла проверку: {details}"),
                )
                .await;
            }
        }
    }
}

/// Немедленный безопасный проход штатного мониторинга из админ-панели.
pub async fn run_once(bot: &Bot, cfg: &Config, vpn: &Vpn, store: &Store) {
    let _guard = MONITOR_RUN.lock().await;
    tick(bot, cfg, vpn, store, now_epoch()).await;
}

pub async fn run(bot: Bot, cfg: Arc<Config>, vpn: Arc<Vpn>, store: Arc<Store>) {
    let version = env!("CARGO_PKG_VERSION");
    let previous = store.runtime_version();
    if previous.as_deref() != Some(version) {
        let text = previous.map_or_else(
            || format!("✅ Бот успешно запущен после установки или обновления.\nВерсия: v{version}"),
            |old| format!("✅ Бот успешно обновлён\n\nv{old} → v{version}\nСлужба запущена и принимает команды."),
        );
        if notify_admins(&bot, &cfg, text).await {
            store.set_runtime_version(version);
        }
    }
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
    loop {
        interval.tick().await;
        run_once(&bot, &cfg, &vpn, &store).await;
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

    #[test]
    fn replacement_becomes_stale_after_one_day() {
        let now = 200_000;
        assert!(!replacement_is_stale(now - 86_399, now));
        assert!(replacement_is_stale(now - 86_400, now));
        assert!(!replacement_is_stale(now + 60, now));
    }

    #[test]
    fn capacity_thresholds_are_stable() {
        assert_eq!(capacity_status(79, 100), ("ok", 79));
        assert_eq!(capacity_status(80, 100), ("warning80", 80));
        assert_eq!(capacity_status(90, 100), ("warning90", 90));
        assert_eq!(capacity_status(100, 100), ("error", 100));
        assert_eq!(capacity_status(120, 100), ("error", 120));
    }
}
