#![allow(
    clippy::bool_comparison,
    clippy::derivable_impls,
    clippy::collapsible_if,
    clippy::items_after_test_module
)]
//! Engine-neutral profile-store kernel.
//!
//! This crate owns the persistence machinery that is identical for every
//! backend, platform and UI: the on-disk layout description, durable atomic
//! document replacement, cross-process write fences, the profile-state TOML
//! schema and one engine-scoped store identity. It carries no credential,
//! subscription, Age/keyring, product path literal or UI dependency; product
//! crates inject those through [`layout::ProfileStoreLayout`] and their own
//! secure stores.

pub mod document_store;
pub mod durable_replace;
pub mod file_lock;
pub mod instance_lock;
pub mod layout;
pub mod namespace;
pub mod settings;
pub mod store;
#[cfg(any(windows, test))]
pub mod wide;
