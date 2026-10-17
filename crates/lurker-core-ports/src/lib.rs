//! Engine-neutral ports shared by every Lurker core host.
//!
//! No UI toolkit, host, transport, product crate or concrete engine may appear
//! here. The product port crate (`infiltrator-ports`) and each engine driver
//! depend on this crate, never the reverse.

pub mod core_process;
pub mod core_scope;
pub mod engine_backend;
pub mod engine_switch;
pub mod error;
pub mod outbound;
pub mod profile_document_codec;
pub mod routing_intent;
