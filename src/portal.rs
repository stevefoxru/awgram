use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::Request;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::{self, Next};
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

async fn frontend_css() -> ([(&'static str, &'static str); 1], &'static str) {
    (
        [(header::CONTENT_TYPE.as_str(), "text/css; charset=utf-8")],
        APP_CSS,
    )
}

async fn frontend_js() -> ([(&'static str, &'static str); 1], &'static str) {
    (
        [(
            header::CONTENT_TYPE.as_str(),
            "text/javascript; charset=utf-8",
        )],
        APP_JS,
    )
}

async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    let values = [
        ("cache-control", "no-store, max-age=0"),
        ("pragma", "no-cache"),
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "no-referrer"),
        (
            "content-security-policy",
            "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
        ),
    ];
    for (name, value) in values {
        if let Ok(value) = value.parse() {
            headers.insert(name, value);
        }
    }
    headers.insert(
        "permissions-policy",
        "camera=(), microphone=(), geolocation=(), payment=()"
            .parse()
            .expect("static header is valid"),
    );
    response
}

fn same_site_request(headers: &HeaderMap) -> bool {
    headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
        .is_none_or(|value| matches!(value, "same-origin" | "same-site" | "none"))
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
        "awgram_session={session}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}{}",
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
    match state.store.portal_overview(user_id, now_epoch()) {
        Some(mut overview) => {
            for key in &mut overview.keys {
                key.expires_at = state.vpn.client_expiry(&key.name);
            }
            Json(overview).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn portal_session(State(state): State<PortalState>, headers: HeaderMap) -> Response {
    let Some(user_id) =
        session(&headers).and_then(|value| state.store.portal_user_id(value, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    Json(serde_json::json!({
        "user_id": user_id,
        "role": if state.admin_ids.contains(&user_id) { "owner" } else { "customer" },
        "is_admin": state.admin_ids.contains(&user_id),
    }))
    .into_response()
}

async fn admin_overview(State(state): State<PortalState>, headers: HeaderMap) -> Response {
    let Some(user_id) =
        session(&headers).and_then(|value| state.store.portal_user_id(value, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !state.admin_ids.contains(&user_id) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let now = now_epoch();
    let users = state.store.admin_user_stats(now);
    let clients = state.store.registered_clients();
    let online = clients
        .iter()
        .filter(|client| client.last_handshake.is_some_and(|value| now - value < 300))
        .count();
    let servers = state
        .store
        .vpn_servers()
        .into_iter()
        .map(|server| {
            serde_json::json!({
                "id": server.id,
                "name": server.name,
                "location": server.location,
                "protocol": server.protocol,
                "status": server.status,
                "provisioning": server.enabled_for_provisioning,
                "clients": state.store.server_client_count(server.id),
                "capacity": server.capacity,
            })
        })
        .collect::<Vec<_>>();
    Json(serde_json::json!({
        "users": {"total": users.total, "new_today": users.new_today, "new_30d": users.new_30d, "paying": users.paying, "blocked": users.blocked},
        "keys": {"total": clients.len(), "online": online},
        "servers": servers,
        "payments_pending": state.store.pending_payments().len(),
        "support_open": state.store.open_support_count(),
    }))
    .into_response()
}

async fn logout(State(state): State<PortalState>, headers: HeaderMap) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if let Some(value) = session(&headers) {
        state.store.portal_logout(value, now_epoch());
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        format!(
            "awgram_session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0{}",
            if state.secure_cookie { "; Secure" } else { "" }
        )
        .parse()
        .expect("cookie is valid"),
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

#[derive(serde::Deserialize)]
struct NotificationRequest {
    kind: String,
    enabled: bool,
}

async fn update_notifications(
    State(state): State<PortalState>,
    headers: HeaderMap,
    Json(request): Json<NotificationRequest>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(user_id) =
        session(&headers).and_then(|value| state.store.portal_user_id(value, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !state.store.set_notification_preference(
        user_id,
        &request.kind,
        request.enabled,
        now_epoch(),
    ) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    Json(serde_json::json!({"ok":true})).into_response()
}

#[derive(serde::Deserialize)]
struct TopupRequest {
    amount_rubles: i64,
}

async fn create_topup(
    State(state): State<PortalState>,
    headers: HeaderMap,
    Json(request): Json<TopupRequest>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(user_id) =
        session(&headers).and_then(|value| state.store.portal_user_id(value, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !(100..=100_000).contains(&request.amount_rubles) {
        return (
            StatusCode::BAD_REQUEST,
            "Сумма должна быть от 100 до 100 000 ₽",
        )
            .into_response();
    }
    match state.store.create_payment_request(user_id,0,request.amount_rubles*100,"topup",now_epoch()) {
        Some(id) => Json(serde_json::json!({"ok":true,"payment_id":id,"instructions":state.store.payment_instructions()})).into_response(),
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(serde::Deserialize)]
struct ProofRequest {
    proof: String,
}

async fn submit_payment_proof(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<i64>,
    Json(request): Json<ProofRequest>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(user_id) =
        session(&headers).and_then(|value| state.store.portal_user_id(value, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let proof = request.proof.trim();
    if proof.is_empty() || proof.chars().count() > 500 {
        return (
            StatusCode::BAD_REQUEST,
            "Укажите номер операции или короткий комментарий",
        )
            .into_response();
    }
    let owned = state.store.payment_request(id).is_some_and(|payment| {
        payment.user_id == user_id && payment.status == crate::store::PaymentStatus::Pending
    });
    if !owned || !state.store.set_payment_proof(id, user_id, proof) {
        return StatusCode::CONFLICT.into_response();
    }
    for admin_id in state.admin_ids.iter() {
        let _=state.bot.send_message(ChatId(*admin_id),format!("💳 Подтверждение оплаты из веб-кабинета\nЗаявка: #{id}\nПользователь: {user_id}\nКомментарий: {proof}")).await;
    }
    Json(serde_json::json!({"ok":true})).into_response()
}

async fn support(
    State(state): State<PortalState>,
    headers: HeaderMap,
    Json(request): Json<SupportRequest>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
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
    let existing = state
        .store
        .active_support_ticket_for_user_category(user_id, "connection");
    let ticket_id = existing.as_ref().map(|ticket| ticket.id).or_else(|| {
        state
            .store
            .open_support_ticket_in_category(user_id, "connection", message, now_epoch())
    });
    match ticket_id {
        Some(id) => {
            if existing.is_some() {
                state.store.add_support_message(
                    id,
                    user_id,
                    false,
                    (0, 0),
                    Some(message),
                    now_epoch(),
                );
            }
            for admin_id in state.admin_ids.iter() {
                let _ = state
                    .bot
                    .send_message(
                        ChatId(*admin_id),
                        format!(
                            "🆘 {} из веб-кабинета #{id}\nПользователь: {user_id}\n\n{message}",
                            if existing.is_some() {
                                "Дополнение обращения"
                            } else {
                                "Новое обращение"
                            }
                        ),
                    )
                    .await;
            }
            Json(serde_json::json!({"ok":true,"ticket_id":id,"existing":existing.is_some()}))
                .into_response()
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
        .route("/assets/app.css", get(frontend_css))
        .route("/assets/app.js", get(frontend_js))
        .route("/login", get(login))
        .route("/api/session", get(portal_session))
        .route("/api/me", get(me))
        .route("/api/admin/overview", get(admin_overview))
        .route("/api/logout", post(logout))
        .route("/api/keys/{name}/config", get(download_config))
        .route("/api/keys/{name}/qr", get(download_qr))
        .route("/api/support", post(support))
        .route("/api/notifications", post(update_notifications))
        .route("/api/payments/topup", post(create_topup))
        .route("/api/payments/{id}/proof", post(submit_payment_proof))
        .route("/api/payments/webhook", post(acquiring_webhook))
        .layer(middleware::from_fn(security_headers))
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

const INDEX_HTML: &str = include_str!("../frontend/index.html");
const APP_CSS: &str = include_str!("../frontend/app.css");
const APP_JS: &str = include_str!("../frontend/app.js");

/* Previous embedded frontend kept out of the binary by cfg for an easy audit trail. */
#[cfg(any())]
const LEGACY_INDEX_HTML: &str = r##"<!doctype html>
<html lang="ru"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="theme-color" content="#101828"><title>ZuevVPN — личный кабинет</title>
<style>
:root{color-scheme:dark;--bg:#07101f;--card:#101c30;--line:#22324d;--text:#f5f7fb;--muted:#9fb0c9;--accent:#66e3c4;--warn:#ffc857;--bad:#ff7f86}*{box-sizing:border-box}body{margin:0;background:radial-gradient(circle at 80% 0,#17375b 0,transparent 40%),var(--bg);color:var(--text);font:16px/1.45 system-ui,-apple-system,sans-serif}.wrap{max-width:1080px;margin:auto;padding:32px 18px 64px}header{display:flex;justify-content:space-between;align-items:center;margin-bottom:28px}.brand{font-size:22px;font-weight:800}.brand span{color:var(--accent)}button,.action{border:1px solid var(--line);background:#14233a;color:var(--text);border-radius:12px;padding:10px 14px;cursor:pointer;text-decoration:none;display:inline-block}.hero{display:grid;grid-template-columns:2fr 1fr;gap:18px}.card{background:color-mix(in srgb,var(--card) 92%,transparent);border:1px solid var(--line);border-radius:22px;padding:22px;box-shadow:0 20px 55px #0004}h1{font-size:clamp(28px,5vw,52px);line-height:1.05;margin:8px 0 18px}h2{margin:8px 0}.muted{color:var(--muted)}.balance{font-size:34px;font-weight:800;margin-top:12px}.keys{display:grid;gap:14px;margin-top:18px}.key{display:grid;grid-template-columns:1.5fr 1fr auto;gap:18px;align-items:center}.status{color:var(--accent);font-weight:700}.offline{color:var(--bad)}.warn{color:var(--warn)}.metric{text-align:right}.empty{text-align:center;padding:48px}.login{max-width:560px;margin:14vh auto}.pill{display:inline-block;padding:5px 10px;border:1px solid var(--line);border-radius:99px;color:var(--accent);font-size:13px}.actions{display:flex;gap:8px;flex-wrap:wrap;margin-top:14px}.grid2{display:grid;grid-template-columns:1fr 1fr;gap:18px;margin-top:18px}.payment{padding:10px 0;border-bottom:1px solid var(--line)}textarea,input{width:100%;background:#091426;color:var(--text);border:1px solid var(--line);border-radius:12px;padding:12px;margin:10px 0}textarea{min-height:110px}.toggle{display:flex;justify-content:space-between;align-items:center;padding:12px 0;border-bottom:1px solid var(--line)}.toggle input{width:auto}.section-title{margin:30px 4px 12px}@media(max-width:720px){.hero,.grid2{grid-template-columns:1fr}.key{grid-template-columns:1fr}.metric{text-align:left}}
</style></head><body><main class="wrap" id="app"><section class="card login"><span class="pill">ZuevVPN ID</span><h1>Открываем кабинет…</h1><p class="muted">Если ссылка устарела, запросите новую в Telegram-боте.</p></section></main>
<script>
const fmt=n=>{const u=['Б','КБ','МБ','ГБ','ТБ'];let i=0;while(n>=1024&&i<u.length-1){n/=1024;i++}return `${n.toFixed(i?1:0)} ${u[i]}`};
const esc=s=>String(s).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const date=v=>v?new Date(v*1000).toLocaleDateString('ru-RU'):'Бессрочно';
const labels={manual:'Перевод',topup:'Пополнение',acquiring:'Онлайн',pending:'Ожидает',approved:'Подтверждено',rejected:'Отклонено',open:'Ожидает ответа',in_progress:'В работе',closed:'Закрыто',connection:'Подключение'};
fetch('/api/me').then(async r=>{if(!r.ok)throw 0;return r.json()}).then(d=>{
 const now=Math.floor(Date.now()/1000);
 const keys=d.keys.map(k=>{const expired=k.expires_at&&k.expires_at<=now,down=k.server_status!=='online',disabled=k.enabled===false,connected=k.last_handshake&&now-k.last_handshake<300;const health=expired?'Срок истёк':down?'Сервер недоступен':disabled?'Ключ отключён':'Готов к работе';const cls=expired||down||disabled?'offline':'';return `<article class="card key"><div><div class="status ${cls}">${health}</div><h2>${esc(k.device)}</h2><div class="muted">${esc(k.name)} · ${esc(k.location)} · ${k.protocol==='amneziawg-2'?'AWG 2.0':'AWG 1.0'}</div><div class="actions"><a class="action" href="/api/keys/${encodeURIComponent(k.name)}/config">Скачать конфиг</a><a class="action" target="_blank" href="/api/keys/${encodeURIComponent(k.name)}/qr">QR-код</a></div></div><div><div class="muted">Подключение</div><strong class="${connected?'status':''}">${connected?'Устройство подключено':k.last_handshake?'Было '+new Date(k.last_handshake*1000).toLocaleString('ru-RU'):'Подключений не было'}</strong><div class="muted">Срок: ${date(k.expires_at)}</div></div><div class="metric"><div class="muted">Трафик</div><strong>↓ ${fmt(k.rx)} · ↑ ${fmt(k.tx)}</strong></div></article>`}).join('');
 const payments=d.payments.map(p=>`<div class="payment"><strong>#${p.id} · ${(p.amount_kopecks/100).toLocaleString('ru-RU')} ₽</strong><div class="muted">${labels[p.method]||esc(p.method)} · ${labels[p.status]||esc(p.status)} · ${date(p.created_at)}</div></div>`).join('');
 const history=d.balance_history.map(x=>`<div class="payment"><strong>${x.amount_kopecks>0?'+':''}${(x.amount_kopecks/100).toFixed(2)} ₽</strong><div class="muted">${date(x.created_at)} · ${x.kind==='purchase'?'Покупка или продление':x.kind==='refund'?'Возврат':x.kind==='referral'?'Реферальное начисление':x.kind==='topup'?'Пополнение':'Корректировка'}</div></div>`).join('');
 const tickets=d.tickets.map(t=>`<div class="payment"><strong>#${t.id} · ${labels[t.category]||esc(t.category)}</strong><div class="muted">${labels[t.status]||esc(t.status)} · ${date(t.updated_at)}</div></div>`).join('');
 document.querySelector('#app').innerHTML=`<header><div class="brand">Zuev<span>VPN</span></div><button id="logout">Выйти</button></header><section class="hero"><article class="card"><span class="pill">Личный кабинет</span><h1>${esc(d.display_name||'Пользователь')}</h1><p class="muted">Ключи, подключения, финансы и помощь в одном месте.</p><div class="muted">Приглашено: ${d.referral_count} · Вознаграждение: ${d.referral_percent}% · Скидка: ${d.discount_percent?d.discount_percent+'%':'нет'}</div></article><article class="card"><div class="muted">Внутренний баланс</div><div class="balance">${(d.balance_kopecks/100).toLocaleString('ru-RU',{style:'currency',currency:'RUB'})}</div><div class="muted">Ключей: ${d.keys.length}</div></article></section><h2 class="section-title">Подключения</h2><section class="keys">${keys||'<article class="card empty">У вас пока нет ключей.</article>'}</section><section class="grid2"><article class="card"><h2>Баланс</h2>${history||'<p class="muted">Операций пока нет.</p>'}<h2>Пополнение</h2><input id="topupAmount" type="number" min="100" max="100000" placeholder="Сумма, ₽"><button id="topupCreate">Создать заявку</button><textarea id="topupProof" maxlength="500" placeholder="Номер операции или комментарий"></textarea><button id="topupProofSend" disabled>Я оплатил</button><p id="topupResult" class="muted"></p></article><article class="card"><h2>Уведомления</h2><label class="toggle"><span>Окончание подписки</span><input id="notifyExpiry" type="checkbox" ${d.expiry_notifications?'checked':''}></label><label class="toggle"><span>Плановые работы</span><input id="notifyMaintenance" type="checkbox" ${d.maintenance_notifications?'checked':''}></label><p id="notifyResult" class="muted"></p><h2>Обращения</h2>${tickets||'<p class="muted">Обращений пока нет.</p>'}<textarea id="supportText" maxlength="1000" placeholder="Что не работает?"></textarea><button id="supportSend">Отправить</button><p id="supportResult" class="muted"></p></article></section><section class="card" style="margin-top:18px"><h2>Платежные заявки</h2>${payments||'<p class="muted">Заявок пока нет.</p>'}</section>`;
 document.querySelector('#logout').onclick=()=>fetch('/api/logout',{method:'POST'}).then(()=>location.reload());
 document.querySelector('#supportSend').onclick=async()=>{const message=document.querySelector('#supportText').value;const r=await fetch('/api/support',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({message})});if(r.ok){const x=await r.json();document.querySelector('#supportResult').textContent=x.existing?`Обращение #${x.ticket_id} дополнено.`:`Обращение #${x.ticket_id} создано.`}else document.querySelector('#supportResult').textContent='Не удалось отправить обращение.'};
 for(const [id,kind] of [['notifyExpiry','expiry'],['notifyMaintenance','maintenance']])document.querySelector('#'+id).onchange=async e=>{const r=await fetch('/api/notifications',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({kind,enabled:e.target.checked})});document.querySelector('#notifyResult').textContent=r.ok?'Настройки сохранены.':'Не удалось сохранить настройку.'};
 let paymentId=null;document.querySelector('#topupCreate').onclick=async()=>{const amount_rubles=Number(document.querySelector('#topupAmount').value);const r=await fetch('/api/payments/topup',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({amount_rubles})});if(r.ok){const x=await r.json();paymentId=x.payment_id;document.querySelector('#topupResult').textContent=`Заявка #${paymentId}. ${x.instructions}`;document.querySelector('#topupProofSend').disabled=false}else document.querySelector('#topupResult').textContent='Проверьте сумму.'};document.querySelector('#topupProofSend').onclick=async()=>{const proof=document.querySelector('#topupProof').value;const r=await fetch(`/api/payments/${paymentId}/proof`,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({proof})});document.querySelector('#topupResult').textContent=r.ok?'Подтверждение отправлено администратору.':'Не удалось отправить подтверждение.'}
}).catch(()=>{document.querySelector('#app').innerHTML='<section class="card login"><span class="pill">Требуется вход</span><h1>Откройте кабинет через Telegram</h1><p class="muted">В боте нажмите «Кабинет → Открыть веб-кабинет». Ссылка одноразовая и действует 15 минут.</p></section>'});
</script></body></html>"##;

#[cfg(test)]
mod tests {
    use super::same_site_request;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn state_changes_reject_cross_site_browser_requests() {
        let mut headers = HeaderMap::new();
        assert!(same_site_request(&headers));
        headers.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        assert!(same_site_request(&headers));
        headers.insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
        assert!(!same_site_request(&headers));
    }
}
