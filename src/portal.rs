use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::store::Store;
use crate::vpn::Vpn;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use teloxide::prelude::*;

#[derive(Clone)]
struct PortalState {
    store: Arc<Store>,
    vpn: Arc<Vpn>,
    acquiring_webhook_secret: Option<String>,
    bot: Bot,
    admin_ids: Arc<Vec<i64>>,
    secure_cookie: bool,
}

#[derive(serde::Deserialize)]
struct LoginQuery {
    token: String,
}

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or_default()
}

fn session(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix("awgram_session="))
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn login(State(state): State<PortalState>, Query(query): Query<LoginQuery>) -> Response {
    let Some(session) = state.store.activate_portal_token(&query.token, now_epoch()) else {
        return (
            StatusCode::UNAUTHORIZED,
            "Ссылка недействительна или уже использована",
        )
            .into_response();
    };
    let cookie = format!(
        "awgram_session={session}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}{}",
        30 * 86_400,
        if state.secure_cookie { "; Secure" } else { "" }
    );
    let mut response = Redirect::to("/").into_response();
    if let Ok(value) = cookie.parse() {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}

async fn me(State(state): State<PortalState>, headers: HeaderMap) -> Response {
    let Some(user_id) =
        session(&headers).and_then(|value| state.store.portal_user_id(value, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    match state.store.portal_overview(user_id) {
        Some(overview) => Json(overview).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn logout(State(state): State<PortalState>, headers: HeaderMap) -> Response {
    if let Some(value) = session(&headers) {
        state.store.portal_logout(value, now_epoch());
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        "awgram_session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0"
            .parse()
            .expect("static cookie is valid"),
    );
    response
}

async fn client_artifacts(
    state: &PortalState,
    user_id: i64,
    name: &str,
) -> crate::error::Result<crate::vpn::model::AddResult> {
    if state.store.client_owner(name) != Some(user_id) {
        return Err(crate::error::Error::ClientNotFound(name.into()));
    }
    match state.store.client_vpn_server(name) {
        Some(server) if !server.is_local && server.protocol == "amneziawg-panel" => {
            let secret = state
                .store
                .panel_password(server.id)
                .ok_or_else(|| crate::error::Error::Parse("пароль панели не настроен".into()))?;
            state.vpn.panel_existing_files(&server, &secret, name).await
        }
        Some(server) if !server.is_local => {
            if let (Some(node), Some(secret)) = (
                state.store.vpn_node_for_server(server.id),
                state.store.node_secret(server.id),
            ) {
                state
                    .vpn
                    .agent_existing_files(&server, &node, &secret, name)
                    .await
            } else {
                state.vpn.remote_existing_files(&server, name).await
            }
        }
        _ => state.vpn.existing_files(name),
    }
}

async fn download_config(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(name): AxumPath<String>,
) -> Response {
    let Some(user_id) =
        session(&headers).and_then(|value| state.store.portal_user_id(value, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    match client_artifacts(&state, user_id, &name)
        .await
        .and_then(|result| std::fs::read(result.conf_path).map_err(Into::into))
    {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=awgram.conf",
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn download_qr(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(name): AxumPath<String>,
) -> Response {
    let Some(user_id) =
        session(&headers).and_then(|value| state.store.portal_user_id(value, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let bytes = match client_artifacts(&state, user_id, &name).await {
        Ok(result) if !result.qr_path.is_empty() => std::fs::read(result.qr_path).ok(),
        Ok(result) => std::fs::read(result.conf_path).ok().and_then(|conf| {
            let code = qrcode::QrCode::new(conf).ok()?;
            let image = code
                .render::<image::Luma<u8>>()
                .min_dimensions(600, 600)
                .build();
            let mut output = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageLuma8(image)
                .write_to(&mut output, image::ImageFormat::Png)
                .ok()?;
            Some(output.into_inner())
        }),
        Err(_) => None,
    };
    match bytes {
        Some(bytes) => (
            [
                (header::CONTENT_TYPE, "image/png"),
                (
                    header::CONTENT_DISPOSITION,
                    "inline; filename=awgram-qr.png",
                ),
            ],
            bytes,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(serde::Deserialize)]
struct SupportRequest {
    message: String,
}

async fn support(
    State(state): State<PortalState>,
    headers: HeaderMap,
    Json(request): Json<SupportRequest>,
) -> Response {
    let Some(user_id) =
        session(&headers).and_then(|value| state.store.portal_user_id(value, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let message = request.message.trim();
    if message.is_empty() || message.chars().count() > 1000 {
        return (
            StatusCode::BAD_REQUEST,
            "Введите сообщение длиной до 1000 символов",
        )
            .into_response();
    }
    match state
        .store
        .open_support_ticket_in_category(user_id, "connection", message, now_epoch())
    {
        Some(id) => {
            for admin_id in state.admin_ids.iter() {
                let _ = state
                    .bot
                    .send_message(
                        ChatId(*admin_id),
                        format!(
                            "🆘 Новое обращение из веб-кабинета #{id}\nПользователь: {user_id}\n\n{message}"
                        ),
                    )
                    .await;
            }
            Json(serde_json::json!({"ok":true,"ticket_id":id})).into_response()
        }
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(serde::Deserialize)]
struct AcquiringNotice {
    order_id: i64,
    amount_kopecks: i64,
    status: String,
    transaction_id: String,
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect()
}

async fn acquiring_webhook(
    State(state): State<PortalState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(secret) = state.acquiring_webhook_secret.as_deref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let signature = headers
        .get("x-awgram-signature")
        .and_then(|value| value.to_str().ok())
        .and_then(decode_hex);
    let Some(signature) = signature else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let mut mac = match Hmac::<Sha256>::new_from_slice(secret.as_bytes()) {
        Ok(mac) => mac,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    mac.update(&body);
    if mac.verify_slice(&signature).is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let notice: AcquiringNotice = match serde_json::from_slice(&body) {
        Ok(notice) => notice,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if notice.status != "paid" {
        return Json(serde_json::json!({"ok":true,"ignored":true})).into_response();
    }
    if state.store.claim_acquiring_webhook(
        notice.order_id,
        notice.amount_kopecks,
        &notice.transaction_id,
    ) {
        for admin_id in state.admin_ids.iter() {
            let _ = state.bot.send_message(ChatId(*admin_id),format!("🏦 Эквайринг подтвердил оплату заявки #{}.\nСумма: {:.2} ₽\nТранзакция: {}\n\nПроверьте заявку в разделе «Финансы» и выполните выдачу.",notice.order_id,notice.amount_kopecks as f64/100.0,notice.transaction_id)).await;
        }
        Json(serde_json::json!({"ok":true})).into_response()
    } else if state
        .store
        .payment_request(notice.order_id)
        .is_some_and(|payment| {
            payment.amount_kopecks == notice.amount_kopecks
                && payment.proof.as_deref() == Some(&format!("acquiring:{}", notice.transaction_id))
        })
    {
        Json(serde_json::json!({"ok":true,"duplicate":true})).into_response()
    } else {
        (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"ok":false,"error":"order mismatch"})),
        )
            .into_response()
    }
}

pub async fn run(
    bind: &str,
    store: Arc<Store>,
    vpn: Arc<Vpn>,
    acquiring_webhook_secret: Option<String>,
    bot: Bot,
    admin_ids: Vec<i64>,
    secure_cookie: bool,
) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let app = Router::new()
        .route("/", get(index))
        .route("/login", get(login))
        .route("/api/me", get(me))
        .route("/api/logout", post(logout))
        .route("/api/keys/{name}/config", get(download_config))
        .route("/api/keys/{name}/qr", get(download_qr))
        .route("/api/support", post(support))
        .route("/api/payments/webhook", post(acquiring_webhook))
        .with_state(PortalState {
            store,
            vpn,
            acquiring_webhook_secret,
            bot,
            admin_ids: Arc::new(admin_ids),
            secure_cookie,
        });
    tracing::info!(bind, "внутренний личный кабинет запущен");
    axum::serve(listener, app).await
}

const INDEX_HTML: &str = r##"<!doctype html>
<html lang="ru"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="theme-color" content="#101828"><title>ZuevVPN — личный кабинет</title>
<style>
:root{color-scheme:dark;--bg:#07101f;--card:#101c30;--line:#22324d;--text:#f5f7fb;--muted:#9fb0c9;--accent:#66e3c4;--bad:#ff7f86}*{box-sizing:border-box}body{margin:0;background:radial-gradient(circle at 80% 0,#17375b 0,transparent 40%),var(--bg);color:var(--text);font:16px/1.45 system-ui,-apple-system,sans-serif}.wrap{max-width:1080px;margin:auto;padding:32px 18px 64px}header{display:flex;justify-content:space-between;align-items:center;margin-bottom:28px}.brand{font-size:22px;font-weight:800}.brand span{color:var(--accent)}button,.action{border:1px solid var(--line);background:#14233a;color:var(--text);border-radius:12px;padding:10px 14px;cursor:pointer;text-decoration:none;display:inline-block}.hero{display:grid;grid-template-columns:2fr 1fr;gap:18px}.card{background:color-mix(in srgb,var(--card) 92%,transparent);border:1px solid var(--line);border-radius:22px;padding:22px;box-shadow:0 20px 55px #0004}h1{font-size:clamp(28px,5vw,52px);line-height:1.05;margin:8px 0 18px}h2{margin:8px 0}.muted{color:var(--muted)}.balance{font-size:34px;font-weight:800;margin-top:12px}.keys{display:grid;gap:14px;margin-top:18px}.key{display:grid;grid-template-columns:1.5fr 1fr auto;gap:18px;align-items:center}.status{color:var(--accent);font-weight:700}.offline{color:var(--bad)}.metric{text-align:right}.empty{text-align:center;padding:48px}.login{max-width:560px;margin:14vh auto}.pill{display:inline-block;padding:5px 10px;border:1px solid var(--line);border-radius:99px;color:var(--accent);font-size:13px}.actions{display:flex;gap:8px;flex-wrap:wrap;margin-top:14px}.grid2{display:grid;grid-template-columns:1fr 1fr;gap:18px;margin-top:18px}.payment{padding:10px 0;border-bottom:1px solid var(--line)}textarea{width:100%;min-height:110px;background:#091426;color:var(--text);border:1px solid var(--line);border-radius:12px;padding:12px;margin:10px 0}@media(max-width:720px){.hero,.grid2{grid-template-columns:1fr}.key{grid-template-columns:1fr}.metric{text-align:left}}
</style></head><body><main class="wrap" id="app"><section class="card login"><span class="pill">ZuevVPN ID</span><h1>Открываем кабинет…</h1><p class="muted">Если ссылка устарела, запросите новую в Telegram-боте.</p></section></main>
<script>
const fmt=n=>{const u=['Б','КБ','МБ','ГБ','ТБ'];let i=0;while(n>=1024&&i<u.length-1){n/=1024;i++}return `${n.toFixed(i?1:0)} ${u[i]}`};
const esc=s=>String(s).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
fetch('/api/me').then(async r=>{if(!r.ok)throw 0;return r.json()}).then(d=>{const keys=d.keys.map(k=>`<article class="card key"><div><div class="status ${k.server_status==='offline'?'offline':''}">${k.server_status==='offline'?'Недоступен':'Активен'}</div><h2>${esc(k.device)}</h2><div class="muted">${esc(k.name)} · ${esc(k.location)} · AWG 1.0</div><div class="actions"><a class="action" href="/api/keys/${encodeURIComponent(k.name)}/config">Скачать конфиг</a><a class="action" target="_blank" href="/api/keys/${encodeURIComponent(k.name)}/qr">QR-код</a></div></div><div><div class="muted">Последняя активность</div><strong>${k.last_handshake?new Date(k.last_handshake*1000).toLocaleString('ru-RU'):'Нет данных'}</strong></div><div class="metric"><div class="muted">Трафик</div><strong>${fmt(k.rx+k.tx)}</strong></div></article>`).join('');const payments=d.payments.map(p=>`<div class="payment"><strong>#${p.id} · ${(p.amount_kopecks/100).toLocaleString('ru-RU')} ₽</strong><div class="muted">${esc(p.method)} · ${esc(p.status)} · ${new Date(p.created_at*1000).toLocaleDateString('ru-RU')}</div></div>`).join('');document.querySelector('#app').innerHTML=`<header><div class="brand">Zuev<span>VPN</span></div><button id="logout">Выйти</button></header><section class="hero"><article class="card"><span class="pill">Личный кабинет</span><h1>${esc(d.display_name||'Пользователь')}</h1><p class="muted">Ваши подключения, состояние серверов и статистика использования.</p></article><article class="card"><div class="muted">Внутренний баланс</div><div class="balance">${(d.balance_kopecks/100).toLocaleString('ru-RU',{style:'currency',currency:'RUB'})}</div><div class="muted">Активных ключей: ${d.keys.length}</div></article></section><section class="keys">${keys||'<article class="card empty">У вас пока нет активных ключей.</article>'}</section><section class="grid2"><article class="card"><h2>Платежи</h2>${payments||'<p class="muted">Операций пока нет.</p>'}</article><article class="card"><h2>Поддержка</h2><p class="muted">Опишите проблему — обращение появится у администратора.</p><textarea id="supportText" maxlength="1000" placeholder="Что не работает?"></textarea><button id="supportSend">Отправить</button><p id="supportResult" class="muted"></p></article></section>`;document.querySelector('#logout').onclick=()=>fetch('/api/logout',{method:'POST'}).then(()=>location.reload());document.querySelector('#supportSend').onclick=async()=>{const message=document.querySelector('#supportText').value;const r=await fetch('/api/support',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({message})});document.querySelector('#supportResult').textContent=r.ok?`Обращение #${(await r.json()).ticket_id} создано.`:'Не удалось создать обращение.'}}).catch(()=>{document.querySelector('#app').innerHTML='<section class="card login"><span class="pill">Требуется вход</span><h1>Откройте кабинет через Telegram</h1><p class="muted">В боте нажмите «Кабинет → Открыть веб-кабинет». Ссылка одноразовая и действует 15 минут.</p></section>'});
</script></body></html>"##;
