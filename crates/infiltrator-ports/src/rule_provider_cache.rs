//! Host port for the kernel's local rule-provider files (DUAL-11-06/07).
//!
//! The host owns the path layout (`-d <config dir>` plus mihomo's
//! `rules/<md5(url)>` cache naming); the application owns the policy. A host
//! without any provider cache location returns
//! [`infiltrator_ports::error::PortError::unsupported`] instead of pretending
//! it purged or read something.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::provider_cache::ProviderCachePurge;
use infiltrator_contract::provider_cache::ProviderContentOrigin;
use infiltrator_contract::provider_cache::RuleProviderCacheSnapshot;
use infiltrator_domain::rules::provider_store::RuleProviderDeclaration;
use std::path::PathBuf;

/// Raw provider bytes the host found on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCacheEntry {
    pub origin: ProviderContentOrigin,
    pub path: Option<PathBuf>,
    pub bytes: Vec<u8>,
}

/// One real provider file the host fingerprinted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderFileFact {
    pub path: PathBuf,
    pub fingerprint: infiltrator_contract::provider_cache::ProviderFileFingerprint,
}

/// Read + purge access to the kernel's cached rule-provider files.
#[async_trait]
pub trait RuleProviderCachePort: Send + Sync {
    /// Read one declared provider's local file, if the host has one.
    ///
    /// `Ok(None)` means "no file for this declaration" and is a normal answer;
    /// `Err` means the lookup itself failed.
    async fn read_provider(
        &self,
        declaration: &RuleProviderDeclaration,
    ) -> Result<Option<ProviderCacheEntry>, PortError>;

    /// Fingerprint the provider file the kernel downloaded for `declaration`.
    ///
    /// Only remote (`type: http`) providers have a downloaded file to observe;
    /// a declaration without one answers `Ok(None)`. The size, digest and
    /// last-modified time are read from the same file, and the digest is only
    /// recomputed when the file's length or timestamp no longer match the last
    /// file this host hashed — an unchanged-input shortcut, never a fabricated
    /// value for a file that moved.
    async fn fingerprint(
        &self,
        declaration: &RuleProviderDeclaration,
    ) -> Result<Option<ProviderFileFact>, PortError>;

    /// Delete cached rule-provider files only, reporting real counts.
    async fn purge(&self) -> Result<ProviderCachePurge, PortError>;

    /// Observed cache location fact (count + size), never a guess.
    async fn snapshot(&self) -> Result<RuleProviderCacheSnapshot, PortError>;
}
