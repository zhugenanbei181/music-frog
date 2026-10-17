//! Engine-neutral proxy core contract shared by every Lurker surface.
//!
//! No UI toolkit, host, transport or product-private type may appear here. The
//! product contract (`infiltrator-contract`) and each engine driver depend on
//! this crate, never the reverse.

pub mod core_process;
pub mod error;
pub mod identity;
pub mod lifecycle;
pub mod outbound;
pub mod owner_switch;
pub mod routing_intent;
pub mod session;
