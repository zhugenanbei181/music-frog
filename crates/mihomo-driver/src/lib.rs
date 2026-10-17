//! Engine driver for the mihomo (Clash.Meta) core.
//!
//! The driver owns the engine-specific control API client, WebSocket streams,
//! process supervision and version knowledge behind `lurker-core-ports`. It
//! carries no product type, platform host or UI dependency: every product
//! surface consumes these types and implements its own product-level ports on
//! top of them.
//!
//! Feature layout:
//! * `http` — REST controller client, log/connection/traffic WebSocket streams,
//!   outbound inventory probes, routing-intent switching and controller
//!   readiness probing.
//! * `process` — desktop process supervision: spawn, readiness, PID ownership,
//!   exact terminal readback and child reaping.
//! * `codec` — native Mihomo YAML codec (inspection and editor snippet splice)
//!   plus the native `-t` configuration dry-run, which drives the process
//!   command boundary.

#[cfg(feature = "http")]
pub mod api;
#[cfg(feature = "backend")]
pub mod backend;
#[cfg(feature = "codec")]
pub mod codec;
pub mod driver;
pub mod error;
#[cfg(feature = "http")]
pub mod network_error;
#[cfg(feature = "http")]
pub mod outbound;
#[cfg(feature = "process")]
pub mod process;
#[cfg(feature = "http")]
pub mod routing_intent;
#[cfg(feature = "http")]
pub mod types;
pub mod version;

#[cfg(all(test, feature = "backend"))]
#[path = "backend_tests.rs"]
mod backend_tests;
#[cfg(test)]
#[path = "error_test.rs"]
mod error_test;
#[cfg(all(test, feature = "http"))]
#[path = "types_test.rs"]
mod types_test;
