#![allow(
    clippy::large_enum_variant,
    clippy::derivable_impls,
    clippy::manual_pattern_char_comparison,
    clippy::bool_assert_comparison,
    clippy::field_reassign_with_default
)]
//! Pure cross-surface projections over accepted contract snapshots.
//!
//! This crate owns display reductions shared by Iced and Bevy. It performs no
//! I/O, has no runtime owner, and cannot dispatch commands.

pub mod aggregation_preview_projection;
pub mod app_routing_page_model;
pub mod chart;
pub mod clock;
pub mod command_palette_projection;
pub mod connection_detail_projection;
pub mod connection_grouping;
pub mod connection_page_model;
pub mod connection_search;
pub mod core_control_projection;
pub mod diff;
pub mod dns_cache_actions;
pub mod dns_cache_projection;
pub mod dns_health_projection;
pub mod dns_hosts_editor;
pub mod dns_hosts_projection;
pub mod dns_latency_projection;
pub mod dns_leak_projection;
pub mod dns_mapping_projection;
pub mod dns_page_model;
pub mod dns_query_actions;
pub mod dns_query_history_projection;
pub mod dns_query_projection;
pub mod dns_settings_projection;
pub mod doctor_actions;
pub mod doctor_page_model;
pub mod doctor_projection;
pub mod dpad_navigation_projection;
pub mod editable_draft;
pub mod filter_query;
pub mod geo;
pub mod host_network_projection;
pub mod input_intent;
pub mod ip_quality_projection;
pub mod lan_share_projection;
pub mod language_choice;
pub mod latency_projection;
pub mod list_filter;
pub mod log_export_actions;
pub mod log_export_projection;
pub mod log_page_model;
pub mod log_projection;
pub mod log_search;
pub mod logical_rule_projection;
pub mod media_unlock_projection;
pub mod mixin_studio_projection;
pub mod network_status_projection;
pub mod operation_phase;
pub mod overview_page_model;
pub mod performance_watchdog;
pub mod privilege_repair_projection;
pub mod profile_edit_session;
pub mod profile_edit_status_projection;
pub mod profile_editor_projection;
pub mod profile_metadata_projection;
pub mod profile_preview;
pub mod profiles_page_model;
pub mod protocol_editor_operation;
pub mod protocol_form;
pub mod protocol_studio_projection;
pub mod proxies_page_model;
pub mod proxy_strategy_projection;
pub mod routing_intent_projection;
pub mod routing_projection;
pub mod rule_condition_projection;
pub mod rule_form_binding;
pub mod rule_json_projection;
pub mod rule_list_editor;
pub mod rule_list_projection;
pub mod rule_mrs_projection;
pub mod rule_provider_projection;
pub mod rule_row_projection;
pub mod rule_statistics_inspector_projection;
pub mod rule_statistics_projection;
pub mod rule_statistics_workbench;
pub mod rule_trace_actions;
pub mod rule_trace_projection;
pub mod rules_page_model;
pub mod runtime_probe_settings;
pub mod runtime_probe_settings_editor;
pub mod script_console_projection;
pub mod script_export_projection;
pub mod script_workbench;
pub mod settings_page_model;
pub mod settings_status_projection;
pub mod shell_readout;
pub mod shell_readout_fold;
pub mod skeleton_projection;
pub mod snapshot_presentation;
pub mod snapshot_restore_projection;
pub mod snapshot_restore_workbench;
pub mod speedtest_detail_projection;
pub mod speedtest_summary_projection;
pub mod stun_projection;
pub mod subscription_diagnostic_projection;
pub mod subscription_filter_copy;
pub mod subscription_filter_editor;
pub mod subscription_quota_projection;
pub mod subscription_status_projection;
pub mod sync_page_model;
pub mod sync_projection;
pub mod system_toggle_projection;
pub mod top_talkers_projection;
pub mod traffic_readout;
pub mod traffic_topology_projection;
pub mod tween;
pub mod ui_presence;
pub mod unified_runtime_status;

#[cfg(test)]
mod dns_projections_tests;

mod text;

#[cfg(test)]
mod dns_cache_actions_tests;

#[cfg(test)]
mod dns_query_actions_tests;

#[cfg(test)]
mod log_export_actions_tests;

#[cfg(test)]
mod log_search_tests;

#[cfg(test)]
mod script_workbench_tests;

#[cfg(test)]
mod rule_fixtures;

#[cfg(test)]
mod snapshot_restore_workbench_tests;

#[cfg(test)]
#[path = "runtime_probe_settings_tests.rs"]
mod runtime_probe_settings_tests;
