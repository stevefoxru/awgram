use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Protocol {
    AmneziaWg1,
    AmneziaWg2,
    AmneziaWg3,
    AmneziaWgPanel,
}

impl Protocol {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "legacy" | "amneziawg-1" => Some(Self::AmneziaWg1),
            "modern" | "amneziawg-2" => Some(Self::AmneziaWg2),
            "amneziawg-3" | "amneziawg-3.1" => Some(Self::AmneziaWg3),
            "amneziawg-panel" => Some(Self::AmneziaWgPanel),
            _ => None,
        }
    }

    pub const fn canonical(self) -> &'static str {
        match self {
            Self::AmneziaWg1 => "amneziawg-1",
            Self::AmneziaWg2 => "amneziawg-2",
            Self::AmneziaWg3 => "amneziawg-3",
            Self::AmneziaWgPanel => "amneziawg-panel",
        }
    }

    pub const fn capabilities(self) -> DriverCapabilities {
        match self {
            Self::AmneziaWg1 | Self::AmneziaWg2 => DriverCapabilities::full(false, true),
            // AWG 3.1 installed by AmneziaVPN needs its own container adapter.
            // Until its preflight succeeds it is deliberately inventory-only.
            Self::AmneziaWg3 => DriverCapabilities::inventory_only(),
            Self::AmneziaWgPanel => DriverCapabilities::full(true, false),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverCapabilities {
    pub install: bool,
    pub list_clients: bool,
    pub create_client: bool,
    pub download_config: bool,
    pub regenerate: bool,
    pub revoke: bool,
    pub enable_disable: bool,
    pub expiry: bool,
    pub traffic: bool,
    pub backup_restore: bool,
    pub panel_sync: bool,
    pub kernel_module: bool,
}

impl DriverCapabilities {
    const fn inventory_only() -> Self {
        Self {
            install: false,
            list_clients: false,
            create_client: false,
            download_config: false,
            regenerate: false,
            revoke: false,
            enable_disable: false,
            expiry: false,
            traffic: false,
            backup_restore: false,
            panel_sync: false,
            kernel_module: false,
        }
    }
    const fn full(panel_sync: bool, kernel_module: bool) -> Self {
        Self {
            install: true,
            list_clients: true,
            create_client: true,
            download_config: true,
            regenerate: true,
            revoke: true,
            enable_disable: true,
            expiry: true,
            traffic: true,
            backup_restore: true,
            panel_sync,
            kernel_module,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeTransport {
    Local,
    RestrictedSsh,
    SignedSsh,
    HttpsAgent,
    PanelApi,
}

impl NodeTransport {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::RestrictedSsh => "restricted_ssh",
            Self::SignedSsh => "signed_ssh",
            Self::HttpsAgent => "https_agent",
            Self::PanelApi => "panel_api",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_aliases_have_one_canonical_driver() {
        assert_eq!(Protocol::parse("legacy"), Some(Protocol::AmneziaWg1));
        assert_eq!(
            Protocol::parse("amneziawg-1").unwrap().canonical(),
            "amneziawg-1"
        );
        assert!(Protocol::parse("pptp").is_none());
        assert!(Protocol::parse("wireguard").is_none());
        assert!(Protocol::parse("openvpn").is_none());
        assert!(Protocol::parse("outline").is_none());
    }

    #[test]
    fn panel_advertises_sync_without_kernel_module() {
        let capabilities = Protocol::AmneziaWgPanel.capabilities();
        assert!(capabilities.panel_sync);
        assert!(!capabilities.kernel_module);
    }

    #[test]
    fn awg31_has_a_distinct_safe_driver() {
        let protocol = Protocol::parse("amneziawg-3.1").unwrap();
        assert_eq!(protocol.canonical(), "amneziawg-3");
        assert!(!protocol.capabilities().create_client);
        assert!(!protocol.capabilities().list_clients);
    }
}
