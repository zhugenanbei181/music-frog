//! Headless business-journey tests: drive the real `AppState::update()`
//! state machine through multi-step, cross-domain user stories, including
//! the async-result legs (异步结果回灌) and real filesystem outcomes inside
//! a per-journey temp HOME (global `mihomo_platform` override, mutex-
//! serialized). No window, no compositor, no tray, no network, no real user
//! directory — red lines.
//!
//! Journeys (name = documentation):
//! - `profile_lifecycle`  — import/activate/restart chain, subscription
//!   settings + auto-update persistence, manual update outcomes, delete with
//!   sidecar cleanup, and the router dead-end tripwire (documented defect).
//! - `options_editors`    — Editor mixin three-pane journey and the Filter
//!   pane journey, with sidecar-on-disk assertions.
//! - `sync_conflicts`     — key-level diff merge, conflict resolve/dismiss,
//!   mixed switch→rebuild→refetch journey.
//! - `runtime_tray_kernels` — lifecycle degradation, script-mode gate,
//!   kernel management + throttled tray refresh, tray event chains.
//! - `settings_lifecycle` — notification task surface, factory reset,
//!   language/theme persistence, toast lifecycle.
//!
//! Mounted via `src/test_mounts.rs` (crate root).
//!
//! test-intent: behavior

#[path = "business_flow/support.rs"]
mod support;

#[path = "business_flow/profile_lifecycle.rs"]
mod profile_lifecycle;

#[path = "business_flow/options_editors.rs"]
mod options_editors;

#[path = "business_flow/sync_conflicts.rs"]
mod sync_conflicts;

#[path = "business_flow/runtime_tray_kernels.rs"]
mod runtime_tray_kernels;

#[path = "business_flow/settings_lifecycle.rs"]
mod settings_lifecycle;
