use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoutingSnapshot {
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub automatic: bool,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub exits: Vec<ExitStatus>,
    #[serde(default)]
    pub clients: Vec<ClientRoute>,
    #[serde(default)]
    pub events: Vec<RoutingEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExitStatus {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub public_ip: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub healthy: bool,
    #[serde(default)]
    pub latency_ms: Option<u64>,
    #[serde(default)]
    pub last_handshake: Option<i64>,
    #[serde(default)]
    pub current: bool,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRoute {
    pub ip: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub effective_exit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingEvent {
    #[serde(default)]
    pub at: i64,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Clone)]
pub struct RoutingControl {
    command: Option<PathBuf>,
    timeout: Duration,
}

impl RoutingControl {
    pub fn new(command: Option<PathBuf>, timeout_secs: u64) -> Self {
        Self {
            command,
            timeout: Duration::from_secs(timeout_secs.clamp(5, 120)),
        }
    }

    pub fn configured(&self) -> bool {
        self.command.is_some()
    }

    async fn invoke(&self, args: &[&str]) -> Result<serde_json::Value, String> {
        let command = self
            .command
            .as_ref()
            .ok_or_else(|| "управление EXIT ещё не настроено".to_string())?;
        let mut process = tokio::process::Command::new(command);
        process.args(args);
        process.kill_on_drop(true);
        let output = tokio::time::timeout(self.timeout, process.output())
            .await
            .map_err(|_| "превышено время ожидания Routing Manager".to_string())?
            .map_err(|error| format!("не удалось запустить Routing Manager: {error}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "Routing Manager завершился с ошибкой: {}",
                stderr.trim().chars().take(800).collect::<String>()
            ));
        }
        serde_json::from_str(stdout.trim())
            .map_err(|error| format!("некорректный ответ Routing Manager: {error}"))
    }

    pub async fn status(&self) -> Result<RoutingSnapshot, String> {
        let value = self.invoke(&["status"]).await?;
        serde_json::from_value(value)
            .map_err(|error| format!("не удалось разобрать статус: {error}"))
    }

    pub async fn set_automatic(&self, enabled: bool) -> Result<RoutingSnapshot, String> {
        let value = self
            .invoke(&["automatic", if enabled { "on" } else { "off" }])
            .await?;
        serde_json::from_value(value)
            .map_err(|error| format!("не удалось разобрать статус: {error}"))
    }

    pub async fn set_client(&self, ip: &str, mode: &str) -> Result<RoutingSnapshot, String> {
        validate_client_ip(ip)?;
        if mode != "auto" && mode != "main" && mode != "nl" {
            return Err("неподдерживаемый режим клиента".into());
        }
        let value = self.invoke(&["client", "set", ip, mode]).await?;
        serde_json::from_value(value)
            .map_err(|error| format!("не удалось разобрать статус: {error}"))
    }

    pub async fn dry_run(&self) -> Result<RoutingSnapshot, String> {
        let value = self.invoke(&["dry-run"]).await?;
        serde_json::from_value(value)
            .map_err(|error| format!("не удалось разобрать статус: {error}"))
    }

    pub async fn rollback(&self) -> Result<RoutingSnapshot, String> {
        let value = self.invoke(&["rollback"]).await?;
        serde_json::from_value(value)
            .map_err(|error| format!("не удалось разобрать статус: {error}"))
    }
}

pub fn validate_client_ip(value: &str) -> Result<Ipv4Addr, String> {
    let ip = value
        .parse::<Ipv4Addr>()
        .map_err(|_| "некорректный VPN-IP".to_string())?;
    let octets = ip.octets();
    if octets[0..3] != [10, 66, 0] || octets[3] == 0 || octets[3] == 1 || octets[3] == 255 {
        return Err("IP должен принадлежать клиентской сети 10.66.0.0/24".into());
    }
    Ok(ip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_ip_validation_is_strict() {
        assert!(validate_client_ip("10.66.0.6").is_ok());
        assert!(validate_client_ip("10.66.0.1").is_err());
        assert!(validate_client_ip("10.66.1.6").is_err());
        assert!(validate_client_ip("; reboot").is_err());
    }
}
