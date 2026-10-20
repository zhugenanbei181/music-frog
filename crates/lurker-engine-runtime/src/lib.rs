#![warn(missing_docs)]
//! Engine-neutral backend provider registry.
//!
//! The runtime owns dispatch only: it never depends on an engine driver, a
//! product crate or a UI toolkit. Callers compose this registry with the
//! providers and drivers they support.
//!
//! * [`registry`] maps an engine identity to the provider that creates its
//!   backend bundle.
//! * [`facade`] is the high-level runtime handle an embedding host exposes to
//!   an application, with one exhaustive lifecycle state machine.
//! * [`switch`] owns the durable owner-switch coordinator and its journal.
//! * [`error`] carries the typed registry mutation failures.
//! * [`conformance`] is the optional, engine-neutral adapter conformance kit.
//!
//! # Example
//!
//! ```
//! use lurker_core_contract::identity::EngineId;
//! use lurker_core_ports::engine_backend::{
//!     EngineBackendPorts, EngineBackendProviderAvailability,
//! };
//! use lurker_core_ports::engine_switch::EngineSwitchJournal;
//! use lurker_engine_runtime::registry::EngineBackendProviderRegistry;
//! use lurker_engine_runtime::switch::memory_journal::MemoryEngineSwitchJournal;
//!
//! // The registry is empty until a host registers its providers; an unknown
//! // engine is a typed "unsupported", never a silent no-op.
//! let registry: EngineBackendProviderRegistry<EngineBackendPorts> =
//!     EngineBackendProviderRegistry::empty();
//! assert!(matches!(
//!     registry.availability(&EngineId::mihomo()),
//!     EngineBackendProviderAvailability::Unsupported { .. }
//! ));
//!
//! // The durable owner-switch journal starts unowned at revision zero.
//! let journal = MemoryEngineSwitchJournal::new(EngineId::mihomo());
//! let snapshot = futures_executor::block_on(journal.snapshot()).expect("journal snapshot");
//! assert_eq!(snapshot.revision.value(), 0);
//! assert!(snapshot.bound_owner().is_none());
//! ```

#[cfg(feature = "conformance")]
pub mod conformance;
pub mod error;
pub mod facade;
pub mod registry;
pub mod switch;
