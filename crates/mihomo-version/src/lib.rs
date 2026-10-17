//! Mihomo core delivery: release channels, artifact download/verification and
//! the installed-version manager.
//!
//! Engine version knowledge (engine identity, capability matrix, target
//! inventory) and the neutral controller client live in `mihomo-driver`.

pub mod app_updater;
pub mod bundled_kernel;
pub mod channel;
pub mod download;
pub mod geo_updater;
pub mod manager;
pub mod verify;
