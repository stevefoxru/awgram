use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Read;
use std::path::{Path, PathBuf};

use awgram::vpn::driver::Protocol;
use awgram::vpn::node_api::{NodeCapabilities, NodeCommand, NodeResponse, SignedNodeRequest};

fn response(ok: bool, code: &str, message: impl Into<String>) -> NodeResponse {
    NodeResponse {
        ok,
        code: code.into(),
        message: message.into(),
        data: None,
    }
}

fn emit(value: &NodeResponse) {
    println!(
        "{}",
        serde_json::to_string(value).unwrap_or_else(|_| {
            r#"{"ok":false,"code":"serialization","message":"internal error"}"#.into()
        })
    );
}

fn read_config(path: &Path) -> std::io::Result<HashMap<String, String>> {
    let contents = std::fs::read_to_string(path)?;
    Ok(contents
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().into(), value.trim().trim_matches('"').into()))
        .collect())
}

fn claim_nonce(nonce: &str, now: i64) -> std::io::Result<bool> {
    if nonce.len() < 16
        || nonce.len() > 128
        || !nonce
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Ok(false);
    }
    let directory = Path::new("/var/lib/awgram-node/nonces");
    std::fs::create_dir_all(directory)?;
    for entry in std::fs::read_dir(directory)?.flatten() {
        let stale = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age.as_secs() > 600);
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    let path = directory.join(nonce);
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            use std::io::Write;
            writeln!(file, "{now}")?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error),
    }
}

fn valid_name(name: &str) -> bool {
    name.len() <= 64
        && name
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}

fn driver_args(command: &NodeCommand) -> Option<Vec<String>> {
    let args = match command {
        NodeCommand::Health => vec!["status".into()],
        NodeCommand::Diagnose => vec!["diagnose".into()],
        NodeCommand::Install { protocol } => {
            let protocol = Protocol::parse(protocol)?.canonical();
            vec!["install".into(), protocol.into()]
        }
        NodeCommand::ListClients => vec!["list".into()],
        NodeCommand::CreateClient { name } if valid_name(name) => vec!["add".into(), name.clone()],
        NodeCommand::GetConfiguration { name } if valid_name(name) => {
            vec!["get".into(), name.clone()]
        }
        NodeCommand::RegenerateClient { name } if valid_name(name) => {
            vec!["regen".into(), name.clone()]
        }
        NodeCommand::RevokeClient { name } if valid_name(name) => {
            vec!["remove".into(), name.clone()]
        }
        NodeCommand::SetClientEnabled { name, enabled } if valid_name(name) => vec![
            if *enabled { "enable" } else { "disable" }.into(),
            name.clone(),
        ],
        NodeCommand::SetClientExpiry { name, expires_at } if valid_name(name) => vec![
            "set-expiry".into(),
            name.clone(),
            expires_at.map_or_else(|| "none".into(), |value| value.to_string()),
        ],
        NodeCommand::Backup => vec!["backup".into()],
        NodeCommand::Restore { backup_ref }
            if !backup_ref.is_empty()
                && backup_ref.len() <= 128
                && backup_ref.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
                }) =>
        {
            vec!["restore".into(), backup_ref.clone()]
        }
        NodeCommand::Capabilities => return None,
        _ => return Some(Vec::new()),
    };
    Some(args)
}

fn run() -> NodeResponse {
    let config_path = std::env::var_os("AWGRAM_NODE_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/etc/awgram-node/node.conf"));
    let config = match read_config(&config_path) {
        Ok(value) => value,
        Err(error) => return response(false, "config", error.to_string()),
    };
    let node_id = match config.get("NODE_ID").and_then(|value| value.parse().ok()) {
        Some(value) => value,
        None => return response(false, "config", "NODE_ID is missing"),
    };
    let protocol = match config
        .get("PROTOCOL")
        .and_then(|value| Protocol::parse(value))
    {
        Some(value) => value,
        None => return response(false, "config", "PROTOCOL is invalid"),
    };
    let secret_path = config
        .get("SECRET_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/etc/awgram-node/agent.secret"));
    let secret = match std::fs::read(secret_path) {
        Ok(value) if value.len() >= 32 => value,
        _ => return response(false, "config", "node secret is missing"),
    };
    let mut input = String::new();
    if let Err(error) = std::io::stdin().read_to_string(&mut input) {
        return response(false, "request", error.to_string());
    }
    let request: SignedNodeRequest = match serde_json::from_str(&input) {
        Ok(value) => value,
        Err(error) => return response(false, "request", error.to_string()),
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default();
    if let Err(error) = request.verify(node_id, now, &secret) {
        return response(false, "unauthorized", error.to_string());
    }
    match claim_nonce(&request.nonce, now) {
        Ok(true) => {}
        Ok(false) => return response(false, "replay", "nonce already used or invalid"),
        Err(error) => return response(false, "nonce_store", error.to_string()),
    }
    if matches!(request.payload, NodeCommand::Capabilities) {
        return NodeResponse {
            ok: true,
            code: "ok".into(),
            message: "capabilities".into(),
            data: serde_json::to_value(NodeCapabilities::for_protocol(protocol)).ok(),
        };
    }
    let Some(args) = driver_args(&request.payload) else {
        return response(false, "unsupported", "command is not supported");
    };
    if args.is_empty() {
        return response(false, "invalid_arguments", "command arguments are invalid");
    }
    let driver = config
        .get("DRIVER_COMMAND")
        .map(String::as_str)
        .unwrap_or("/usr/local/libexec/awgram-driver");
    if !Path::new(driver).is_file() {
        return response(false, "driver_missing", "VPN driver is not installed");
    }
    let output = match std::process::Command::new(driver).args(args).output() {
        Ok(value) => value,
        Err(error) => return response(false, "driver_start", error.to_string()),
    };
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() {
        return response(
            false,
            "driver_failed",
            String::from_utf8_lossy(&output.stderr).trim(),
        );
    }
    NodeResponse {
        ok: true,
        code: "ok".into(),
        message: "command completed".into(),
        data: serde_json::from_str(&stdout)
            .ok()
            .or_else(|| Some(serde_json::Value::String(stdout))),
    }
}

fn main() {
    let result = run();
    emit(&result);
    if !result.ok {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_shell_metacharacters_in_names_and_backups() {
        assert!(driver_args(&NodeCommand::CreateClient {
            name: "alice_1".into()
        })
        .is_some_and(|args| !args.is_empty()));
        assert_eq!(
            driver_args(&NodeCommand::CreateClient {
                name: "alice;reboot".into()
            }),
            Some(Vec::new())
        );
        assert_eq!(
            driver_args(&NodeCommand::Restore {
                backup_ref: "../../etc/shadow".into()
            }),
            Some(Vec::new())
        );
    }
}
