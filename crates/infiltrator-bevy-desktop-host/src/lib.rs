//! Bevy desktop composition root.
//!
//! The UI crate owns the surface and accepts typed host adapters. This crate
//! owns `DesktopProductSession`, the application-backed snapshot source, and
//! the matching command sink.

pub mod command_sink;
pub mod desktop;
pub mod entry;
pub mod surface;
