use std::{sync::Arc, time::Duration};

use teloxide::{dispatching::dialogue::InMemStorage, prelude::*};

use crate::{
    bot::{handlers, State},
    config::Config,
    store::Store,
    vpn::Vpn,
};

pub async fn supervise(cfg: Arc<Config>, vpn: Arc<Vpn>, store: Arc<Store>) {
    let mut running: Option<(i64, tokio::task::JoinHandle<()>)> = None;
    loop {
        let configured = store.mirror_bot_config();
        let desired = configured
            .and_then(|(_, path, enabled, revision)| {
                enabled
                    .then(|| std::fs::read_to_string(path).ok().map(|v| (v, revision)))
                    .flatten()
            })
            .or_else(|| cfg.mirror_bot_token.clone().map(|token| (token, 0)));
        let keep = match (&running, &desired) {
            (Some((current, task)), Some((_, revision))) => {
                current == revision && !task.is_finished()
            }
            _ => false,
        };
        if !keep {
            if let Some((_, task)) = running.take() {
                task.abort();
            }
            if let Some((token, revision)) = desired {
                match token {
                    token if !token.trim().is_empty() => {
                        let bot = Bot::new(token.trim().to_owned());
                        let mirror_cfg = cfg.clone();
                        let mirror_vpn = vpn.clone();
                        let mirror_store = store.clone();
                        running = Some((
                            revision,
                            tokio::spawn(async move {
                                Dispatcher::builder(bot, handlers::schema())
                                    .dependencies(dptree::deps![
                                        InMemStorage::<State>::new(),
                                        mirror_cfg,
                                        mirror_vpn,
                                        mirror_store
                                    ])
                                    .build()
                                    .dispatch()
                                    .await;
                            }),
                        ));
                    }
                    _ => tracing::warn!("файл токена зеркала пуст"),
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
