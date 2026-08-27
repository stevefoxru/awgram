use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::store::Store;

#[derive(Clone)]
struct PortalState {
    store: Arc<Store>,
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
        "awgram_session={session}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}",
        30 * 86_400
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

pub async fn run(bind: &str, store: Arc<Store>) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let app = Router::new()
        .route("/", get(index))
        .route("/login", get(login))
        .route("/api/me", get(me))
        .route("/api/logout", post(logout))
        .with_state(PortalState { store });
    tracing::info!(bind, "внутренний личный кабинет запущен");
    axum::serve(listener, app).await
}

const INDEX_HTML: &str = r##"<!doctype html>
<html lang="ru"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="theme-color" content="#101828"><title>ZuevVPN — личный кабинет</title>
<style>
:root{color-scheme:dark;--bg:#07101f;--card:#101c30;--line:#22324d;--text:#f5f7fb;--muted:#9fb0c9;--accent:#66e3c4;--bad:#ff7f86}*{box-sizing:border-box}body{margin:0;background:radial-gradient(circle at 80% 0,#17375b 0,transparent 40%),var(--bg);color:var(--text);font:16px/1.45 system-ui,-apple-system,sans-serif}.wrap{max-width:1080px;margin:auto;padding:32px 18px 64px}header{display:flex;justify-content:space-between;align-items:center;margin-bottom:28px}.brand{font-size:22px;font-weight:800}.brand span{color:var(--accent)}button{border:1px solid var(--line);background:#14233a;color:var(--text);border-radius:12px;padding:10px 14px;cursor:pointer}.hero{display:grid;grid-template-columns:2fr 1fr;gap:18px}.card{background:color-mix(in srgb,var(--card) 92%,transparent);border:1px solid var(--line);border-radius:22px;padding:22px;box-shadow:0 20px 55px #0004}h1{font-size:clamp(28px,5vw,52px);line-height:1.05;margin:8px 0 18px}.muted{color:var(--muted)}.balance{font-size:34px;font-weight:800;margin-top:12px}.keys{display:grid;gap:14px;margin-top:18px}.key{display:grid;grid-template-columns:1.5fr 1fr auto;gap:18px;align-items:center}.status{color:var(--accent);font-weight:700}.offline{color:var(--bad)}.metric{text-align:right}.empty{text-align:center;padding:48px}.login{max-width:560px;margin:14vh auto}.pill{display:inline-block;padding:5px 10px;border:1px solid var(--line);border-radius:99px;color:var(--accent);font-size:13px}@media(max-width:720px){.hero{grid-template-columns:1fr}.key{grid-template-columns:1fr}.metric{text-align:left}}
</style></head><body><main class="wrap" id="app"><section class="card login"><span class="pill">ZuevVPN ID</span><h1>Открываем кабинет…</h1><p class="muted">Если ссылка устарела, запросите новую в Telegram-боте.</p></section></main>
<script>
const fmt=n=>{const u=['Б','КБ','МБ','ГБ','ТБ'];let i=0;while(n>=1024&&i<u.length-1){n/=1024;i++}return `${n.toFixed(i?1:0)} ${u[i]}`};
const esc=s=>String(s).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
fetch('/api/me').then(async r=>{if(!r.ok)throw 0;return r.json()}).then(d=>{const keys=d.keys.map(k=>`<article class="card key"><div><div class="status ${k.server_status==='offline'?'offline':''}">${k.server_status==='offline'?'Недоступен':'Активен'}</div><h2>${esc(k.device)}</h2><div class="muted">${esc(k.name)} · ${esc(k.location)} · AWG 1.0</div></div><div><div class="muted">Последняя активность</div><strong>${k.last_handshake?new Date(k.last_handshake*1000).toLocaleString('ru-RU'):'Нет данных'}</strong></div><div class="metric"><div class="muted">Трафик</div><strong>${fmt(k.rx+k.tx)}</strong></div></article>`).join('');document.querySelector('#app').innerHTML=`<header><div class="brand">Zuev<span>VPN</span></div><button id="logout">Выйти</button></header><section class="hero"><article class="card"><span class="pill">Личный кабинет</span><h1>${esc(d.display_name||'Пользователь')}</h1><p class="muted">Ваши подключения, состояние серверов и статистика использования.</p></article><article class="card"><div class="muted">Внутренний баланс</div><div class="balance">${(d.balance_kopecks/100).toLocaleString('ru-RU',{style:'currency',currency:'RUB'})}</div><div class="muted">Активных ключей: ${d.keys.length}</div></article></section><section class="keys">${keys||'<article class="card empty">У вас пока нет активных ключей.</article>'}</section>`;document.querySelector('#logout').onclick=()=>fetch('/api/logout',{method:'POST'}).then(()=>location.reload())}).catch(()=>{document.querySelector('#app').innerHTML='<section class="card login"><span class="pill">Требуется вход</span><h1>Откройте кабинет через Telegram</h1><p class="muted">В боте нажмите «Кабинет → Открыть веб-кабинет». Ссылка одноразовая и действует 15 минут.</p></section>'});
</script></body></html>"##;
