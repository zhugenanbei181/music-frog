//! Controller REST client, controller wire streams and raw proxy decoding.
//!
//! Everything here speaks the Mihomo/Clash controller HTTP and WebSocket
//! protocol. It is engine knowledge, not product knowledge: no product port or
//! product DTO appears in this module.

/// The controller REST client and its lifecycle-aware WebSocket streams.
pub mod client;
mod memory;
/// The neutral decoder of the controller `/proxies` observation wire.
pub mod proxy;
pub(crate) mod seam;
mod stream;

#[cfg(test)]
#[path = "api/client_auth_tests.rs"]
mod auth_tests;
#[cfg(test)]
#[path = "api/client_tests.rs"]
mod client_tests;
#[cfg(test)]
#[path = "api/proxy_wire_tests.rs"]
mod proxy_wire_tests;
