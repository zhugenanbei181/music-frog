//! Mount points that pull the `tests/` tree into the lib's unit-test binary.
//!
//! Production modules keep only short `#[cfg(test)] #[path = ...]` mount
//! declarations; test bodies live under `tests/{common,headless,gui}`.
//! Crate-root-level GUI-pipeline tests (former inline `src/tests.rs`) mount
//! here; module-scoped tests mount from their owning production module
//! (`tray.rs`, `utils.rs`, `admin_server.rs`, `demo.rs`).
//!
//! Kept as a dedicated module so `#[path]` targets resolve uniformly
//! relative to `src/` and `lib.rs` stays free of test plumbing.

// test-intent: behavior (mount-only module; bodies live under tests/gui/)
#[path = "../tests/gui/app_state_tests.rs"]
mod app_state_tests;

#[path = "../tests/gui/proxy_mode_race_tests.rs"]
mod proxy_mode_race_tests;

#[path = "../tests/gui/proxy_mode_native_tests.rs"]
mod proxy_mode_native_tests;

#[path = "../tests/gui/proxy_logic_tests.rs"]
mod proxy_logic_tests;

#[path = "../tests/gui/rules_dns_tests.rs"]
mod rules_dns_tests;

#[path = "../tests/gui/admin_settings_tests.rs"]
mod admin_settings_tests;

#[path = "../tests/gui/filter_editor_race_tests.rs"]
mod filter_editor_race_tests;
#[path = "../tests/gui/options_flow_tests.rs"]
mod options_flow_tests;

#[path = "../tests/gui/doctor_flow_tests.rs"]
mod doctor_flow_tests;

#[path = "../tests/gui/business_flow_tests.rs"]
mod business_flow_tests;

#[path = "../tests/gui/crash_report_tests.rs"]
mod crash_report_tests;

#[path = "../tests/gui/iced_six_advancements_tests.rs"]
mod iced_six_advancements_tests;

#[path = "../tests/gui/iced_six_advancements_wave2_tests.rs"]
mod iced_six_advancements_wave2_tests;

#[path = "../tests/gui/iced_six_advancements_wave3_tests.rs"]
mod iced_six_advancements_wave3_tests;

#[path = "../tests/gui/iced_six_advancements_wave4_tests.rs"]
mod iced_six_advancements_wave4_tests;

#[path = "../tests/gui/iced_six_advancements_wave5_tests.rs"]
mod iced_six_advancements_wave5_tests;

#[path = "../tests/gui/surface_contract_tests.rs"]
mod surface_contract_tests;

#[path = "../tests/gui/responsive_elasticity_tests.rs"]
mod responsive_elasticity_tests;

#[path = "../tests/gui/protocol_codec_tests.rs"]
mod protocol_codec_tests;

#[path = "../tests/gui/multimodal_shell_tests.rs"]
mod multimodal_shell_tests;

#[path = "../tests/gui/ime_tests.rs"]
mod ime_tests;

#[path = "../tests/gui/gesture_tests.rs"]
mod gesture_tests;

#[path = "../tests/common/command_harness.rs"]
pub(crate) mod command_harness;

#[path = "../tests/gui/filter_native_tests.rs"]
mod filter_native_tests;

#[path = "../tests/gui/connection_grouping_native_tests.rs"]
mod connection_grouping_native_tests;
#[path = "../tests/common/native_widgets.rs"]
pub(crate) mod native_widgets;

#[path = "../tests/gui/connection_search_native_tests.rs"]
mod connection_search_native_tests;

#[path = "../tests/gui/log_owner_tests.rs"]
mod log_owner_tests;

#[path = "../tests/gui/log_search_native_tests.rs"]
mod log_search_native_tests;

#[path = "../tests/gui/proxy_scope_tests.rs"]
mod proxy_scope_tests;

#[path = "../tests/gui/traffic_observation_tests.rs"]
mod traffic_observation_tests;

#[path = "../tests/gui/script_workbench_tests.rs"]
pub(crate) mod script_workbench_tests;

#[path = "../tests/gui/snapshot_restore_tests.rs"]
pub(crate) mod snapshot_restore_tests;

#[path = "../tests/gui/profile_edit_fixture.rs"]
pub mod profile_edit_fixture;

#[path = "../tests/gui/editor_observation_tests.rs"]
mod editor_observation_tests;
#[path = "../tests/gui/runtime_copy_tests.rs"]
mod runtime_copy_tests;
