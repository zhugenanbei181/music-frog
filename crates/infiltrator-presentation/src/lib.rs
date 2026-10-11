//! Pure cross-surface projections over accepted contract snapshots.
//!
//! This crate owns display reductions shared by Iced and Bevy. It performs no
//! I/O, has no runtime owner, and cannot dispatch commands.

pub mod profile_preview;
pub mod shell_readout;
pub mod traffic_readout;
