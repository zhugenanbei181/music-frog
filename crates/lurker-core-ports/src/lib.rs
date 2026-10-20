#![warn(missing_docs)]
//! Engine-neutral ports shared by every Lurker core host.
//!
//! No UI toolkit, host, transport, product crate or concrete engine may appear
//! here. The product port crate (`infiltrator-ports`) and each engine driver
//! depend on this crate, never the reverse.
//!
//! The ports are deliberately narrow:
//!
//! * [`core_process`] supervises the core process and proves readiness.
//! * [`core_scope`] publishes the scope every scoped operation is fenced to.
//! * [`outbound`] observes and changes outbound state for one activation.
//! * [`routing_intent`] observes and applies the shared routing intent.
//! * [`profile_document_codec`] inspects opaque profile documents purely.
//! * [`engine_backend`] bundles the ports of exactly one engine.
//! * [`engine_switch`] owns the durable owner-switch journal and its leases.
//!
//! Every port failure is a typed [`error::PortError`]; no port ever leaks a
//! transport, OS or runtime error type to the application boundary.
//!
//! # Example
//!
//! ```
//! use lurker_core_contract::capability::Capability;
//! use lurker_core_contract::error::{ErrorCode, Failure};
//! use lurker_core_ports::error::PortError;
//! use lurker_core_ports::outbound::{OutboundProbeRequest, OutboundProbeRequestError, ProbeTarget};
//!
//! // A probe target must be an absolute, credential-free HTTP(S) URL.
//! let target = ProbeTarget::http_url("https://example.test/health")
//!     .expect("credential-free HTTP URL is accepted");
//! let request = OutboundProbeRequest::new(target, 3_000).expect("positive timeout");
//! assert_eq!(request.timeout_ms(), 3_000);
//!
//! // A zero timeout is refused before the adapter ever sees the request.
//! assert!(matches!(
//!     OutboundProbeRequest::new(request.target().clone(), 0),
//!     Err(OutboundProbeRequestError::ZeroTimeout)
//! ));
//!
//! // An unsupported capability becomes a typed, non-retryable failure.
//! let failure: Failure = PortError::unsupported(Capability::Tun, "no TUN device").into();
//! assert_eq!(failure.code, ErrorCode::Unsupported);
//! assert!(!failure.retryable);
//! ```

pub mod core_process;
pub mod core_scope;
pub mod engine_backend;
pub mod engine_switch;
pub mod error;
pub mod outbound;
pub mod profile_document_codec;
pub mod routing_intent;
