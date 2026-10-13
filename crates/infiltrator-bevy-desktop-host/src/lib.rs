//! Bevy desktop composition root.
//!
//! The UI crate owns the surface and accepts typed host adapters. This crate
//! owns `DesktopProductSession`, the application-backed snapshot source, and
//! the matching command sink.

mod command_sink;
mod desktop;
mod entry;
mod surface;

pub use desktop::run_desktop;
pub use entry::{DesktopEntry, DesktopEntryParseError, parse_entry};
