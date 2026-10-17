//! Engine driver for the sing-box core.
//!
//! The driver owns the engine-specific control API client, configuration
//! document codec and process supervision behind `lurker-core-ports`. It
//! carries no product type, platform host or UI dependency.
//!
//! Implementation lands with PUB-002.3/002.4.
