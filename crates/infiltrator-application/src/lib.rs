#![allow(
    clippy::large_enum_variant,
    clippy::collapsible_if,
    clippy::needless_borrow,
    clippy::needless_return,
    clippy::too_many_arguments,
    clippy::useless_conversion,
    clippy::single_match,
    clippy::let_and_return,
    clippy::clone_on_copy,
    clippy::field_reassign_with_default
)]
//! Application services for MusicFrog Infiltrator.
//!
//! The application layer is runtime-neutral. Its public surface is expressed
//! in contract values and ports, so a UI or FFI consumer never needs to know
//! about an executor, task handles, or concrete Mihomo clients.

use infiltrator_domain::sub_rules;
use std::sync::mpsc::sync_channel;
pub mod active_exit_application;
pub mod android_control_application;
pub mod android_core_owner_recovery_application;
pub mod cache_application;
pub mod certificate_authority_application;
pub mod command_application;
pub mod command_capability;
pub mod configuration_application;
pub mod connection_application;
#[cfg(test)]
mod connection_grouping_capture_tests;
#[cfg(test)]
mod connection_grouping_presentation_tests;
pub mod connection_rate_application;
#[cfg(test)]
mod connection_search_presentation_tests;
pub mod core_application;
pub mod core_capability_application;
#[cfg(test)]
pub mod core_capability_test_support;
pub mod core_owner_shutdown_application;
#[cfg(test)]
mod core_product_control_tests;
pub mod core_version_management_application;
pub mod dialer_chain_application;
pub mod dns_cache_application;
pub mod dns_latency_application;
pub mod dns_leak_application;
pub mod dns_query_application;
pub mod dns_self_heal_application;
pub mod dns_workbench_application;
pub mod doctor_application;
pub mod geo_database_application;
pub mod media_unlock_application;
pub mod mini_hud_application;
pub mod mrs_acceleration_application;
pub mod mtu_application;
pub mod network_application;
pub mod network_roaming_application;
pub mod offline_startup_application;
pub mod outbound_application;
pub mod overview;
pub mod overview_layout_application;
pub mod pac_application;
pub mod port_conflict_application;
pub mod privileged_network_application;
pub mod profile_aggregation_application;
pub mod profile_application;
pub mod profile_document_application;
pub mod profile_options_application;
pub mod profile_reset_application;
pub mod protocol_codec_application;
pub mod protocol_codec_matrix_application;
pub mod public_ip_application;
pub mod reconnect_mask_application;
pub mod resource_application;
pub mod responsive_viewport_application;
pub mod routing_application;
pub mod routing_intent_application;
pub mod rule_list_application;
pub mod rule_provider_application;
pub mod rule_provider_runtime_application;
pub mod rule_tracer_application;
pub mod runtime_configuration_application;
pub mod runtime_control_projection;
pub mod runtime_query_application;
mod runtime_scope;
pub mod runtime_status_application;
pub mod runtime_stream_application;
pub mod script_application;
#[cfg(feature = "script-engine-boa")]
pub mod script_engine_boa;
pub mod script_engine_direct;
pub mod script_export_application;
pub mod script_sandbox_matrix_application;
pub mod service_mode_application;
pub mod settings_application;
pub mod shortcut_application;
pub mod snapshot_application;
pub mod speedtest_application;
pub mod stun_probe_application;
pub mod subscription_import_application;
pub mod subscription_quota_application;
pub mod subscription_refresh_application;
pub mod surface_application;
pub mod sync_application;
pub mod sync_conflict_application;
#[cfg(test)]
mod sync_conflict_test_support;
pub mod system_proxy_application;
pub mod traffic_scale_application;
pub mod traffic_topology_application;
pub mod traffic_waveform_application;
pub mod uwp_loopback_application;
pub mod version_application;
pub mod vpn_application;
pub mod vpn_preparation;

use infiltrator_ports::application_runtime::ApplicationRuntime;
use std::future::Future;

/// Drive a typed future through the host-provided runtime without putting a
/// result type into the runtime port. This is used by application workers
/// that run on a dedicated thread and need to synchronously observe an
/// async-port result before publishing a snapshot.
pub(crate) fn run_on_runtime<T, F>(runtime: &dyn ApplicationRuntime, future: F) -> T
where
    T: Send + 'static,
    F: Future<Output = T> + Send + 'static,
{
    let (sender, receiver) = sync_channel(1);
    runtime.block_on(Box::pin(async move {
        let _ = sender.send(future.await);
    }));
    receiver
        .recv()
        .expect("application runtime dropped a completed future")
}

/// Validate a logical routing rule without exposing the domain error type to
/// a surface. The application owns the public error boundary; the pure AST
/// parser remains in `infiltrator-domain`.
pub fn validate_logical_rule_syntax(rule: &str) -> Result<(), String> {
    sub_rules::validate_logical_rule_syntax(rule).map_err(|error| error.to_string())
}

pub mod log_application;
pub mod log_export_application;
pub mod log_export_capture;
mod log_stream;

#[cfg(test)]
mod log_application_tests;

#[cfg(test)]
mod log_stream_test_support;

#[cfg(test)]
mod script_service_tests;

mod profile_editor_observations;

#[cfg(test)]
#[path = "outbound_application_tests.rs"]
mod outbound_application_tests;

#[cfg(test)]
#[path = "outbound_batch_tests.rs"]
mod outbound_batch_tests;

#[cfg(test)]
#[path = "command_outbound_tests.rs"]
mod command_outbound_tests;
