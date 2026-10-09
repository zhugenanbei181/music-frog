#[path = "headless/support.rs"]
mod support;

#[path = "headless/dns_observation_tests.rs"]
mod dns_observation_tests;

#[path = "headless/shell_tests.rs"]
mod shell_tests;

#[path = "headless/overview_tests.rs"]
mod overview_tests;

#[path = "headless/controller_tests.rs"]
mod controller_tests;

#[path = "headless/pages_matrix_tests.rs"]
mod pages_matrix_tests;

#[path = "headless/pages_matrix_a_tests.rs"]
mod pages_matrix_a_tests;

#[path = "headless/pages_matrix_b_tests.rs"]
mod pages_matrix_b_tests;

#[path = "headless/responsive_ui_tests.rs"]
mod responsive_ui_tests;

#[path = "headless/command_palette_tests.rs"]
mod command_palette_tests;

#[path = "headless/mini_hud_tests.rs"]
mod mini_hud_tests;

#[path = "headless/surface_tests.rs"]
mod surface_tests;

#[path = "headless/theme_skin_tests.rs"]
mod theme_skin_tests;

#[path = "headless/shortcut_tests.rs"]
mod shortcut_tests;

#[path = "headless/toast_overlay_tests.rs"]
mod toast_overlay_tests;

#[path = "headless/design_token_tests.rs"]
mod design_token_tests;

#[path = "headless/window_chrome_tests.rs"]
mod window_chrome_tests;

#[path = "headless/a11y_semantics_tests.rs"]
mod a11y_semantics_tests;

#[path = "headless/tray_status_tests.rs"]
mod tray_status_tests;

#[path = "headless/cadence_tests.rs"]
mod cadence_tests;

#[path = "headless/ime_tests.rs"]
mod ime_tests;

#[path = "headless/gesture_tests.rs"]
mod gesture_tests;

// DUAL-05: protocol-ecosystem studio (custom node URI codec) dual-surface tests.
#[path = "headless/protocol_codec_tests.rs"]
mod protocol_codec_tests;

#[path = "headless/inspection_scroll_tests.rs"]
mod inspection_scroll_tests;

#[path = "common/command_harness.rs"]
mod command_harness;
#[path = "headless/protocol_form_tests.rs"]
mod protocol_form_tests;

#[path = "headless/product_launch_tests.rs"]
mod product_launch_tests;

#[path = "headless/localization_tests.rs"]
mod localization_tests;
#[path = "headless/mode_change_tests.rs"]
mod mode_change_tests;
#[path = "headless/runtime_observation_tests.rs"]
mod runtime_observation_tests;
#[path = "headless/settings_observation_tests.rs"]
mod settings_observation_tests;

#[path = "headless/proxy_identity_tests.rs"]
mod proxy_identity_tests;
#[path = "headless/proxy_inspection_tests.rs"]
mod proxy_inspection_tests;

#[path = "headless/core_control_tests.rs"]
mod core_control_tests;

#[path = "headless/proxy_search_tests.rs"]
mod proxy_search_tests;

#[path = "common/probe_settings_store.rs"]
mod probe_settings_store;
#[path = "headless/probe_settings_tests.rs"]
mod probe_settings_tests;

#[path = "headless/group_order_tests.rs"]
mod group_order_tests;

#[path = "headless/language_choice_tests.rs"]
mod language_choice_tests;

#[path = "headless/doctor_action_tests.rs"]
mod doctor_action_tests;
#[path = "headless/doctor_port.rs"]
mod doctor_port;

#[path = "headless/dns_leak_action_tests.rs"]
mod dns_leak_action_tests;

#[path = "headless/dns_hosts_action_tests.rs"]
mod dns_hosts_action_tests;

#[path = "headless/dns_cache_action_tests.rs"]
mod dns_cache_action_tests;

#[path = "headless/dns_query_action_tests.rs"]
mod dns_query_action_tests;

#[path = "common/native_input.rs"]
mod native_input;
#[path = "headless/rule_list_action_tests.rs"]
mod rule_list_action_tests;
#[path = "headless/rule_trace_action_tests.rs"]
mod rule_trace_action_tests;

#[path = "headless/filter_transaction_tests.rs"]
mod filter_transaction_tests;

#[path = "headless/log_owner_tests.rs"]
mod log_owner_tests;

#[path = "headless/log_export_tests.rs"]
mod log_export_tests;

#[path = "headless/traffic_observation_tests.rs"]
mod traffic_observation_tests;

#[path = "headless/rule_statistics_action_tests.rs"]
mod rule_statistics_action_tests;

#[path = "headless/rule_locale_tests.rs"]
mod rule_locale_tests;

#[path = "headless/routing_locale_tests.rs"]
mod routing_locale_tests;

#[path = "headless/script_workbench_tests.rs"]
mod script_workbench_tests;

#[path = "headless/dns_form_locale_tests.rs"]
mod dns_form_locale_tests;

#[path = "headless/host_network_locale_tests.rs"]
mod host_network_locale_tests;

#[path = "headless/settings_preferences_locale_tests.rs"]
mod settings_preferences_locale_tests;

#[path = "headless/speedtest_locale_tests.rs"]
mod speedtest_locale_tests;

#[path = "headless/sync_observation_tests.rs"]
mod sync_observation_tests;

#[path = "headless/profile_metadata_locale_tests.rs"]
mod profile_metadata_locale_tests;

#[path = "headless/snapshot_restore_tests.rs"]
mod snapshot_restore_tests;

#[path = "headless/editor_observation_tests.rs"]
mod editor_observation_tests;

#[path = "headless/globe_tests.rs"]
mod globe_tests;
