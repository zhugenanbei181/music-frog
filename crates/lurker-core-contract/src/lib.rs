//! Engine-neutral proxy core contract shared by every Lurker surface.
//!
//! No UI toolkit, host, transport or product-private type may appear here. The
//! product contract (`infiltrator-contract`) and each engine driver depend on
//! this crate, never the reverse.

pub mod session;
