//! Stable cross-surface contract for MusicFrog Infiltrator.
//!
//! This crate is intentionally transport-, runtime-, platform-, and
//! toolkit-neutral. It is suitable for Rust frontends, REST DTO mapping, and
//! UniFFI conversion without exposing Tokio or a concrete HTTP client.

pub mod a11y;
pub mod active_exit;
pub mod aggregator;
pub mod apply_transaction;
pub mod cadence;
pub mod capability;
pub mod command;
pub mod command_catalogue;
pub mod connection;
pub mod controller;
pub mod design_tokens;
pub mod dialer_chain;
pub mod dns;
pub mod dns_form;
pub mod doctor;
pub mod editor_viewport;
pub mod error;
pub mod intent;
pub mod ipv6;
pub mod lan;
pub mod mini_hud;
pub mod mrs_acceleration;
pub mod mtu;
pub mod network_roaming;
pub mod offline_startup;
pub mod overview_layout;
pub mod overview_matrix;
pub mod pac;
pub mod port_conflict;
pub mod privileged_network;
pub mod profile_document;
pub mod profile_options;
pub mod profile_protection;
pub mod protocol_fidelity;
pub mod protocol_matrix;
pub mod protocol_params;
pub mod protocol_params_ext;
mod protocol_psk;
pub mod protocol_trust;
pub mod provider_cache;
pub mod proxies;

#[cfg(test)]
#[path = "protocol_fidelity_test.rs"]
mod protocol_fidelity_test;

#[cfg(test)]
#[path = "protocol_params_test.rs"]
mod protocol_params_test;
pub mod proxy_mode;
pub mod public_ip;
pub mod reconnect_mask;
pub mod resources;
pub mod responsive_viewport;
pub mod rule_edit;
pub mod rule_tracer;
pub mod script_sandbox;
pub mod script_sandbox_matrix;
pub mod service_mode;
pub mod session;
pub mod shortcuts;
pub mod snapshot;
pub mod snapshot_history;
pub mod speedtest;
pub mod speedtest_matrix;
pub mod subscription_import;
pub mod subscription_quota;
pub mod surface;
pub mod surface_snapshot;
pub mod sync;
pub mod system_proxy;
pub mod system_toggle;
pub mod theme;
pub mod toast;
pub mod traffic_scale;
pub mod traffic_topology;
pub mod traffic_waveform;
pub mod tray_status;
pub mod tun;
pub mod uwp;
pub mod version;
pub mod vpn;
pub mod window_chrome;
pub mod yaml_ast_diff;
pub mod yaml_snippets;

#[cfg(test)]
mod tests {
    use super::capability::{Availability, Capability, CapabilitySnapshot, CapabilityStatus};
    use super::command::{CommandIntent, CommandKind, CoreLogLevel, ProxyMode};
    use super::ipv6::Ipv6RoutingSnapshot;
    use super::lan::LanCredentials;
    use super::surface::HostKind;
    use super::tun::TunStack;

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
