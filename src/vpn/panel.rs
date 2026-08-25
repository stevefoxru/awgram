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
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub expired_at: Option<String>,
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

pub async fn list(base_url: &str, password: &str) -> Result<Vec<PanelClient>> {
    let (client, base) = session(base_url, password).await?;
    let response = client
        .get(api_url(&base, "wireguard/client")?)
        .send()
        .await
        .map_err(|error| Error::Parse(error.to_string()))?;
    if !response.status().is_success() {
        return Err(Error::Parse(format!(
            "список клиентов: HTTP {}",
            response.status()
        )));
    }
    response
        .json()
        .await
        .map_err(|error| Error::Parse(format!("ответ панели: {error}")))
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
    list(base_url, password)
        .await?
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
    let response = client
        .put(api_url(
            &base,
            &format!("wireguard/client/{client_id}/expireDate/"),
        )?)
        .json(&serde_json::json!({"expireDate": expire_date}))
        .send()
        .await
        .map_err(|error| Error::Parse(error.to_string()))?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(Error::Parse(format!(
            "срок клиента: HTTP {}",
            response.status()
        )))
    }
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
}
