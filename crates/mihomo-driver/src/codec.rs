//! Mihomo native YAML codec, editor snippet splicing and the native `-t`
//! configuration dry-run hook.
//!
//! The neutral half of the codec only parses and validates exact document
//! bytes; it never reads the profile store or contacts a running core. The
//! native `-t` check drives the process command boundary and therefore shares
//! the `process` feature slice.

pub mod config;
pub mod profile_document_codec;
pub mod snippet;
