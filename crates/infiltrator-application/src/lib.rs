//! Application services for MusicFrog Infiltrator.
//!
//! The application layer is runtime-neutral. Its public surface is expressed
//! in contract values and ports, so a UI or FFI consumer never needs to know
//! about an executor, task handles, or concrete Mihomo clients.

use infiltrator_domain::sub_rules;
use std::sync::mpsc::sync_channel;
pub mod active_exit_application;
pub mod aggregation_preview_projection;
pub mod cache_application;
pub mod certificate_authority_application;
pub mod command_application;
pub mod command_palette_projection;
pub mod configuration_application;
pub mod connection_application;
pub mod connection_detail_projection;
pub mod connection_grouping;
pub mod connection_grouping_fixtures;
pub mod connection_rate_application;
pub mod connection_search;
pub mod core_application;
pub mod core_control_projection;
pub mod core_status_projection;
pub mod dialer_chain_application;
pub mod dns_cache_actions;
pub mod dns_cache_application;
pub mod dns_cache_fixtures;
pub mod dns_cache_projection;
pub mod dns_health_projection;
pub mod dns_hosts_editor;
pub mod dns_hosts_fixtures;
pub mod dns_hosts_projection;
pub mod dns_latency_application;
pub mod dns_latency_projection;
pub mod dns_leak_actions;
pub mod dns_leak_application;
pub mod dns_leak_fixtures;
pub mod dns_leak_projection;
pub mod dns_mapping_projection;
pub mod dns_observation_projection;
pub mod dns_query_actions;
pub mod dns_query_application;
pub mod dns_query_fixtures;
pub mod dns_query_projection;
pub mod dns_self_heal_application;
pub mod dns_status_projection;
pub mod dns_workbench_application;
pub mod doctor_actions;
pub mod doctor_application;
pub mod doctor_capture_fixtures;
pub mod doctor_projection;
pub mod failure_projection;
pub mod filter_capture_store;
pub mod language_choice;
pub mod language_choice_fixtures;
pub mod latency_projection;
pub mod mini_hud_application;
pub mod mixin_studio_projection;
pub mod mrs_acceleration_application;
pub mod mtu_application;
pub mod network_application;
pub mod network_roaming_application;
pub mod network_status_projection;
pub mod offline_startup_application;
pub mod overview;
pub mod overview_layout_application;
pub mod pac_application;
pub mod port_conflict_application;
pub mod privileged_network_application;
pub mod profile_aggregation_application;
pub mod profile_application;
pub mod profile_document_application;
pub mod profile_editor_projection;
pub mod profile_metadata_projection;
pub mod profile_options_application;
pub mod profile_reset_application;
#[cfg(test)]
mod profile_workspace_test_support;
pub mod protocol_codec_application;
pub mod protocol_codec_matrix_application;
pub mod protocol_node_params;
pub mod protocol_node_projection;
pub mod proxy_application;
pub mod proxy_group_order_editor;
pub mod proxy_group_order_fixtures;
pub mod proxy_inspection_fixtures;
pub mod proxy_inspection_projection;
pub mod proxy_inspection_reader;
pub mod proxy_mode_actions;
pub mod proxy_mode_application;
pub mod proxy_mode_fixtures;
pub mod proxy_mode_projection;
pub mod proxy_preferences_application;
pub mod proxy_probe_editor;
pub mod proxy_probe_options_projection;
pub mod proxy_projection;
pub mod proxy_search_fixtures;
pub mod proxy_search_projection;
pub mod public_ip_application;
pub mod reconnect_mask_application;
pub mod resource_application;
pub mod responsive_viewport_application;
pub mod routing_application;
pub mod routing_projection;
pub mod rule_condition_projection;
pub mod rule_form_binding;
pub mod rule_list_application;
pub mod rule_list_editor;
pub mod rule_list_fixtures;
pub mod rule_list_projection;
pub mod rule_provider_application;
pub mod rule_row_projection;
pub mod rule_source_identity;
pub mod rule_trace_actions;
pub mod rule_trace_fixtures;
pub mod rule_trace_projection;
pub mod rule_tracer_application;
pub mod runtime_control_projection;
pub mod runtime_query_application;
pub mod script_application;
pub mod script_console_projection;
#[cfg(feature = "script-engine-boa")]
pub mod script_engine_boa;
pub mod script_engine_direct;
pub mod script_export_application;
pub mod script_export_projection;
pub mod script_sandbox_matrix_application;
pub mod script_workbench;
pub mod service_mode_application;
pub mod settings_application;
pub mod settings_preference_projection;
pub mod settings_status_projection;
pub mod shell_readout_application;
pub mod shell_readout_projection;
pub mod shortcut_application;
pub mod snapshot_application;
pub mod snapshot_restore_projection;
pub mod snapshot_restore_workbench;
pub mod speedtest_application;
pub mod speedtest_matrix_application;
pub mod stun_probe_application;
pub mod stun_projection;
pub mod subscription_filter_copy;
pub mod subscription_filter_editor;
pub mod subscription_filter_fixture;
pub mod subscription_import_application;
pub mod subscription_quota_application;
pub mod subscription_refresh_application;
pub mod subscription_status_projection;
pub mod surface_application;
pub mod surface_reader;
pub mod sync_application;
pub mod sync_projection;
pub mod system_proxy_application;
pub mod system_toggle_application;
pub mod system_toggle_projection;
pub mod telemetry_observation_fixtures;
pub mod traffic_readout_projection;
pub mod traffic_scale_application;
pub mod traffic_topology_application;
pub mod traffic_topology_navigation_application;
pub mod traffic_topology_projection;
pub mod traffic_waveform_application;
pub mod uwp_loopback_application;
pub mod version_application;
pub mod vpn_application;

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

pub mod byte_format;
pub mod speedtest_detail_projection;
pub mod speedtest_summary_projection;

pub mod protocol_form;

pub mod protocol_studio_projection;

pub mod search_text;

pub mod log_capture_fixtures;
pub mod log_projection;
pub mod log_search;

pub mod log_application;
pub mod log_export_actions;
pub mod log_export_application;
pub mod log_export_capture;
pub mod log_export_projection;
pub mod log_follow;
mod log_stream;

#[cfg(test)]
mod log_application_tests;

#[cfg(test)]
mod log_stream_test_support;

#[cfg(test)]
mod log_search_tests;

pub mod logical_rule_projection;
pub mod rule_json_projection;
pub mod rule_mrs_projection;
pub mod rule_provider_projection;
pub mod rule_statistics_projection;

pub mod rule_statistics_workbench;

pub mod rule_statistics_inspector_projection;

#[cfg(test)]
mod script_service_tests;

pub mod host_network_projection;

pub mod snapshot_presentation;

pub mod subscription_quota_projection;

mod profile_editor_observations;

pub mod profile_edit_session;
