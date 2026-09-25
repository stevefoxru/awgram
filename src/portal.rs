use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::Request;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};

use crate::store::Store;
use crate::vpn::Vpn;
use hmac::{Hmac, Mac};
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
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
    smtp: Option<crate::config::SmtpConfig>,
    public_url: String,
}

pub struct PortalOptions {
    pub acquiring_webhook_secret: Option<String>,
    pub admin_ids: Vec<i64>,
    pub secure_cookie: bool,
    pub smtp: Option<crate::config::SmtpConfig>,
    pub public_url: Option<String>,
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

fn session_response(session: &str, secure: bool) -> Response {
    let cookie = format!(
        "awgram_session={session}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}{}",
        30 * 86_400,
        if secure { "; Secure" } else { "" }
    );
    let mut response = Json(serde_json::json!({"ok":true})).into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, cookie.parse().expect("cookie is valid"));
    response
}

async fn send_email_code(smtp: crate::config::SmtpConfig, email: String, code: String) -> bool {
    tokio::task::spawn_blocking(move || {
        let from: Mailbox = smtp.from.parse().ok()?;
        let to: Mailbox = email.parse().ok()?;
        let message = Message::builder().from(from).to(to).subject("Код входа ZuevVPN")
            .body(format!("Код подтверждения ZuevVPN: {code}\n\nОн действует 10 минут. Если вы не запрашивали код, просто проигнорируйте письмо.")).ok()?;
        let transport = SmtpTransport::relay(&smtp.host).ok()?.port(smtp.port)
            .credentials(Credentials::new(smtp.username, smtp.password)).build();
        transport.send(&message).ok()?;
        Some(())
    }).await.ok().flatten().is_some()
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn robots(State(state): State<PortalState>) -> Response {
    let body = format!(
        "User-agent: *\nAllow: /\nDisallow: /api/\nDisallow: /login\nSitemap: {}/sitemap.xml\n",
        state.public_url.trim_end_matches('/')
    );
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], body).into_response()
}

async fn sitemap(State(state): State<PortalState>) -> Response {
    let url = state.public_url.trim_end_matches('/');
    let pages = [
        "",
        "vpn-for-android",
        "vpn-for-iphone",
        "vpn-for-windows",
        "vpn-for-keenetic",
        "instructions",
        "status",
    ];
    let entries = pages.iter().map(|page| format!("<url><loc>{url}/{page}</loc><changefreq>{}</changefreq><priority>{}</priority></url>",if page.is_empty(){"weekly"}else{"monthly"},if page.is_empty(){"1.0"}else{"0.7"})).collect::<String>();
    let body = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">{entries}</urlset>"
    );
    (
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        body,
    )
        .into_response()
}

async fn marketing_page(
    State(state): State<PortalState>,
    AxumPath(page): AxumPath<String>,
) -> Response {
    let allowed = [
        "vpn-for-android",
        "vpn-for-iphone",
        "vpn-for-windows",
        "vpn-for-keenetic",
        "instructions",
        "status",
    ];
    if !allowed.contains(&page.as_str()) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let (title, description) = match page.as_str() {
        "vpn-for-android" => (
            "VPN для Android — ZPNet",
            "Подключение AmneziaWG на Android: личный кабинет, QR-код и управление ключом.",
        ),
        "vpn-for-iphone" => (
            "VPN для iPhone и iPad — ZPNet",
            "Подключение AmneziaWG на iPhone и iPad с удобным управлением в веб-кабинете.",
        ),
        "vpn-for-windows" => (
            "VPN для Windows — ZPNet",
            "VPN на базе AmneziaWG для компьютера Windows: конфигурация, трафик и поддержка.",
        ),
        "vpn-for-keenetic" => (
            "VPN для роутера Keenetic — ZPNet",
            "Установка VPN-ключа на совместимый роутер Keenetic для защиты домашних устройств.",
        ),
        "instructions" => (
            "Инструкции по установке VPN — ZPNet",
            "Инструкции по установке и подключению AmneziaWG на телефоне, компьютере и роутере.",
        ),
        _ => (
            "Состояние VPN-сервиса — ZPNet",
            "Публичная информация о доступности VPN-сервиса и его компонентов.",
        ),
    };
    let canonical = format!("{}/{page}", state.public_url.trim_end_matches('/'));
    let html = INDEX_HTML
        .replace("ZPNet — VPN для телефона, компьютера и роутера", title)
        .replace("https://zpnet.pro/\">", &format!("{canonical}\">"))
        .replace(
            "VPN на базе AmneziaWG для телефона, компьютера и роутера. Покупка, управление подключениями, трафиком и поддержкой в одном личном кабинете.",
            description,
        );
    Html(html).into_response()
}

async fn manifest() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            "application/manifest+json; charset=utf-8",
        )],
        MANIFEST_JSON,
    )
        .into_response()
}

async fn service_worker() -> Response {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        SERVICE_WORKER,
    )
        .into_response()
}

async fn app_icon() -> Response {
    (
        [(header::CONTENT_TYPE, "image/svg+xml; charset=utf-8")],
        APP_ICON,
    )
        .into_response()
}

async fn public_status(State(state): State<PortalState>) -> Response {
    let servers = state.store.available_vpn_servers();
    let total = servers.len();
    let online = servers
        .iter()
        .filter(|server| server.status == "online")
        .count();
    let components = state
        .store
        .monitor_states()
        .into_iter()
        .map(|item| {
            serde_json::json!({
                "component":item.component,"status":item.status,"checked_at":item.checked_at
            })
        })
        .collect::<Vec<_>>();
    Json(serde_json::json!({
        "status":if total > 0 && online == total {"operational"} else if online > 0 {"degraded"} else {"outage"},
        "servers":{"online":online,"total":total},"components":components,"updated_at":now_epoch()
    })).into_response()
}

