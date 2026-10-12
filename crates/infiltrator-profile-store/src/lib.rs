//! Engine-neutral durable profile persistence.

pub mod active_reference_cas;
pub mod active_reference_file;
pub mod aggregation_templates;
mod document_path;
pub mod document_store;
pub mod file_lock;
pub mod metadata;
pub mod profile_catalog;
pub mod profile_files;
pub mod store;
pub mod workspace;
pub mod workspace_store;
