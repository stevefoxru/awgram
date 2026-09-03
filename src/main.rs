use std::path::PathBuf;
use std::sync::Arc;

use teloxide::dispatching::dialogue::InMemStorage;
use teloxide::prelude::*;

use awgram::bot::{handlers, State};
use awgram::config::Config;
use awgram::store::Store;
use awgram::vpn::Vpn;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cfg_path = std::env::var("AWGRAM_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/etc/awgram/config.toml"));

    let cfg = match Config::load(&cfg_path) {
        Ok(c) => Arc::new(c),
        Err(e) => {
            tracing::error!(error = %e, path = %cfg_path.display(), "не удалось загрузить конфиг");
            std::process::exit(1);
        }
    };
    tracing::info!(admins = cfg.admin_ids.len(), "конфиг загружен");

    let bot = Bot::new(&cfg.bot_token);
    let vpn = Arc::new(Vpn::from_config(&cfg));
    let store = match Store::open(&cfg.db_path) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            tracing::error!(error = %e, path = %cfg.db_path.display(), "не удалось открыть БД");
            std::process::exit(1);
        }
    };
    if let Some(bind) = cfg.portal_bind.clone() {
        let portal_store = store.clone();
        let portal_vpn = vpn.clone();
        let webhook_secret = cfg.acquiring_webhook_secret.clone();
        let portal_bot = bot.clone();
        let portal_admins = cfg.admin_ids.clone();
        let portal_smtp = cfg.smtp.clone();
        let secure_cookie = cfg
            .portal_public_url
            .as_deref()
            .is_some_and(|url| url.starts_with("https://"));
        tokio::spawn(async move {
            if let Err(error) = awgram::portal::run(
                &bind,
                portal_store,
                portal_vpn,
                portal_bot,
                awgram::portal::PortalOptions {
                    acquiring_webhook_secret: webhook_secret,
                    admin_ids: portal_admins,
                    secure_cookie,
                    smtp: portal_smtp,
                },
            )
            .await
            {
                tracing::error!(%error, %bind, "личный кабинет остановлен");
            }
        });
    }
    store.migrate_state_json(&cfg.state_file);
    if cfg.controller_only {
        let removed = store.remove_empty_local_vpn_servers();
        tracing::info!(
            removed,
            "запущен режим отдельного контроллера без локального VPN"
        );
    } else {
        let hostname = std::env::var("HOSTNAME")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
            .unwrap_or_else(|| "local-vpn".into());
        if let Some(id) = store.ensure_local_vpn_server(
            hostname.trim(),
            cfg.admin_ids.first().copied().unwrap_or_default(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|value| value.as_secs() as i64)
                .unwrap_or_default(),
        ) {
            tracing::info!(
                server_id = id,
                hostname = hostname.trim(),
                "локальный VPN-сервер зарегистрирован"
            );
        }
    }

    if !cfg.controller_only {
        tokio::spawn(awgram::collector::run(vpn.clone(), store.clone()));
    }
    tokio::spawn(awgram::subscriptions::run(
        bot.clone(),
        vpn.clone(),
        store.clone(),
    ));
    tokio::spawn(awgram::operations::run(
        bot.clone(),
        cfg.clone(),
        vpn.clone(),
        store.clone(),
    ));

    tracing::info!("запуск long polling");
    Dispatcher::builder(bot, handlers::schema())
        .dependencies(dptree::deps![InMemStorage::<State>::new(), cfg, vpn, store])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
