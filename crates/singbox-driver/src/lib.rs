#![warn(missing_docs)]
//! Engine driver for the sing-box core.
//!
//! The driver owns the engine-specific control API client, outbound/routing
//! observation, readiness probe, native configuration codec and process
//! supervision. It carries no product type, platform host or UI dependency.
//!
//! Feature slices: `http` (Clash API, readiness, outbound, routing intent),
//! `process` (process supervision) and `codec` (native JSON codec, which drives
//! the process command boundary). Binary version probing is always available.
//!
//! # Example
//!
//! ```
//! use singbox_driver::codec::profile_document_codec::{
//!     PROFILE_FORMAT, PROFILE_SCHEMA_VERSION, SingBoxProfileDocumentCodec,
//! };
//! use lurker_core_ports::profile_document_codec::ProfileDocumentCodecPort;
//!
//! let codec = SingBoxProfileDocumentCodec;
//!
//! // The codec is pure: it inspects exact bytes and never touches storage.
//! assert_eq!(codec.scope().format_id.as_str(), PROFILE_FORMAT);
//! assert_eq!(codec.scope().schema_version.value(), PROFILE_SCHEMA_VERSION);
//!
//! // A valid document is a JSON object ...
//! assert!(matches!(
//!     codec.inspect(br#"{"outbounds": []}"#).expect("inspection never fails"),
//!     lurker_core_contract::profile_document::DocumentValidation::Valid
//! ));
//!
//! // ... and a syntactically broken document is reported, not guessed.
//! assert!(matches!(
//!     codec.inspect(b"{").expect("inspection never fails"),
//!     lurker_core_contract::profile_document::DocumentValidation::Invalid { .. }
//! ));
//! ```

#[cfg(feature = "http")]
pub mod api;
#[cfg(feature = "backend")]
pub mod backend;
#[cfg(feature = "codec")]
pub mod codec;
#[cfg(feature = "process")]
pub mod driver;
#[cfg(feature = "http")]
pub mod outbound;
#[cfg(feature = "process")]
pub mod process;
#[cfg(feature = "http")]
pub mod readiness;
#[cfg(feature = "http")]
pub mod routing_intent;
pub mod version;

#[cfg(all(test, feature = "backend"))]
#[path = "backend_tests.rs"]
mod backend_tests;
#[cfg(all(test, feature = "http"))]
mod test_support;
