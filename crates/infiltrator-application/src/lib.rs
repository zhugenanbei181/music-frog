//! Application services for MusicFrog Infiltrator.
//!
//! The application layer is allowed to use Tokio internally. Its public
//! surface is deliberately expressed in contract values and standard Rust
//! methods, so a UI or FFI consumer never needs to know about Tokio channels,
//! task handles, or concrete Mihomo clients.

pub mod core_application;
pub mod overview;
