//! Stable cross-surface contract for MusicFrog Infiltrator.
//!
//! This crate is intentionally transport-, runtime-, platform-, and
//! toolkit-neutral. It is suitable for Rust frontends, REST DTO mapping, and
//! UniFFI conversion without exposing Tokio or a concrete HTTP client.

pub mod a11y;
pub mod active_exit;
pub mod adapter_operation_declaration;
pub mod aggregator;
pub mod android_control;
pub mod app_routing;
pub mod apply_transaction;
pub mod cadence;
pub mod capability;
pub mod command;
pub mod command_catalogue;
pub mod command_output;
pub mod connection;
pub mod connection_search;
pub mod controller;
pub mod core_capabilities;
pub mod core_control;
pub mod core_identity;
pub mod design_tokens;
pub mod dialer_chain;
pub mod dns;
pub mod dns_cache;
pub mod dns_form;
pub mod dns_hosts;
mod dns_hosts_alias;
pub mod dns_latency;
pub mod dns_leak;
pub mod dns_query;
pub mod dns_self_heal;
pub mod doctor;
pub mod editor_viewport;
pub mod error;
pub mod fixture_identities;
pub mod ime;
pub mod intent;
pub mod ipv6;
pub mod lan;
pub mod language;
pub mod latency_display;
pub mod mini_hud;
pub mod mrs_acceleration;
pub mod mtu;
pub mod network_roaming;
pub mod offline_startup;
pub mod outbound;
pub mod overview_layout;
pub mod pac;
pub mod parity;
pub mod port_conflict;
pub mod privileged_network;
pub mod profile_configuration;
pub mod profile_document;
pub mod profile_document_identity;
pub mod profile_options;
pub mod profile_protection;
pub mod profile_source;
pub mod protocol_fidelity;
pub mod protocol_matrix;
pub mod protocol_params;
pub mod protocol_params_ext;
mod protocol_psk;
pub mod protocol_trust;
pub mod provider_cache;
pub mod proxies;
pub mod runtime_settings;

pub mod speedtest_details;

#[cfg(test)]
#[path = "protocol_fidelity_test.rs"]
mod protocol_fidelity_test;

#[cfg(test)]
#[path = "adapter_operation_declaration_tests.rs"]
mod adapter_operation_declaration_tests;
#[cfg(test)]
#[path = "protocol_params_test.rs"]
mod protocol_params_test;
pub mod public_ip;
pub mod reconnect_mask;
pub mod resources;
pub mod responsive_viewport;
pub mod routing_intent;
pub mod rule_condition;
pub mod rule_document;
pub mod rule_edit;
pub mod rule_location;
pub mod rule_provider_snapshot;
pub mod rule_snapshot;
pub mod rule_source;
pub mod rule_trace_facts;
pub mod rule_trace_run;
pub mod rule_tracer;
pub mod rules_workspace;
pub mod runtime_control;
pub mod script_export;
pub mod script_export_review;
pub mod script_run;
pub mod script_sandbox;
pub mod script_sandbox_matrix;
pub mod service_mode;
pub mod session;
pub mod shell_gesture;
pub mod shell_readout;
pub mod shortcuts;
pub mod snapshot;
pub mod snapshot_history;
pub mod snapshot_restore;
pub mod speedtest;
pub mod stun_probe;
pub mod subscription_filter_form;
pub mod subscription_filter_result;
pub mod subscription_import;
pub mod subscription_quota;
pub mod surface;
pub mod surface_snapshot;
pub mod sync;
pub mod sync_snapshot;
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
    use super::command::{CommandIntent, CommandKind, CoreLogLevel};
    use super::ipv6::Ipv6RoutingSnapshot;
    use super::lan::LanCredentials;
    use super::routing_intent::RoutingIntent;
    use super::surface::HostKind;
    use super::tun::TunStack;

    #[test]
    fn command_kind_is_stable_and_transport_free() {
        assert_eq!(
            CommandIntent::SetRoutingIntent {
                intent: RoutingIntent::FollowRules,
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
        assert_eq!(CoreLogLevel::from_key("warn"), Some(CoreLogLevel::Warn));
        assert_eq!(CoreLogLevel::from_key("warning"), None);
        assert_eq!(CoreLogLevel::from_key("trace"), None);
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

pub mod protocol_form;

pub mod log_export;
pub mod logs;

pub mod rule_statistics;

pub mod rule_hit_audit;

pub mod rule_json_feedback;

pub mod profile_editor_read;
