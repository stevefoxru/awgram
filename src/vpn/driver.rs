use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Protocol {
    AmneziaWg1,
    AmneziaWg2,
    AmneziaWgPanel,
    WireGuard,
    OpenVpn,
    Outline,
}

impl Protocol {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "legacy" | "amneziawg-1" => Some(Self::AmneziaWg1),
            "modern" | "amneziawg-2" => Some(Self::AmneziaWg2),
            "amneziawg-panel" => Some(Self::AmneziaWgPanel),
            "wireguard" => Some(Self::WireGuard),
            "openvpn" => Some(Self::OpenVpn),
            "outline" => Some(Self::Outline),
            _ => None,
        }
    }

    pub const fn canonical(self) -> &'static str {
        match self {
            Self::AmneziaWg1 => "amneziawg-1",
            Self::AmneziaWg2 => "amneziawg-2",
            Self::AmneziaWgPanel => "amneziawg-panel",
            Self::WireGuard => "wireguard",
            Self::OpenVpn => "openvpn",
            Self::Outline => "outline",
        }
    }

    pub const fn capabilities(self) -> DriverCapabilities {
        match self {
            Self::AmneziaWg1 | Self::AmneziaWg2 | Self::WireGuard => {
                DriverCapabilities::full(false, true)
            }
            Self::AmneziaWgPanel => DriverCapabilities::full(true, false),
            Self::OpenVpn => DriverCapabilities::full(false, true),
            Self::Outline => DriverCapabilities {
                install: true,
                list_clients: true,
                create_client: true,
                download_config: true,
                regenerate: false,
                revoke: true,
                enable_disable: true,
                expiry: true,
                traffic: true,
                backup_restore: true,
                panel_sync: false,
                kernel_module: false,
            },
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
    }

    #[test]
    fn panel_advertises_sync_without_kernel_module() {
        let capabilities = Protocol::AmneziaWgPanel.capabilities();
        assert!(capabilities.panel_sync);
        assert!(!capabilities.kernel_module);
    }
}
