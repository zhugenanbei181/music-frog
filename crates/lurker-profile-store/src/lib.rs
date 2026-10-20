#![allow(
    clippy::bool_comparison,
    clippy::derivable_impls,
    clippy::collapsible_if,
    clippy::items_after_test_module
)]
#![warn(missing_docs)]
//! Portable profile-store kernel.
//!
//! This crate owns the persistence machinery that is identical for every
//! backend, platform and UI: the on-disk layout description, durable atomic
//! document replacement, cross-process write fences, coherent workspace
//! publication, the profile-state TOML schema and one engine-scoped store
//! identity. It carries no credential, subscription, Age/keyring, product path
//! literal or UI dependency; product crates inject those through
//! [`layout::ProfileStoreLayout`], [`profile_files::ProfileDeletionParticipant`],
//! [`workspace::ProfileSourceFacts`] and their own secure stores.
//!
//! Platform code is not part of the portable logic: every filesystem and
//! kernel-lock primitive is expressed by [`platform::StorePlatform`], and
//! [`platform::NativeStorePlatform`] is the only implementation that contains
//! per-OS code. Hosts and tests substitute their own platform through
//! [`store::ProfileStoreCore::with_platform`].
//!
//! The two rules every consumer relies on:
//!
//! * **Durable writes are read back.** A record is only "committed" once the
//!   bytes on disk match the committed value; an uncertain durability failure
//!   is reconciled by reading the record back, never by assuming success.
//! * **Write fences are ordered.** The product-root lock is taken before the
//!   owner-state lock, and the owner-state lock before the profile-write lock.
//!   A path that takes them in another order is a protocol violation.
//!
//! # Example
//!
//! ```
//! use std::path::Path;
//! use lurker_profile_store::layout::ProfileStoreLayout;
//!
//! // The layout owns every historical file name; the kernel never embeds a
//! // product path literal.
//! let layout = ProfileStoreLayout::default();
//! let config_root = Path::new("/home/app/.config/music-frog");
//!
//! // One engine namespace, one profile-state document, one profile document.
//! let config_dir = layout.engine_config_dir(config_root, "mihomo");
//! assert!(config_dir.ends_with("engines/mihomo"));
//! assert!(layout.profile_state_file(&config_dir).ends_with("profile-state.toml"));
//!
//! // The per-profile document keeps the native extension of the injected
//! // codec, so the layout stays format neutral.
//! let document = layout.profile_document_path(&config_dir, "primary", "yaml");
//! assert!(document.ends_with("primary.yaml"));
//! ```

pub mod document_path;
pub mod document_store;
pub mod durable_replace;
pub mod file_lock;
pub mod instance_lock;
pub mod layout;
pub mod namespace;
pub mod owner_switch;
pub mod platform;
pub mod profile_files;
pub mod settings;
pub mod store;
#[cfg(test)]
mod test_core;
pub mod wide;
pub mod workspace;
pub mod workspace_store;
