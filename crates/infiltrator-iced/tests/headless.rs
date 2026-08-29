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
