//! Headless integration tests: pure logic through the public lib API only.
//!
//! These run in their own test binary compiled against `infiltrator_iced`
//! as an external crate — no window, no compositor, no GUI toolkit runtime,
//! no system side effects (demo mode gates every production integration).
//! test-intent: behavior

#[path = "common/test_support.rs"]
mod test_support;

#[path = "headless/demo_fixture_tests.rs"]
mod demo_fixture_tests;

#[path = "headless/confirmation_tests.rs"]
mod confirmation_tests;

#[path = "headless/i18n_parity_tests.rs"]
mod i18n_parity_tests;

// DUAL-10-15: the shared scripting-sandbox regression matrix on Iced.
#[path = "headless/scripting_matrix_tests.rs"]
mod scripting_matrix_tests;

#[path = "headless/speedtest_details_tests.rs"]
mod speedtest_details_tests;

#[path = "headless/palette_scenarios_tests.rs"]
mod palette_scenarios_tests;

#[path = "common/command_harness.rs"]
mod command_harness;

#[path = "headless/connection_inspection_tests.rs"]
mod connection_inspection_tests;

#[path = "headless/protocol_form_tests.rs"]
mod protocol_form_tests;

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
#[path = "common/doctor_port.rs"]
mod doctor_port;

#[path = "headless/dns_leak_action_tests.rs"]
mod dns_leak_action_tests;

#[path = "headless/dns_hosts_action_tests.rs"]
mod dns_hosts_action_tests;

#[path = "headless/dns_cache_action_tests.rs"]
mod dns_cache_action_tests;

#[path = "headless/dns_query_action_tests.rs"]
mod dns_query_action_tests;

#[path = "headless/connection_grouping_tests.rs"]
mod connection_grouping_tests;
