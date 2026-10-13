//! Pure cross-surface projections over accepted contract snapshots.
//!
//! This crate owns display reductions shared by Iced and Bevy. It performs no
//! I/O, has no runtime owner, and cannot dispatch commands.

pub mod aggregation_preview_projection;
pub mod command_palette_projection;
pub mod connection_detail_projection;
pub mod core_control_projection;
pub mod core_status_projection;
pub mod dns_leak_projection;
pub mod dns_mapping_projection;
pub mod doctor_projection;
pub mod failure_projection;
pub mod host_network_projection;
pub mod log_projection;
pub mod logical_rule_projection;
pub mod network_status_projection;
pub mod profile_edit_status_projection;
pub mod profile_editor_projection;
pub mod profile_metadata_projection;
pub mod profile_preview;
pub mod protocol_studio_projection;
pub mod routing_projection;
pub mod rule_json_projection;
pub mod rule_mrs_projection;
pub mod rule_provider_projection;
pub mod script_console_projection;
pub mod settings_preference_projection;
pub mod settings_status_projection;
pub mod shell_readout;
pub mod speedtest_detail_projection;
pub mod speedtest_summary_projection;
pub mod subscription_quota_projection;
pub mod subscription_status_projection;
pub mod sync_projection;
pub mod system_toggle_projection;
pub mod traffic_readout;
pub mod traffic_topology_projection;
