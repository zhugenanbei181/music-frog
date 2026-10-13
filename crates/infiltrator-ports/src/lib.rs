//! Outbound ports used by the 0.30 application layer.
//!
//! Traits here describe capabilities, not implementations. They deliberately
//! contain no Tokio channels, task handles, HTTP response types, or UI types.

pub mod active_profile_reference_store;
pub mod android_core_process_recovery;
pub mod app_routing_store;
pub mod application_runtime;
pub mod capability_provider;
pub mod certificate_authority;
pub mod connection;
pub mod controller_fake_ip_cache;
pub mod core_binary_selection;
pub mod core_capability_provider;
pub mod core_lifecycle;
pub mod core_log_level;
pub mod core_process;
pub mod core_watchdog;
pub mod data_dir;
pub mod data_store;
pub mod dns_latency;
pub mod dns_leak;
pub mod dns_query;
pub mod doctor;
pub mod endpoint;
pub mod engine_backend_factory;
pub mod engine_version_backend_factory;
pub mod error;
pub mod fake_ip_cache;
pub mod host_runtime;
pub mod log_export;
pub mod mini_hud_window;
pub mod mtu_probe;
pub mod network_roaming;
pub mod offline_startup;
pub mod outbound;
pub mod overview;
pub mod pac;
pub mod port_conflict;
pub mod privileged_network;
pub mod profile_configuration_transaction;
pub mod profile_document_codec;
pub mod profile_reset;
pub mod profile_store;
pub mod profile_workspace;
pub mod public_ip_probe;
pub mod resources;
pub mod routing_intent;
pub mod rule_provider_cache;
pub mod rule_tracer;
pub mod runtime_configuration;
pub mod runtime_gateway;
pub mod runtime_http_proxy_endpoint;
pub mod runtime_stream;
pub mod script_engine;
pub mod script_export;
pub mod secure_store;
pub mod service_mode;
pub mod settings_store;
pub mod snapshot_store;
pub mod speedtest_history;
pub mod stun_probe;
pub mod subscription_import;
pub mod subscription_notification;
pub mod subscription_source;
pub mod surface;
pub mod sync;
pub mod system_dns_cache;
pub mod system_proxy;
pub mod touch_gesture;
pub mod uwp_loopback;
pub mod version;
pub mod vpn_service;

#[cfg(test)]
mod tests {
    use super::error::PortError;
    use infiltrator_contract::capability::Capability;
    use infiltrator_contract::error::{ErrorCode, Failure};

    #[test]
    fn adapter_error_maps_to_contract_failure_without_runtime_types() {
        let failure: Failure = PortError::unsupported(
            Capability::SystemProxy,
            "Android VpnService owns proxy routing",
        )
        .into();
        assert_eq!(failure.code, ErrorCode::Unsupported);
        assert!(!failure.retryable);
        assert!(failure.message.contains("SystemProxy"));
    }
}
