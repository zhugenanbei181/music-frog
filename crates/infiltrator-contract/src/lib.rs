//! Stable cross-surface contract for MusicFrog Infiltrator.
//!
//! This crate is intentionally transport-, runtime-, platform-, and
//! toolkit-neutral. It is suitable for Rust frontends, REST DTO mapping, and
//! UniFFI conversion without exposing Tokio or a concrete HTTP client.

pub mod capability;
pub mod controller;
pub mod command;
pub mod port_conflict;
pub mod resources;
pub mod doctor;
pub mod error;
pub mod intent;
pub mod lan;
pub mod ipv6;
pub mod pac;
pub mod uwp;
pub mod offline_startup;
pub mod mtu;
pub mod network_roaming;
pub mod snapshot;
pub mod session;
pub mod service_mode;
pub mod system_proxy;
pub mod surface_snapshot;
pub mod sync;
pub mod tun;
pub mod version;
pub mod surface;

#[cfg(test)]
mod tests {
    use super::capability::{Availability, Capability, CapabilitySnapshot, CapabilityStatus};
    use super::command::{CommandIntent, CommandKind, CoreLogLevel, ProxyMode};
    use super::lan::LanCredentials;
    use super::ipv6::Ipv6RoutingSnapshot;
    use super::tun::TunStack;
    use super::surface::HostKind;

    #[test]
    fn command_kind_is_stable_and_transport_free() {
        assert_eq!(
            CommandIntent::SetProxyMode {
                mode: ProxyMode::Rule,
            }
            .kind(),
            CommandKind::Proxy
        );
        assert_eq!(
            CommandIntent::RestartCore.kind(),
            CommandKind::CoreLifecycle
        );
        assert_eq!(CommandIntent::SyncNow.kind(), CommandKind::Sync);
        assert_eq!(CommandIntent::RollbackCore.kind(), CommandKind::Update);
        assert_eq!(
            CommandIntent::SetCoreLogLevel {
                level: CoreLogLevel::Debug
            }
            .kind(),
            CommandKind::Runtime
        );
        assert_eq!(
            CommandIntent::SetTunStack {
                stack: TunStack::Mixed
            }
            .kind(),
            CommandKind::Runtime
        );
        assert_eq!(CommandIntent::ProbeTunMtu.kind(), CommandKind::Network);
        assert_eq!(
            CommandIntent::SetIpv6Routing { enabled: false }.kind(),
            CommandKind::Network
        );
        assert_eq!(
            CommandIntent::ApplyPac {
                enabled: true,
                bypass_domains: Vec::new(),
                bypass_lan: true,
                minify: false,
            }
            .kind(),
            CommandKind::Network
        );
        assert_eq!(CoreLogLevel::parse("warning"), Some(CoreLogLevel::Warn));
        assert_eq!(CoreLogLevel::parse("trace"), None);
    }

    #[test]
    fn capability_snapshot_reports_missing_capabilities_explicitly() {
        let capabilities = CapabilitySnapshot::new(
            HostKind::Android,
            4,
            vec![CapabilityStatus {
                capability: Capability::Tun,
                availability: Availability::Supported,
            }],
        );
        assert!(capabilities.supports(Capability::Tun));
        assert!(matches!(
            capabilities.availability(Capability::SystemProxy),
            Availability::Unsupported { .. }
        ));
    }

    #[test]
    fn lan_credentials_never_debug_or_serialize_the_password() {
        let credentials = LanCredentials {
            username: "lan-user".to_owned(),
            password: "secret-value".to_owned(),
        };
        assert!(!format!("{credentials:?}").contains("secret-value"));
        let serialized = serde_json::to_string(&credentials).expect("serialize credential input");
        assert!(serialized.contains("lan-user"));
        assert!(!serialized.contains("secret-value"));
    }

    #[test]
    fn ipv6_routing_snapshot_keeps_core_and_tun_context() {
        let snapshot = Ipv6RoutingSnapshot::new(9, false, true);
        assert!(!snapshot.enabled);
        assert!(snapshot.tun_enabled);
        assert_eq!(snapshot.revision, 9);
    }
}
