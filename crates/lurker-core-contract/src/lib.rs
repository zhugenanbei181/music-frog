//! Engine-neutral proxy core contract shared by every Lurker surface.
//!
//! No UI toolkit, host, transport or product-private type may appear here. The
//! product contract (`infiltrator-contract`) and each engine driver depend on
//! this crate, never the reverse.

pub mod active_profile_reference;
pub mod capability;
pub mod core_process;
pub mod engine_switch;
pub mod error;
pub mod identity;
pub mod lifecycle;
pub mod outbound;
pub mod owner_switch;
pub mod page;
pub mod profile_document;
pub mod profile_document_identity;
pub mod profile_name;
pub mod profile_protection;
pub mod redact;
pub mod routing_intent;
pub mod session;