async fn catalog(State(state): State<PortalState>) -> Response {
    let mirror = state.store.mirror_bot_config();
    let bot_username = mirror.as_ref().map(|(username, _, _, _)| username.clone());
    let brand = mirror
        .map(|(username, _, _, _)| {
            username
                .trim_start_matches('@')
                .trim_end_matches("_bot")
                .to_string()
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "ZuevVPN".to_string());
    let tariffs = [1_i64, 3, 6, 12]
        .into_iter()
        .filter_map(|months| {
            state
                .store
                .tariff_price_kopecks(months)
                .map(|price| serde_json::json!({"months":months,"price_kopecks":price}))
        })
        .collect::<Vec<_>>();
    let servers = state.store.available_vpn_servers().into_iter().map(|server| serde_json::json!({
        "id":server.id,"name":server.name,"location":server.location,"protocol":match server.protocol.as_str(){"amneziawg-3"=>"AWG 3.1","amneziawg-2"=>"AWG 2.0",_=>"AWG 1.0"},
        "available":server.capacity.saturating_sub(state.store.server_client_count(server.id)).max(0)
    })).collect::<Vec<_>>();
    Json(serde_json::json!({"brand":brand,"bot_username":bot_username,"email_login_enabled":state.smtp.is_some(),"tariffs":tariffs,"servers":servers})).into_response()
}

#[derive(serde::Deserialize)]
struct PurchaseRequest {
    months: i64,
    server_id: i64,
}

#[derive(serde::Deserialize)]
struct RenewalRequest {
    months: Option<i64>,
}

#[derive(serde::Deserialize)]
struct PartnerWalletRequest {
    amount_rubles: f64,
    requisites: Option<String>,
}

async fn partner_wallet_action(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(action): AxumPath<String>,
    Json(input): Json<PartnerWalletRequest>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(user_id) = session(&headers).and_then(|v| state.store.portal_user_id(v, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(partner) = state.store.partner_by_owner(user_id) else {
        return (
            StatusCode::CONFLICT,
            "Партнёрский кабинет ещё не активирован",
        )
            .into_response();
    };
    let amount = (input.amount_rubles * 100.0).round() as i64;
    let now = now_epoch();
    let result = match action.as_str() {
        "transfer" => state
            .store
            .transfer_partner_balance_to_owner(partner.id, user_id, amount, now)
            .map(|_| None),
        "withdraw" => state
            .store
            .create_partner_withdrawal(
                partner.id,
                amount,
                input.requisites.as_deref().unwrap_or(""),
                now,
            )
            .map(Some),
        _ => return StatusCode::BAD_REQUEST.into_response(),
    };
    match result {
        Ok(id) => {
            if let Some(id) = id {
                for admin in state.admin_ids.iter() {
                    let _ = state
                        .bot
                        .send_message(
                            ChatId(*admin),
                            format!(
                                "💸 Заявка партнёра на вывод #{id}\nПартнёр: {}\nСумма: {:.2} ₽",
                                partner.display_name,
                                amount as f64 / 100.0
                            ),
                        )
                        .await;
                }
            }
            Json(serde_json::json!({"ok":true,"withdrawal_id":id})).into_response()
        }
        Err(e) => (StatusCode::CONFLICT, e).into_response(),
    }
}

async fn create_web_renewal(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(name): AxumPath<String>,
    Json(input): Json<RenewalRequest>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(user_id) = session(&headers).and_then(|v| state.store.portal_user_id(v, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if state.store.client_owner(&name) != Some(user_id) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let now = now_epoch();
    let (months, amount, legacy) = if state.store.is_legacy_client(&name, user_id) {
        let year = crate::calendar::year_at(now);
        if now < crate::calendar::start_of_december(year)
            || now > crate::calendar::end_of_year(year)
        {
            return (
                StatusCode::CONFLICT,
                "Льготное продление Legacy-ключей доступно только в декабре",
            )
                .into_response();
        }
        (
            12,
            state
                .store
                .legacy_renewal_price_for_user(user_id, state.store.legacy_renewal_price_kopecks()),
            true,
        )
    } else {
        let months = input.months.unwrap_or(12);
        let Some(amount) = state.store.tariff_price_kopecks(months).filter(|v| *v > 0) else {
            return StatusCode::BAD_REQUEST.into_response();
        };
        (months, amount, false)
    };
    let id = if legacy {
        state
            .store
            .create_legacy_renewal_request(user_id, &name, amount, now)
    } else {
        state
            .store
            .create_renewal_request(user_id, &name, months, amount, now)
    };
    let Some(id) = id else {
        return (
            StatusCode::CONFLICT,
            "По этому ключу уже есть заявка на продление",
        )
            .into_response();
    };
    state.store.add_portal_notification(
        user_id,
        "renewal",
        "Заявка на продление создана",
        &format!("Ключ «{name}», сумма {:.2} ₽.", amount as f64 / 100.0),
        Some("/?view=finance"),
        now,
    );
    for admin in state.admin_ids.iter() {
        let _=state.bot.send_message(ChatId(*admin),format!("📅 Веб-заявка на продление #{id}\nКлюч: {name}\nПользователь: {user_id}\nСумма: {:.2} ₽",amount as f64/100.0)).await;
    }
    Json(serde_json::json!({"ok":true,"payment_id":id,"amount_kopecks":amount,"months":months,"legacy":legacy,"instructions":state.store.payment_instructions()})).into_response()
}

#[derive(serde::Deserialize)]
struct PromoRequest {
    code: String,
}

#[derive(serde::Deserialize)]
struct TransferRequest {
    recipient: String,
}

#[derive(serde::Deserialize)]
struct LegacyRestoreRequest {
    name: String,
    comment: Option<String>,
    code: Option<String>,
}

#[derive(serde::Deserialize)]
struct FolderRequest {
    folder: Option<String>,
}

async fn set_key_folder(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(name): AxumPath<String>,
    Json(input): Json<FolderRequest>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(user_id) = session(&headers).and_then(|v| state.store.portal_user_id(v, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if state
        .store
        .set_portal_folder(user_id, &name, input.folder.as_deref())
    {
        Json(serde_json::json!({"ok":true})).into_response()
    } else {
        StatusCode::BAD_REQUEST.into_response()
    }
}

async fn notifications(State(state): State<PortalState>, headers: HeaderMap) -> Response {
    let Some(user_id) = session(&headers).and_then(|v| state.store.portal_user_id(v, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    Json(serde_json::json!({"items":state.store.portal_notifications(user_id,50)})).into_response()
}

async fn read_notifications(State(state): State<PortalState>, headers: HeaderMap) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(user_id) = session(&headers).and_then(|v| state.store.portal_user_id(v, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    Json(serde_json::json!({"ok":true,"updated":state.store.mark_portal_notifications_read(user_id,now_epoch())})).into_response()
}

async fn create_web_legacy_request(
    State(state): State<PortalState>,
    headers: HeaderMap,
    Json(input): Json<LegacyRestoreRequest>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(user_id) =
        session(&headers).and_then(|value| state.store.portal_user_id(value, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let now = now_epoch();
    if !state.store.legacy_user_eligible(user_id, now) {
        let Some(code) = input.code.as_deref() else {
            return (
                StatusCode::PRECONDITION_REQUIRED,
                "Введите технический промокод",
            )
                .into_response();
        };
        if !state.store.activate_legacy_promo(user_id, code, now) {
            return (
                StatusCode::CONFLICT,
                "Промокод недействителен или приём заявок закрыт",
            )
                .into_response();
        }
    }
    match state
        .store
        .create_legacy_request(user_id, &input.name, input.comment.as_deref(), now)
    {
        Some(id) => {
            state.store.add_portal_notification(
                user_id,
                "recovery",
                "Заявка на восстановление принята",
                &format!("Заявка #{id} для ключа «{}» ожидает проверки.", input.name),
                Some("/?view=restore"),
                now,
            );
            for admin in state.admin_ids.iter() {
                let _=state.bot.send_message(ChatId(*admin),format!("♻️ Новая веб-заявка на восстановление #{id}\nПользователь: ID {user_id}\nЖелаемое имя: {}",input.name)).await;
            }
            Json(serde_json::json!({"ok":true,"request_id":id})).into_response()
        }
        None => (StatusCode::CONFLICT, "Не удалось создать заявку").into_response(),
    }
}

async fn create_web_transfer(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(name): AxumPath<String>,
    Json(input): Json<TransferRequest>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(user_id) =
        session(&headers).and_then(|value| state.store.portal_user_id(value, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let recipient = input.recipient.trim();
    let target = recipient
        .parse::<i64>()
        .ok()
        .and_then(|id| state.store.user(id))
        .or_else(|| {
            state
                .store
                .find_user_by_username(recipient.trim_start_matches('@'))
        });
    let Some(target) = target else {
        return (StatusCode::NOT_FOUND, "Получатель не найден").into_response();
    };
    match state
        .store
        .create_key_transfer(&name, user_id, target.user_id, now_epoch())
    {
        Ok(id) => {
            state.store.add_portal_notification(
                user_id,
                "transfer",
                "Передача ключа создана",
                &format!("Ожидаем подтверждение получателя для ключа «{name}»."),
                Some("/?view=keys"),
                now_epoch(),
            );
            state.store.add_portal_notification(
                target.user_id,
                "transfer",
                "Вам передают VPN-ключ",
                &format!("Пользователь предлагает вам принять ключ «{name}»."),
                Some("/?view=keys"),
                now_epoch(),
            );
            if target.user_id > 0 {
                let _=state.bot.send_message(ChatId(target.user_id),format!("🎁 Вам предлагают принять VPN-ключ «{name}». Откройте веб-кабинет или раздел ключей в боте, чтобы подтвердить передачу.")).await;
            }
            Json(serde_json::json!({"ok":true,"transfer_id":id})).into_response()
        }
        Err(error) => (StatusCode::CONFLICT, error).into_response(),
    }
}

async fn web_transfer_action(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<i64>,
    Json(input): Json<AdminCrmAction>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(user_id) =
        session(&headers).and_then(|value| state.store.portal_user_id(value, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let ok = match input.action.as_str() {
        "accept" => state
            .store
            .accept_key_transfer(id, user_id, now_epoch())
            .is_some(),
        "cancel" => state.store.cancel_key_transfer(id, user_id, now_epoch()),
        _ => false,
    };
    if ok {
        Json(serde_json::json!({"ok":true})).into_response()
    } else {
        (StatusCode::CONFLICT, "Передача недоступна или истекла").into_response()
    }
}

async fn activate_web_promo(
    State(state): State<PortalState>,
    headers: HeaderMap,
    Json(input): Json<PromoRequest>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(user_id) =
        session(&headers).and_then(|value| state.store.portal_user_id(value, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let code = input.code.trim();
    if code.is_empty() || code.len() > 64 {
        return StatusCode::BAD_REQUEST.into_response();
    }
    match state.store.activate_promo(user_id, code, now_epoch()) {
        Some(discount) => {
            Json(serde_json::json!({"ok":true,"discount_percent":discount})).into_response()
        }
        None => (
            StatusCode::CONFLICT,
            "Промокод недействителен или уже использован",
        )
            .into_response(),
    }
}

async fn create_purchase(
    State(state): State<PortalState>,
    headers: HeaderMap,
    Json(input): Json<PurchaseRequest>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(user_id) =
        session(&headers).and_then(|value| state.store.portal_user_id(value, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let available = state
        .store
        .available_vpn_servers()
        .into_iter()
        .any(|server| server.id == input.server_id);
    let Some(base) = state
        .store
        .tariff_price_kopecks(input.months)
        .filter(|price| *price > 0)
    else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if !available {
        return (
            StatusCode::CONFLICT,
            "Выбранный сервер сейчас недоступен для новых ключей",
        )
            .into_response();
    }
    let discount = state
        .store
        .peek_purchase_discount(user_id, now_epoch())
        .clamp(0, 100);
    let amount = base.saturating_mul(100 - discount) / 100;
    match state.store.create_web_purchase_request(
        user_id,
        input.months,
        amount,
        input.server_id,
        now_epoch(),
    ) {
        Some(id) => {
            if discount > 0 {
                state.store.take_promo_discount(user_id);
            }
            state.store.add_portal_notification(
                user_id,
                "payment",
                "Заявка на покупку создана",
                &format!(
                    "Заявка #{id} на сумму {:.2} ₽ ожидает оплаты.",
                    amount as f64 / 100.0
                ),
                Some("/?view=finance"),
                now_epoch(),
            );
            Json(serde_json::json!({"ok":true,"payment_id":id,"amount_kopecks":amount,"discount_percent":discount,"instructions":state.store.payment_instructions()})).into_response()
        }
        None => (
            StatusCode::CONFLICT,
            "У вас уже есть незавершённая заявка или выбранный сервер недоступен",
        )
            .into_response(),
    }
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
    let path = request.uri().path().to_string();
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    let private = path.starts_with("/api/") || path == "/login";
    let cache = if private {
        "no-store, max-age=0"
    } else if path.starts_with("/assets/") {
        "public, max-age=3600"
    } else {
        "public, max-age=300"
    };
    let values = [
        ("cache-control", cache),
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
    if private {
        headers.insert(
            "pragma",
            "no-cache".parse().expect("static header is valid"),
        );
        headers.insert(
            "x-robots-tag",
            "noindex, nofollow, noarchive"
                .parse()
                .expect("static header is valid"),
        );
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

#[derive(serde::Deserialize)]
struct RenameKeyRequest {
    label: String,
}

async fn rename_key(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(name): AxumPath<String>,
    Json(input): Json<RenameKeyRequest>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(user_id) =
        session(&headers).and_then(|value| state.store.portal_user_id(value, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if state.store.set_device_label(&name, user_id, &input.label) {
        Json(serde_json::json!({"ok":true,"label":input.label.trim()})).into_response()
    } else {
        (
            StatusCode::BAD_REQUEST,
            "Название должно содержать 1–40 символов",
        )
            .into_response()
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
        "email": state.store.portal_email(user_id),
        "email_login_enabled": state.smtp.is_some(),
    }))
    .into_response()
}

#[derive(serde::Deserialize)]
struct EmailRequest {
    email: String,
}

#[derive(serde::Deserialize)]
struct EmailConfirm {
    email: String,
    code: String,
}

fn valid_email(value: &str) -> bool {
    let value = value.trim();
    value.len() <= 254
        && value
            .split_once('@')
            .is_some_and(|(a, b)| !a.is_empty() && b.contains('.') && !b.ends_with('.'))
}

async fn request_email_bind(
    State(state): State<PortalState>,
    headers: HeaderMap,
    Json(input): Json<EmailRequest>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(user_id) = session(&headers).and_then(|v| state.store.portal_user_id(v, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(smtp) = state.smtp.clone() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Почтовый шлюз не настроен").into_response();
    };
    if !valid_email(&input.email)
        || state
            .store
            .verified_user_by_email(&input.email)
            .is_some_and(|id| id != user_id)
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let code = format!("{:06}", rand::random_range(0..1_000_000));
    if !state
        .store
        .request_email_code(user_id, &input.email, "bind", &code, now_epoch())
    {
        return (StatusCode::TOO_MANY_REQUESTS, "Повторите через минуту").into_response();
    }
    if !send_email_code(smtp, input.email, code).await {
        return StatusCode::BAD_GATEWAY.into_response();
    }
    Json(serde_json::json!({"ok":true,"expires_in":600})).into_response()
}

async fn confirm_email_bind(
    State(state): State<PortalState>,
    headers: HeaderMap,
    Json(input): Json<EmailConfirm>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(user_id) = session(&headers).and_then(|v| state.store.portal_user_id(v, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if state.store.confirm_email_code(
        user_id,
        &input.email,
        "bind",
        input.code.trim(),
        now_epoch(),
    ) {
        Json(serde_json::json!({"ok":true})).into_response()
    } else {
        (StatusCode::BAD_REQUEST, "Неверный или просроченный код").into_response()
    }
}

async fn request_email_login(
    State(state): State<PortalState>,
    headers: HeaderMap,
    Json(input): Json<EmailRequest>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(smtp) = state.smtp.clone() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Почтовый вход не настроен").into_response();
    };
    if valid_email(&input.email) {
        if let Some(user_id) = state.store.ensure_email_user(&input.email, now_epoch()) {
            let purpose = if state.store.portal_email(user_id).is_some() {
                "login"
            } else {
                "bind"
            };
            let code = format!("{:06}", rand::random_range(0..1_000_000));
            if state
                .store
                .request_email_code(user_id, &input.email, purpose, &code, now_epoch())
            {
                let _ = send_email_code(smtp, input.email, code).await;
            }
        }
    }
    Json(serde_json::json!({"ok":true,"message":"Если адрес подтверждён, письмо уже отправлено"}))
        .into_response()
}

async fn confirm_email_login(
    State(state): State<PortalState>,
    headers: HeaderMap,
    Json(input): Json<EmailConfirm>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(user_id) = state.store.email_user(&input.email) else {
        return (StatusCode::BAD_REQUEST, "Неверный или просроченный код").into_response();
    };
    let purpose = if state.store.portal_email(user_id).is_some() {
        "login"
    } else {
        "bind"
    };
    if !state.store.confirm_email_code(
        user_id,
        &input.email,
        purpose,
        input.code.trim(),
        now_epoch(),
    ) {
        return (StatusCode::BAD_REQUEST, "Неверный или просроченный код").into_response();
    }
    match state.store.create_portal_session(user_id, now_epoch()) {
        Some(value) => session_response(&value, state.secure_cookie),
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
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
        .chain(state.store.archived_vpn_servers())
        .map(|server| {
            let runtime = state.store.server_runtime_summary(server.id, now);
            serde_json::json!({
                "id": server.id,
                "name": server.name,
                "location": server.location,
                "protocol": server.protocol,
                "status": server.status,
                "provisioning": server.enabled_for_provisioning,
                "clients": state.store.server_client_count(server.id),
                "capacity": server.capacity,
                "archived": server.archived_at.is_some(),
                "blocked_by_rkn": server.blocked_by_rkn,
                "operator_unavailable": server.operator_unavailable,
                "unavailable_reason": server.unavailable_reason,
                "telemetry_at": runtime.observed_at,
                "online": runtime.online,
                "rx": runtime.rx,
                "tx": runtime.tx,
            })
        })
        .collect::<Vec<_>>();
    let crm_users = state.store.portal_crm_users(250);
    let payments = state.store.pending_payments().into_iter().take(100).map(|item| serde_json::json!({
        "id":item.id,"user_id":item.user_id,"amount_kopecks":item.amount_kopecks,
        "method":item.method,"proof":item.proof,"created_at":item.created_at,"months":item.months
    })).collect::<Vec<_>>();
    let tickets = state.store.support_tickets("open",100).into_iter()
        .chain(state.store.support_tickets("in_progress",100))
        .map(|item| serde_json::json!({"id":item.id,"user_id":item.user_id,"subject":item.subject,
            "status":item.status,"category":item.category,"priority":item.priority,"updated_at":item.updated_at}))
        .collect::<Vec<_>>();
    Json(serde_json::json!({
        "users": {"total": users.total, "new_today": users.new_today, "new_30d": users.new_30d, "paying": users.paying, "blocked": users.blocked},
        "keys": {"total": clients.len(), "online": online},
        "servers": servers,
        "payments_pending": state.store.pending_payments().len(),
        "revenue_kopecks": state.store.approved_revenue_kopecks(),
        "support_open": state.store.open_support_count(),
        "crm_users":crm_users,
        "crm_payments":payments,
        "crm_tickets":tickets,
        "promos":state.store.admin_promos(100),
        "partner_withdrawals":state.store.pending_partner_withdrawals(100),
        "cleanup": {"enabled": state.store.blocked_key_cleanup_enabled(), "days": state.store.blocked_key_cleanup_days()},
    }))
    .into_response()
}

#[derive(serde::Deserialize)]
struct AdminServerAction {
    action: String,
    reason: Option<String>,
    days: Option<i64>,
}

#[derive(serde::Deserialize)]
struct AdminCrmAction {
    action: String,
}

#[derive(serde::Deserialize)]
struct AdminPaymentAction {
    action: String,
    reason: Option<String>,
}

#[derive(serde::Deserialize)]
struct AdminBalanceAction {
    amount_rubles: f64,
    reason: String,
}

#[derive(serde::Deserialize)]
struct AdminPromoInput {
    code: String,
    discount_percent: i64,
    max_uses: Option<i64>,
    expires_at: Option<i64>,
}

async fn admin_create_promo(
    State(state): State<PortalState>,
    headers: HeaderMap,
    Json(input): Json<AdminPromoInput>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(admin_id) = session(&headers).and_then(|v| state.store.portal_user_id(v, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !state.admin_ids.contains(&admin_id) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let code = input.code.trim().to_uppercase();
    if code.len() < 3
        || code.len() > 32
        || !code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        || !(1..=100).contains(&input.discount_percent)
        || input.max_uses.is_some_and(|v| v < 1)
    {
        return (StatusCode::BAD_REQUEST, "Проверьте код, скидку и лимит").into_response();
    }
    if state.store.create_promo(
        &code,
        input.discount_percent,
        input.max_uses,
        input.expires_at,
        admin_id,
        now_epoch(),
    ) {
        Json(serde_json::json!({"ok":true,"code":code})).into_response()
    } else {
        (StatusCode::CONFLICT, "Такой промокод уже существует").into_response()
    }
}

async fn admin_promo_action(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(code): AxumPath<String>,
    Json(input): Json<AdminCrmAction>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(admin_id) = session(&headers).and_then(|v| state.store.portal_user_id(v, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !state.admin_ids.contains(&admin_id) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let active = match input.action.as_str() {
        "enable" => true,
        "disable" => false,
        _ => return StatusCode::BAD_REQUEST.into_response(),
    };
    if state.store.set_promo_active(&code, active) {
        Json(serde_json::json!({"ok":true})).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

async fn admin_withdrawal_action(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<i64>,
    Json(input): Json<AdminPaymentAction>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(admin_id) = session(&headers).and_then(|v| state.store.portal_user_id(v, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !state.admin_ids.contains(&admin_id) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let paid = match input.action.as_str() {
        "paid" => true,
        "reject" => false,
        _ => return StatusCode::BAD_REQUEST.into_response(),
    };
    if state.store.decide_partner_withdrawal(
        id,
        paid,
        admin_id,
        input.reason.as_deref(),
        now_epoch(),
    ) {
        Json(serde_json::json!({"ok":true})).into_response()
    } else {
        StatusCode::CONFLICT.into_response()
    }
}

async fn admin_user_action(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<i64>,
    Json(input): Json<AdminCrmAction>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(admin_id) =
        session(&headers).and_then(|value| state.store.portal_user_id(value, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !state.admin_ids.contains(&admin_id) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let result = match input.action.as_str() {
        "block" => state.store.set_user_blocked(id, true),
        "unblock" => state.store.set_user_blocked(id, false),
        _ => false,
    };
    if result {
        Json(serde_json::json!({"ok":true})).into_response()
    } else {
        StatusCode::CONFLICT.into_response()
    }
}

async fn admin_user_profile(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    let Some(admin_id) = session(&headers).and_then(|v| state.store.portal_user_id(v, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !state.admin_ids.contains(&admin_id) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(user) = state.store.user(id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(overview) = state.store.portal_overview(id, now_epoch()) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    Json(serde_json::json!({"user":{"id":user.user_id,"display_name":user.display_name,"username":user.username,"created_at":user.created_at,"balance_kopecks":overview.balance_kopecks,"referral_count":overview.referral_count},"keys":overview.keys,"payments":overview.payments,"tickets":overview.tickets,"balance_history":overview.balance_history})).into_response()
}

async fn admin_ticket_action(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<i64>,
    Json(input): Json<AdminCrmAction>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(admin_id) =
        session(&headers).and_then(|value| state.store.portal_user_id(value, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !state.admin_ids.contains(&admin_id) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let result = match input.action.as_str() {
        "take" => state.store.assign_support_ticket(id, admin_id, now_epoch()),
        "close" => state.store.close_support_ticket(id, admin_id, now_epoch()),
        _ => false,
    };
    if result {
        Json(serde_json::json!({"ok":true})).into_response()
    } else {
        StatusCode::CONFLICT.into_response()
    }
}

async fn admin_payment_action(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<i64>,
    Json(input): Json<AdminPaymentAction>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(admin_id) = session(&headers).and_then(|v| state.store.portal_user_id(v, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !state.admin_ids.contains(&admin_id) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(payment) = state.store.payment_request(id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let now = now_epoch();
    let result = match input.action.as_str() {
        "approve" if payment.method == "topup" => {
            let decided = state.store.decide_payment(
                id,
                crate::store::PaymentStatus::Approved,
                admin_id,
                None,
                now,
            );
            decided
                && state.store.add_ledger_entry(
                    payment.user_id,
                    payment.amount_kopecks,
                    "topup",
                    &format!("payment:{id}"),
                    Some("Одобрено в веб-админке"),
                    now,
                )
        }
        "approve" => {
            return (
                StatusCode::CONFLICT,
                "Покупка требует выдачи ключа; подтвердите её в Telegram-админке",
            )
                .into_response()
        }
        "reject" => state.store.reject_payment(
            id,
            admin_id,
            input
                .reason
                .as_deref()
                .unwrap_or("Отклонено администратором"),
            now,
        ),
        _ => false,
    };
    if !result {
        return StatusCode::CONFLICT.into_response();
    }
    let (title, body) = if input.action == "approve" {
        (
            "Пополнение подтверждено",
            format!(
                "Баланс пополнен на {:.2} ₽.",
                payment.amount_kopecks as f64 / 100.0
            ),
        )
    } else {
        (
            "Платёж отклонён",
            input
                .reason
                .unwrap_or_else(|| "Обратитесь в поддержку за подробностями.".into()),
        )
    };
    state.store.add_portal_notification(
        payment.user_id,
        "payment",
        title,
        &body,
        Some("/?view=finance"),
        now,
    );
    if payment.user_id > 0 {
        let _ = state
            .bot
            .send_message(ChatId(payment.user_id), format!("{title}\n\n{body}"))
            .await;
    }
    Json(serde_json::json!({"ok":true})).into_response()
}

async fn admin_balance_action(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<i64>,
    Json(input): Json<AdminBalanceAction>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(admin_id) = session(&headers).and_then(|v| state.store.portal_user_id(v, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !state.admin_ids.contains(&admin_id) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let amount = (input.amount_rubles * 100.0).round() as i64;
    let reason = input.reason.trim();
    if amount == 0 || amount.abs() > 10_000_000 || reason.is_empty() || reason.chars().count() > 300
    {
        return (StatusCode::BAD_REQUEST, "Укажите ненулевую сумму и причину").into_response();
    }
    let now = now_epoch();
    if !state.store.add_ledger_entry(
        id,
        amount,
        "admin_adjustment",
        &format!("admin:{admin_id}:user:{id}:{now}"),
        Some(reason),
        now,
    ) {
        return StatusCode::CONFLICT.into_response();
    }
    let body = format!(
        "Баланс {} на {:.2} ₽. Причина: {reason}",
        if amount > 0 {
            "пополнен"
        } else {
            "уменьшен"
        },
        amount.abs() as f64 / 100.0
    );
    state.store.add_portal_notification(
        id,
        "balance",
        "Изменение баланса",
        &body,
        Some("/?view=finance"),
        now,
    );
    if id > 0 {
        let _ = state
            .bot
            .send_message(ChatId(id), format!("💰 {body}"))
            .await;
    }
    Json(serde_json::json!({"ok":true,"balance_kopecks":state.store.balance_kopecks(id)}))
        .into_response()
}

async fn admin_server_action(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<i64>,
    Json(input): Json<AdminServerAction>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(user_id) =
        session(&headers).and_then(|value| state.store.portal_user_id(value, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !state.admin_ids.contains(&user_id) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let now = now_epoch();
    if input.action == "cleanup" {
        return if input
            .days
            .is_some_and(|days| state.store.set_blocked_key_cleanup_days(days))
        {
            Json(serde_json::json!({"ok":true})).into_response()
        } else {
            (
                StatusCode::BAD_REQUEST,
                "Допустимые сроки: 7, 14, 30, 60 или 90 дней",
            )
                .into_response()
        };
    }
    let Some(server) = state.store.vpn_server(id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let result = match input.action.as_str() {
        "unavailable" => {
            let reason = input
                .reason
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty());
            if reason.is_none() {
                return (StatusCode::BAD_REQUEST, "Укажите причину отключения").into_response();
            }
            state
                .store
                .set_server_operator_unavailable(id, true, reason, user_id, now)
        }
        "available" => state
            .store
            .set_server_operator_unavailable(id, false, None, user_id, now),
        "archive" => state.store.set_server_archived(id, true, user_id, now),
        "restore" => state.store.set_server_archived(id, false, user_id, now),
        "provision_on" => state.store.set_server_provisioning(id, true, now),
        "provision_off" => state.store.set_server_provisioning(id, false, now),
        "snapshot" => {
            let Some(campaign) = state
                .store
                .ensure_server_migration_campaign(id, user_id, now)
            else {
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            };
            let items = state.store.server_migration_items(campaign.id);
            return Json(serde_json::json!({
                "ok": true,
                "server": server.name,
                "campaign_id": campaign.id,
                "total": items.len(),
                "notified": items.iter().filter(|item| item.notified_at.is_some()).count(),
                "pending": items.iter().filter(|item| item.replacement_pending).count(),
                "completed": items.iter().filter(|item| item.completed_at.is_some() || !item.active).count(),
            }))
            .into_response();
        }
        _ => return (StatusCode::BAD_REQUEST, "Неизвестное действие").into_response(),
    };
    if result {
        Json(serde_json::json!({"ok":true})).into_response()
    } else {
        (
            StatusCode::CONFLICT,
            "Состояние уже установлено или действие недоступно",
        )
            .into_response()
    }
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

async fn portal_sessions(State(state): State<PortalState>, headers: HeaderMap) -> Response {
    let Some(current) = session(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(user_id) = state.store.portal_user_id(current, now_epoch()) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    Json(serde_json::json!({"sessions":state.store.portal_sessions(user_id,current)}))
        .into_response()
}

async fn revoke_other_sessions(State(state): State<PortalState>, headers: HeaderMap) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(current) = session(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(user_id) = state.store.portal_user_id(current, now_epoch()) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let revoked = state
        .store
        .revoke_other_portal_sessions(user_id, current, now_epoch());
    Json(serde_json::json!({"ok":true,"revoked":revoked})).into_response()
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

#[derive(serde::Deserialize)]
struct TrafficQuery {
    days: Option<i64>,
}

async fn key_traffic(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(name): AxumPath<String>,
    Query(query): Query<TrafficQuery>,
) -> Response {
    let Some(user_id) =
        session(&headers).and_then(|value| state.store.portal_user_id(value, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if state.store.client_owner(&name) != Some(user_id) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let days = query.days.unwrap_or(7).clamp(1, 90);
    let points = state
        .store
        .portal_key_traffic(user_id, &name, now_epoch() - days * 86_400);
    let rx = points.iter().map(|point| point.rx).sum::<u64>();
    let tx = points.iter().map(|point| point.tx).sum::<u64>();
    let online_minutes = points.iter().map(|point| point.online_minutes).sum::<u64>();
    Json(serde_json::json!({
        "name":name,"days":days,"points":points,
        "totals":{"rx":rx,"tx":tx,"online_minutes":online_minutes}
    }))
    .into_response()
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
            state.store.add_portal_notification(
                user_id,
                "support",
                "Обращение отправлено",
                &format!("Обращение #{id} передано в поддержку."),
                Some("/?view=support"),
                now_epoch(),
            );
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

async fn support_thread(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    let Some(user_id) = session(&headers).and_then(|v| state.store.portal_user_id(v, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(ticket) = state.store.support_ticket(id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if ticket.user_id != user_id && !state.admin_ids.contains(&user_id) {
        return StatusCode::FORBIDDEN.into_response();
    }
    Json(serde_json::json!({"ticket":{"id":ticket.id,"user_id":ticket.user_id,"subject":ticket.subject,"status":ticket.status,"category":ticket.category,"priority":ticket.priority,"updated_at":ticket.updated_at},"messages":state.store.support_messages(id,500),"admin":state.admin_ids.contains(&user_id)})).into_response()
}

async fn support_reply(
    State(state): State<PortalState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<i64>,
    Json(request): Json<SupportRequest>,
) -> Response {
    if !same_site_request(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(user_id) = session(&headers).and_then(|v| state.store.portal_user_id(v, now_epoch()))
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(ticket) = state.store.support_ticket(id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let is_admin = state.admin_ids.contains(&user_id);
    if ticket.user_id != user_id && !is_admin {
        return StatusCode::FORBIDDEN.into_response();
    }
    if ticket.status == "closed" {
        return (StatusCode::CONFLICT, "Обращение уже закрыто").into_response();
    }
    let message = request.message.trim();
    if message.is_empty() || message.chars().count() > 2000 {
        return (
            StatusCode::BAD_REQUEST,
            "Введите сообщение длиной до 2000 символов",
        )
            .into_response();
    }
    let now = now_epoch();
    state
        .store
        .add_support_message(id, user_id, is_admin, (0, 0), Some(message), now);
    if is_admin {
        state.store.assign_support_ticket(id, user_id, now);
        state.store.add_portal_notification(
            ticket.user_id,
            "support",
            "Новый ответ поддержки",
            &format!("В обращении #{id} появился ответ."),
            Some("/?view=support"),
            now,
        );
        if ticket.user_id > 0 {
            let _=state.bot.send_message(ChatId(ticket.user_id),format!("💬 Новый ответ поддержки в обращении #{id}. Откройте веб-кабинет, чтобы прочитать.")).await;
        }
    } else {
        for admin in state.admin_ids.iter() {
            let _ = state
                .bot
                .send_message(
                    ChatId(*admin),
                    format!("💬 Новое сообщение в веб-обращении #{id}\nПользователь: {user_id}"),
                )
                .await;
        }
    }
    Json(serde_json::json!({"ok":true})).into_response()
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
    bot: Bot,
    options: PortalOptions,
) -> std::io::Result<()> {
    let smtp = if options.secure_cookie {
        options.smtp
    } else {
        if options.smtp.is_some() {
            tracing::warn!("вход по почте отключён: portal_public_url должен использовать HTTPS");
        }
        None
    };
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let app = Router::new()
        .route("/", get(index))
        .route("/{page}", get(marketing_page))
        .route("/robots.txt", get(robots))
        .route("/sitemap.xml", get(sitemap))
        .route("/manifest.webmanifest", get(manifest))
        .route("/service-worker.js", get(service_worker))
        .route("/assets/app.css", get(frontend_css))
        .route("/assets/app.js", get(frontend_js))
        .route("/assets/icon.svg", get(app_icon))
        .route("/login", get(login))
        .route("/api/catalog", get(catalog))
        .route("/api/session", get(portal_session))
        .route("/api/sessions", get(portal_sessions))
        .route("/api/sessions/revoke-others", post(revoke_other_sessions))
        .route("/api/public/status", get(public_status))
        .route("/api/email/bind/request", post(request_email_bind))
        .route("/api/email/bind/confirm", post(confirm_email_bind))
        .route("/api/email/login/request", post(request_email_login))
        .route("/api/email/login/confirm", post(confirm_email_login))
        .route("/api/me", get(me))
        .route("/api/admin/overview", get(admin_overview))
        .route("/api/admin/users/{id}/action", post(admin_user_action))
        .route("/api/admin/users/{id}", get(admin_user_profile))
        .route("/api/admin/users/{id}/balance", post(admin_balance_action))
        .route("/api/admin/tickets/{id}/action", post(admin_ticket_action))
        .route(
            "/api/admin/payments/{id}/action",
            post(admin_payment_action),
        )
        .route("/api/admin/promos", post(admin_create_promo))
        .route("/api/admin/promos/{code}/action", post(admin_promo_action))
        .route(
            "/api/admin/withdrawals/{id}/action",
            post(admin_withdrawal_action),
        )
        .route("/api/admin/servers/{id}/action", post(admin_server_action))
        .route("/api/logout", post(logout))
        .route("/api/keys/{name}/config", get(download_config))
        .route("/api/keys/{name}/qr", get(download_qr))
        .route("/api/keys/{name}/traffic", get(key_traffic))
        .route("/api/keys/{name}/label", patch(rename_key))
        .route("/api/keys/{name}/folder", patch(set_key_folder))
        .route("/api/keys/{name}/renew", post(create_web_renewal))
        .route("/api/partner/wallet/{action}", post(partner_wallet_action))
        .route("/api/notifications/feed", get(notifications))
        .route("/api/notifications/read", post(read_notifications))
        .route("/api/support", post(support))
        .route("/api/support/{id}", get(support_thread).post(support_reply))
        .route("/api/notifications", post(update_notifications))
        .route("/api/payments/topup", post(create_topup))
        .route("/api/purchases", post(create_purchase))
        .route("/api/promos/activate", post(activate_web_promo))
        .route("/api/keys/{name}/transfer", post(create_web_transfer))
        .route("/api/legacy/requests", post(create_web_legacy_request))
        .route("/api/transfers/{id}/action", post(web_transfer_action))
        .route("/api/payments/{id}/proof", post(submit_payment_proof))
        .route("/api/payments/webhook", post(acquiring_webhook))
        .layer(middleware::from_fn(security_headers))
        .with_state(PortalState {
            store,
            vpn,
            acquiring_webhook_secret: options.acquiring_webhook_secret,
            bot,
            admin_ids: Arc::new(options.admin_ids),
            secure_cookie: options.secure_cookie,
            smtp,
            public_url: options
                .public_url
                .unwrap_or_else(|| "https://zpnet.pro".to_string())
                .trim_end_matches('/')
                .to_string(),
        });
    tracing::info!(bind, "внутренний личный кабинет запущен");
    axum::serve(listener, app).await
}

const INDEX_HTML: &str = include_str!("../frontend/index.html");
const APP_CSS: &str = include_str!("../frontend/app.css");
const APP_JS: &str = include_str!("../frontend/app.js");
const MANIFEST_JSON: &str = include_str!("../frontend/manifest.webmanifest");
const SERVICE_WORKER: &str = include_str!("../frontend/service-worker.js");
const APP_ICON: &str = include_str!("../frontend/icon.svg");

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
    use super::{same_site_request, valid_email};
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

    #[test]
    fn email_validation_rejects_incomplete_addresses() {
        assert!(valid_email("user@example.ru"));
        assert!(!valid_email("user"));
        assert!(!valid_email("@example.ru"));
        assert!(!valid_email("user@example."));
    }
}
