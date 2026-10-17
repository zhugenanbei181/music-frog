#![allow(
    clippy::bool_comparison,
    clippy::derivable_impls,
    clippy::collapsible_if,
    clippy::ptr_arg,
    clippy::items_after_test_module
)]
//! Product profile persistence facade over the neutral `lurker-profile-store` kernel.
//!
//! Credentials, subscription records, catalog, overrides and templates stay
//! product-side; document I/O, backups, workspace publication and deletion
//! ordering are delegated to the kernel.

pub mod active_owner_lease;
pub mod active_owner_registry;
pub mod active_owner_switch;
mod active_profile_reference_port;
pub mod active_profile_selection_lease;
pub mod active_reference_cas;
pub mod active_reference_file;
pub mod active_reference_owner;
pub mod active_reference_owner_coordinator;
mod active_workspace_transaction;
pub mod aggregation_templates;
mod document_path;
pub mod layout;
pub mod metadata;
pub mod named_overrides;
pub mod namespace;
pub mod profile_age_credential;
pub mod profile_catalog;
pub mod profile_catalog_order;
pub mod profile_display_name;
pub mod profile_files;
pub mod profile_reset;
mod profile_store_port;
pub mod reset_fence;
pub mod store;
pub mod workspace;
pub mod workspace_store;
