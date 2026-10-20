#![warn(missing_docs)]
//! Engine-neutral proxy core contract shared by every Lurker surface.
//!
//! No UI toolkit, host, transport or product-private type may appear here. The
//! product contract (`infiltrator-contract`) and each engine driver depend on
//! this crate, never the reverse.
//!
//! The contract is built around two fences that every surface relies on:
//!
//! * **Scope fencing** — every observation carries a [`identity::CoreScope`]
//!   (stable identity + activation generation + optional session token). A
//!   readback that does not match the current scope belongs to an earlier
//!   activation and must be rejected instead of applied.
//! * **Typed failure** — every failure is an [`error::Failure`] with a
//!   machine-readable [`error::ErrorCode`] and, where the cause is known, a
//!   typed [`error::FailureReason`]. Callers branch on the type, never on the
//!   message text.
//!
//! # Example
//!
//! ```
//! use lurker_core_contract::identity::{
//!     CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
//! };
//! use lurker_core_contract::session::SessionToken;
//!
//! // One activation of one core instance, owned by one session.
//! let identity = CoreIdentity::new(
//!     EngineId::mihomo(),
//!     CoreInstanceId::new("desktop-main").expect("valid instance ID"),
//!     None,
//! );
//! let scope = CoreScope {
//!     identity: identity.clone(),
//!     generation: CoreGeneration::new(1),
//!     session_token: Some(SessionToken::new(7)),
//! };
//!
//! // Version evidence may change after readiness; the owner fence does not.
//! assert!(identity.same_instance(&identity));
//! assert_eq!(scope.generation.value(), 1);
//! assert!(scope.session_token.expect("session token").is_valid());
//! ```

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
