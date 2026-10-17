//! Engine driver for the mihomo (Clash.Meta) core.
//!
//! The driver owns the engine-specific control API client, configuration
//! document codec and process supervision behind `lurker-core-ports`. It
//! carries no product type, platform host or UI dependency.
//!
//! Implementation lands with PUB-002.1/002.2.
