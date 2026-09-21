//! Decoder for links exported by the official AmneziaVPN client.
//!
//! Qt's `qCompress` prefixes the zlib stream with a four-byte big-endian
//! uncompressed length. Older/uncompressed exports are accepted as well.

use std::io::Read;

use base64::Engine;
use flate2::read::ZlibDecoder;
use serde_json::Value;

use crate::error::{Error, Result};

const MAX_URI_LEN: usize = 2 * 1024 * 1024;
const MAX_JSON_LEN: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedServerAccess {
    pub host: String,
    pub port: u16,
    pub user: Option<String>,
    pub password: Option<String>,
    pub description: Option<String>,
    pub has_awg31: bool,
    pub containers: Vec<String>,
}

impl SharedServerAccess {
    pub fn is_full_access(&self) -> bool {
        self.user.as_deref().is_some_and(|value| !value.is_empty())
            && self
                .password
                .as_deref()
                .is_some_and(|value| !value.is_empty())
    }
}

pub fn decode(uri: &str) -> Result<SharedServerAccess> {
    let uri = uri.trim();
    if uri.len() > MAX_URI_LEN || !uri.starts_with("vpn://") {
        return Err(Error::Parse("ожидалась ссылка vpn:// из AmneziaVPN".into()));
    }
    let encoded = uri.strip_prefix("vpn://").unwrap_or_default().trim();
    if encoded.is_empty() {
        return Err(Error::Parse("пустая ссылка AmneziaVPN".into()));
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(encoded))
        .map_err(|_| Error::Parse("ссылка vpn:// повреждена".into()))?;
    let json = decode_json_bytes(&bytes)?;
    let root: Value = serde_json::from_slice(&json)
        .map_err(|_| Error::Parse("в ссылке vpn:// нет конфигурации AmneziaVPN".into()))?;
    let object = root.as_object().ok_or_else(|| {
        Error::Parse("корень конфигурации AmneziaVPN не является объектом".into())
    })?;

    let host = string_field(object, &["hostName", "host"])
        .ok_or_else(|| Error::Parse("в ключе отсутствует адрес сервера".into()))?;
    if !valid_host(&host) {
        return Err(Error::Parse(
            "в ключе указан небезопасный адрес сервера".into(),
        ));
    }
    let port = integer_field(object, &["port", "sshPort"])
        .and_then(|value| u16::try_from(value).ok())
        .filter(|value| *value > 0)
        .unwrap_or(22);
    let containers = object
        .get("containers")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("container").and_then(Value::as_str))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let has_awg31 = containers.iter().any(|name| {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "amnezia-awg2" | "awg2" | "amneziawg2"
        )
    });

    Ok(SharedServerAccess {
        host,
        port,
        user: string_field(object, &["userName", "username", "user"]),
        password: string_field(object, &["password"]),
        description: string_field(object, &["description", "displayName"]),
        has_awg31,
        containers,
    })
}

fn decode_json_bytes(bytes: &[u8]) -> Result<Vec<u8>> {
    if serde_json::from_slice::<Value>(bytes).is_ok() {
        return Ok(bytes.to_vec());
    }
    let candidates = [bytes.get(4..).unwrap_or_default(), bytes];
    for candidate in candidates {
        let mut decoder = ZlibDecoder::new(candidate);
        let mut output = Vec::new();
        if decoder
            .by_ref()
            .take((MAX_JSON_LEN + 1) as u64)
            .read_to_end(&mut output)
            .is_ok()
            && output.len() <= MAX_JSON_LEN
            && serde_json::from_slice::<Value>(&output).is_ok()
        {
            return Ok(output);
        }
    }
    Err(Error::Parse(
        "не удалось распаковать конфигурацию AmneziaVPN".into(),
    ))
}

fn string_field(object: &serde_json::Map<String, Value>, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| object.get(*name).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn integer_field(object: &serde_json::Map<String, Value>, names: &[&str]) -> Option<u64> {
    names.iter().find_map(|name| {
        object.get(*name).and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        })
    })
}

fn valid_host(host: &str) -> bool {
    host.len() <= 253
        && !host.is_empty()
        && host
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | ':' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::ZlibEncoder, Compression};
    use std::io::Write;

    fn uri(json: &[u8], compressed: bool) -> String {
        let bytes = if compressed {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
            encoder.write_all(json).unwrap();
            let payload = encoder.finish().unwrap();
            let mut qt = (json.len() as u32).to_be_bytes().to_vec();
            qt.extend(payload);
            qt
        } else {
            json.to_vec()
        };
        format!(
            "vpn://{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
        )
    }

    #[test]
    fn decodes_qt_compressed_full_access_awg31() {
        let input = br#"{"hostName":"192.0.2.4","port":2222,"userName":"root","password":"secret","containers":[{"container":"amnezia-awg2"}]}"#;
        let access = decode(&uri(input, true)).unwrap();
        assert_eq!(access.host, "192.0.2.4");
        assert_eq!(access.port, 2222);
        assert!(access.is_full_access());
        assert!(access.has_awg31);
    }

    #[test]
    fn client_key_is_not_full_access() {
        let input =
            br#"{"hostName":"vpn.example.com","containers":[{"container":"amnezia-awg2"}]}"#;
        let access = decode(&uri(input, false)).unwrap();
        assert!(!access.is_full_access());
    }

    #[test]
    fn rejects_non_vpn_and_unsafe_host() {
        assert!(decode("https://example.com").is_err());
        let input = br#"{"hostName":"host; reboot","containers":[]}"#;
        assert!(decode(&uri(input, false)).is_err());
    }
}
