use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::error::{Error, Result};
use crate::vpn::driver::{DriverCapabilities, Protocol};

pub const API_VERSION: u16 = 1;
pub const MAX_CLOCK_SKEW_SECS: i64 = 300;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum NodeCommand {
    Health,
    Diagnose,
    Capabilities,
    Install {
        protocol: String,
    },
    ListClients,
    CreateClient {
        name: String,
    },
    GetConfiguration {
        name: String,
    },
    RegenerateClient {
        name: String,
    },
    RevokeClient {
        name: String,
    },
    SetClientEnabled {
        name: String,
        enabled: bool,
    },
    SetClientExpiry {
        name: String,
        expires_at: Option<i64>,
    },
    Backup,
    Restore {
        backup_ref: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedNodeRequest {
    pub api_version: u16,
    pub node_id: i64,
    pub timestamp: i64,
    pub nonce: String,
    pub payload: NodeCommand,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeResponse {
    pub ok: bool,
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeCapabilities {
    pub api_version: u16,
    pub agent_version: String,
    pub protocol: String,
    pub capabilities: DriverCapabilities,
}

type HmacSha256 = Hmac<Sha256>;

fn signing_bytes(
    api_version: u16,
    node_id: i64,
    timestamp: i64,
    nonce: &str,
    payload: &NodeCommand,
) -> Result<Vec<u8>> {
    serde_json::to_vec(&(api_version, node_id, timestamp, nonce, payload))
        .map_err(|error| Error::Parse(error.to_string()))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(DIGITS[(byte >> 4) as usize] as char);
        result.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    result
}

fn unhex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect()
}

impl SignedNodeRequest {
    pub fn new(
        node_id: i64,
        timestamp: i64,
        nonce: String,
        payload: NodeCommand,
        secret: &[u8],
    ) -> Result<Self> {
        if secret.len() < 32 || nonce.len() < 16 {
            return Err(Error::Parse("слабый секрет или nonce узла".into()));
        }
        let bytes = signing_bytes(API_VERSION, node_id, timestamp, &nonce, &payload)?;
        let mut mac = HmacSha256::new_from_slice(secret)
            .map_err(|_| Error::Parse("неверный секрет узла".into()))?;
        mac.update(&bytes);
        Ok(Self {
            api_version: API_VERSION,
            node_id,
            timestamp,
            nonce,
            payload,
            signature: hex(&mac.finalize().into_bytes()),
        })
    }

    pub fn verify(&self, expected_node_id: i64, now: i64, secret: &[u8]) -> Result<()> {
        if self.api_version != API_VERSION || self.node_id != expected_node_id {
            return Err(Error::Parse("несовместимый или чужой VPN-узел".into()));
        }
        if (now - self.timestamp).abs() > MAX_CLOCK_SKEW_SECS {
            return Err(Error::Parse("просроченная команда VPN-узла".into()));
        }
        if self.nonce.len() < 16 || secret.len() < 32 {
            return Err(Error::Parse("слабые учётные данные VPN-узла".into()));
        }
        let signature = unhex(&self.signature)
            .ok_or_else(|| Error::Parse("повреждена подпись VPN-узла".into()))?;
        let bytes = signing_bytes(
            self.api_version,
            self.node_id,
            self.timestamp,
            &self.nonce,
            &self.payload,
        )?;
        let mut mac = HmacSha256::new_from_slice(secret)
            .map_err(|_| Error::Parse("неверный секрет узла".into()))?;
        mac.update(&bytes);
        mac.verify_slice(&signature)
            .map_err(|_| Error::Parse("подпись команды VPN-узла не совпала".into()))
    }
}

impl NodeCapabilities {
    pub fn for_protocol(protocol: Protocol) -> Self {
        Self {
            api_version: API_VERSION,
            agent_version: env!("CARGO_PKG_VERSION").into(),
            protocol: protocol.canonical().into(),
            capabilities: protocol.capabilities(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_request_rejects_tampering_and_expiry() {
        let secret = [7_u8; 32];
        let mut request = SignedNodeRequest::new(
            42,
            1_000,
            "0123456789abcdef".into(),
            NodeCommand::Health,
            &secret,
        )
        .unwrap();
        request.verify(42, 1_001, &secret).unwrap();
        request.payload = NodeCommand::Backup;
        assert!(request.verify(42, 1_001, &secret).is_err());
        let fresh = SignedNodeRequest::new(
            42,
            1_000,
            "fedcba9876543210".into(),
            NodeCommand::Health,
            &secret,
        )
        .unwrap();
        assert!(fresh.verify(42, 2_000, &secret).is_err());
    }
}
