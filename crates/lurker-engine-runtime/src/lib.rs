//! Engine-neutral backend provider registry.
//!
//! The runtime owns dispatch only: it never depends on an engine driver, a
//! product crate or a UI toolkit. Callers compose this registry with the
//! providers and drivers they support.

pub mod error;
pub mod registry;
pub mod switch;
