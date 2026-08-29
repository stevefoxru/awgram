use std::path::Path;

use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::Rng;
use serde::Deserialize;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelClient {
    #[serde(deserialize_with = "deserialize_stringish")]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub expired_at: Option<String>,
    #[serde(default, alias = "transferRx", alias = "receivedBytes", alias = "rx")]
    pub transfer_rx: u64,
    #[serde(default, alias = "transferTx", alias = "sentBytes", alias = "tx")]
    pub transfer_tx: u64,
    #[serde(
        default,
        alias = "latestHandshakeAt",
        alias = "lastHandshakeAt",
        alias = "lastHandshake"
    )]
    pub latest_handshake_at: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelProbe {
    pub client_count: Option<usize>,
    pub response_format: String,
}

fn deserialize_stringish<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(value) => Ok(value),
        serde_json::Value::Number(value) => Ok(value.to_string()),
        _ => Err(serde::de::Error::custom(
            "ожидался строковый или числовой id",
        )),
    }
}

impl PanelClient {
    pub fn last_handshake_epoch(&self) -> Option<i64> {
        let value = self.latest_handshake_at.as_ref()?;
        if let Some(epoch) = value.as_i64() {
            return Some(if epoch > 10_000_000_000 {
                epoch / 1000
            } else {
                epoch
            });
        }
        let raw = value.as_str()?;
        raw.parse::<i64>().ok().map(|epoch| {
            if epoch > 10_000_000_000 {
                epoch / 1000
            } else {
                epoch
            }
        })
    }
}

fn parse_base_url(value: &str) -> Result<reqwest::Url> {
    let mut url = reqwest::Url::parse(value.trim())
        .map_err(|error| Error::Parse(format!("неверный URL панели: {error}")))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(Error::Parse(
            "URL панели должен начинаться с http:// или https://".into(),
        ));
    }
    url.set_query(None);
    url.set_fragment(None);
    if !url.path().ends_with('/') {
        url.set_path(&format!("{}/", url.path()));
    }
    Ok(url)
}

fn api_url(base: &reqwest::Url, path: &str) -> Result<reqwest::Url> {
    base.join(&format!("api/{}", path.trim_start_matches('/')))
        .map_err(|error| Error::Parse(error.to_string()))
}

async fn session(base_url: &str, password: &str) -> Result<(reqwest::Client, reqwest::Url)> {
    let base = parse_base_url(base_url)?;
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|error| Error::Parse(error.to_string()))?;
    let response = client
        .post(api_url(&base, "session")?)
        .json(&serde_json::json!({"password": password, "remember": false}))
        .send()
        .await
        .map_err(|error| Error::Parse(format!("панель недоступна: {error}")))?;
    if !response.status().is_success() {
        return Err(Error::Parse(format!(
            "панель отклонила пароль (HTTP {})",
            response.status()
        )));
    }
    Ok((client, base))
}

