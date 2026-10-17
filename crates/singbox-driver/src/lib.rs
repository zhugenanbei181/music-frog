//! Engine driver for the sing-box core.
//!
//! The driver owns the engine-specific control API client, outbound/routing
//! observation, readiness probe, native configuration codec and process
//! supervision. It carries no product type, platform host or UI dependency.
//!
//! Feature slices: `http` (Clash API, readiness, outbound, routing intent),
//! `process` (process supervision) and `codec` (native JSON codec, which drives
//! the process command boundary). Binary version probing is always available.

#[cfg(feature = "http")]
pub mod api;
#[cfg(feature = "backend")]
pub mod backend;
#[cfg(feature = "codec")]
pub mod codec;
#[cfg(feature = "process")]
pub mod driver;
#[cfg(feature = "http")]
pub mod outbound;
#[cfg(feature = "process")]
pub mod process;
#[cfg(feature = "http")]
pub mod readiness;
#[cfg(feature = "http")]
pub mod routing_intent;
pub mod version;

#[cfg(all(test, feature = "backend"))]
#[path = "backend_tests.rs"]
mod backend_tests;
#[cfg(all(test, feature = "http"))]
mod test_support;
