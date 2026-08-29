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

#[path = "../tests/gui/proxy_logic_tests.rs"]
mod proxy_logic_tests;

#[path = "../tests/gui/rules_dns_tests.rs"]
mod rules_dns_tests;

#[path = "../tests/gui/admin_settings_tests.rs"]
mod admin_settings_tests;
