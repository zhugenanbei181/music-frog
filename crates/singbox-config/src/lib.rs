//! Product-owned sing-box profile transaction.
//!
//! The neutral engine codec, native `check` hook and exact config-file readback
//! live in `singbox-driver`. This crate keeps only the product-level workspace
//! CAS transaction that binds that codec to a `ProfileStore` and the Core
//! lifecycle owner.

pub mod profile_configuration_transaction;
pub mod profile_document_codec;
