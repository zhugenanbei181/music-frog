//! The application-wide [`Message`] bus compact `Debug` implementation.
//!
//! The match arms are grouped by message family and live in sibling modules;
//! this module only dispatches to the first section that recognises a variant.
//! Grouping keeps each file readable without a `pub use` forwarding layer.

use crate::types::message::Message;
use std::fmt;
use std::fmt::{Debug, Formatter};

mod config;
mod session;
mod wave2;
mod wave34;

impl Debug for Message {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for section in [session::fmt, config::fmt, wave2::fmt, wave34::fmt] {
            if let Some(result) = section(self, f) {
                return result;
            }
        }
        write!(f, "Message")
    }
}
