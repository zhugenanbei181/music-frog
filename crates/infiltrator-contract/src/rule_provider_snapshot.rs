//! Applied rule-provider facts shared by all native products.
use crate::provider_cache::ProviderCacheFingerprint;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuleProviderSnapshot {
    pub name: String,
    pub rule_count: usize,
    pub behavior: String,
    pub updated_at: String,
    /// DUAL-11-04: the `rule-providers` source URL declared in the active
    /// profile, when the provider is config-backed (runtime-only providers
    /// honestly report `None` instead of a fabricated address).
    #[serde(default)]
    pub source_url: Option<String>,
    /// DUAL-11-05: the `interval` declared for this provider in the active
    /// profile, in seconds. The mihomo kernel owns the scheduled refresh (and
    /// the `ETag`/`If-None-Match` conditional cache behind it); the client only
    /// publishes the declared schedule, never a fabricated cache hit/miss.
    #[serde(default)]
    pub refresh_interval_secs: Option<u64>,
    /// DUAL-11-05: the client's own observation of this provider's local cache
    /// file (size + SHA-256 + last-modified, compared against the previous
    /// observation). `None` means this client has no local file to fingerprint;
    /// it is never a claim about the kernel's HTTP validator.
    #[serde(default)]
    pub cache_fingerprint: Option<ProviderCacheFingerprint>,
}