fn client_array(value: serde_json::Value) -> Option<serde_json::Value> {
    match value {
        value @ serde_json::Value::Array(_) => Some(value),
        serde_json::Value::Object(mut object) => {
            for key in ["clients", "data", "result", "items", "rows"] {
                if let Some(value) = object.remove(key) {
                    if let Some(value) = client_array(value) {
                        return Some(value);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn decode_client_list(body: &[u8]) -> std::result::Result<Vec<PanelClient>, String> {
    let body = body.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(body);
    let value: serde_json::Value = serde_json::from_slice(body).map_err(|error| {
        let preview = String::from_utf8_lossy(body)
            .chars()
            .take(160)
            .collect::<String>();
        format!("не JSON: {error}; начало ответа: {preview:?}")
    })?;
    let array = client_array(value)
        .ok_or_else(|| "JSON не содержит массива clients/data/result/items/rows".to_string())?;
    serde_json::from_value(array).map_err(|error| format!("неизвестный формат клиента: {error}"))
}

async fn fetch_clients(client: &reqwest::Client, base: &reqwest::Url) -> Result<(String, Vec<u8>)> {
    let response = client
        .get(api_url(base, "wireguard/client")?)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::ACCEPT_ENCODING, "identity")
        .send()
        .await
        .map_err(|error| Error::Parse(format!("запрос списка клиентов: {error}")))?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("не указан")
        .to_string();
    if !status.is_success() {
        return Err(Error::Parse(format!("список клиентов: HTTP {status}")));
    }
    let body = response
        .bytes()
        .await
        .map_err(|error| Error::Parse(format!("чтение ответа панели: {error}")))?;
    Ok((content_type, body.to_vec()))
}

pub async fn list(base_url: &str, password: &str) -> Result<Vec<PanelClient>> {
    let (client, base) = session(base_url, password).await?;
    let (content_type, body) = fetch_clients(&client, &base).await?;
    decode_client_list(&body).map_err(|error| {
        Error::Parse(format!(
            "ответ панели не распознан (Content-Type {content_type}): {error}"
        ))
    })
}

pub async fn probe(base_url: &str, password: &str) -> Result<PanelProbe> {
    let (client, base) = session(base_url, password).await?;
    let (content_type, body) = fetch_clients(&client, &base).await?;
    Ok(match decode_client_list(&body) {
        Ok(clients) => PanelProbe {
            client_count: Some(clients.len()),
            response_format: format!("совместимый JSON · Content-Type {content_type}"),
        },
        Err(error) => PanelProbe {
            client_count: None,
            response_format: format!(
                "панель и авторизация работают, но формат списка клиентов пока не распознан · Content-Type {content_type} · {error}"
            ),
        },
    })
}

pub async fn create(base_url: &str, password: &str, name: &str) -> Result<PanelClient> {
    let (client, base) = session(base_url, password).await?;
    let response = client
        .post(api_url(&base, "wireguard/client")?)
        .json(&serde_json::json!({"name": name, "expiredDate": null}))
        .send()
        .await
        .map_err(|error| Error::Parse(error.to_string()))?;
    if !response.status().is_success() {
        return Err(Error::Parse(format!(
            "создание клиента: HTTP {}",
            response.status()
        )));
    }
    let (content_type, body) = fetch_clients(&client, &base).await?;
    decode_client_list(&body)
        .map_err(|error| {
            Error::Parse(format!(
                "ответ панели не распознан (Content-Type {content_type}): {error}"
            ))
        })?
        .into_iter()
        .find(|client| client.name == name)
        .ok_or_else(|| Error::Parse("панель создала ключ, но не вернула его в списке".into()))
}

pub async fn configuration(base_url: &str, password: &str, client_id: &str) -> Result<Vec<u8>> {
    let (client, base) = session(base_url, password).await?;
    let response = client
        .get(api_url(
            &base,
            &format!("wireguard/client/{client_id}/configuration"),
        )?)
        .send()
        .await
        .map_err(|error| Error::Parse(error.to_string()))?;
    if !response.status().is_success() {
        return Err(Error::Parse(format!(
            "конфигурация: HTTP {}",
            response.status()
        )));
    }
    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|error| Error::Parse(error.to_string()))
}

pub async fn delete(base_url: &str, password: &str, client_id: &str) -> Result<()> {
    let (client, base) = session(base_url, password).await?;
    let response = client
        .delete(api_url(&base, &format!("wireguard/client/{client_id}"))?)
        .send()
        .await
        .map_err(|error| Error::Parse(error.to_string()))?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(Error::Parse(format!(
            "удаление клиента: HTTP {}",
            response.status()
        )))
    }
}

pub async fn set_expiry(
    base_url: &str,
    password: &str,
    client_id: &str,
    expire_date: &str,
) -> Result<()> {
    let (client, base) = session(base_url, password).await?;
    let paths = [
        format!("wireguard/client/{client_id}/expireDate"),
        format!("wireguard/client/{client_id}/expireDate/"),
    ];
    let bodies = [
        serde_json::json!({"expireDate": expire_date}),
        serde_json::json!({"expiredDate": expire_date}),
    ];
    let mut last = String::new();
    for path in paths {
        for body in &bodies {
            let response = client
                .put(api_url(&base, &path)?)
                .json(body)
                .send()
                .await
                .map_err(|error| Error::Parse(error.to_string()))?;
            if response.status().is_success() {
                return Ok(());
            }
            let status = response.status();
            let details = response.text().await.unwrap_or_default();
            last = format!(
                "HTTP {status} — {}",
                details.chars().take(300).collect::<String>()
            );
        }
    }
    Err(Error::Parse(format!(
        "панель не приняла срок действия клиента ({last})"
    )))
}

pub fn iso_date(epoch: i64) -> String {
    let z = epoch.div_euclid(86_400) + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

fn load_or_create_key(path: &Path) -> Result<[u8; 32]> {
    if let Ok(bytes) = std::fs::read(path) {
        return bytes
            .try_into()
            .map_err(|_| Error::Parse("повреждён ключ шифрования панели".into()));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut key = [0u8; 32];
    rand::rng().fill_bytes(&mut key);
    std::fs::write(path, key)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(key)
}

pub fn protect_password(key_path: &Path, password: &str) -> Result<String> {
    if password.is_empty() {
        return Err(Error::Parse("пароль панели пуст".into()));
    }
    let key = load_or_create_key(key_path)?;
    let cipher = XChaCha20Poly1305::new((&key).into());
    let mut nonce = [0u8; 24];
    rand::rng().fill_bytes(&mut nonce);
    let encrypted = cipher
        .encrypt(XNonce::from_slice(&nonce), password.as_bytes())
        .map_err(|_| Error::Parse("не удалось зашифровать пароль панели".into()))?;
    let mut payload = nonce.to_vec();
    payload.extend(encrypted);
    Ok(base64::engine::general_purpose::STANDARD_NO_PAD.encode(payload))
}

pub fn reveal_password(key_path: &Path, encrypted: &str) -> Result<String> {
    let payload = base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(encrypted)
        .map_err(|_| Error::Parse("повреждены учётные данные панели".into()))?;
    if payload.len() < 25 {
        return Err(Error::Parse("повреждены учётные данные панели".into()));
    }
    let key = load_or_create_key(key_path)?;
    let cipher = XChaCha20Poly1305::new((&key).into());
    let clear = cipher
        .decrypt(XNonce::from_slice(&payload[..24]), &payload[24..])
        .map_err(|_| Error::Parse("не удалось расшифровать пароль панели".into()))?;
    String::from_utf8(clear).map_err(|_| Error::Parse("пароль панели не UTF-8".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_list_accepts_direct_and_wrapped_arrays() {
        let direct = decode_client_list(br#"[{"id":"a","name":"alice","enabled":true}]"#).unwrap();
        assert_eq!(direct[0].id, "a");
        let wrapped =
            decode_client_list(br#"{"success":true,"data":{"clients":[{"id":42,"name":"bob"}]}}"#)
                .unwrap();
        assert_eq!(wrapped[0].id, "42");
        assert_eq!(wrapped[0].name, "bob");
    }

    #[test]
    fn client_list_accepts_bom_and_reports_html_preview() {
        let with_bom = b"\xef\xbb\xbf[{\"id\":1,\"name\":\"phone\"}]";
        assert_eq!(decode_client_list(with_bom).unwrap()[0].name, "phone");
        let error = decode_client_list(b"<html>login</html>").unwrap_err();
        assert!(error.contains("не JSON"));
        assert!(error.contains("login"));
    }

    #[test]
    fn password_roundtrip_uses_separate_key_file() {
        let dir = tempfile::tempdir().unwrap();
        let key = dir.path().join("panel.key");
        let protected = protect_password(&key, "secret-password").unwrap();
        assert!(!protected.contains("secret"));
        assert_eq!(
            reveal_password(&key, &protected).unwrap(),
            "secret-password"
        );
        assert_eq!(std::fs::read(key).unwrap().len(), 32);
    }

    #[test]
    fn panel_expiry_uses_iso_date() {
        assert_eq!(iso_date(0), "1970-01-01");
        assert_eq!(iso_date(1_766_016_000), "2025-12-18");
    }
}
