//! Rule-provider unpack and local cache maintenance facts (DUAL-11-06/07).
//!
//! Both surfaces must agree on where an unpacked provider's rules really came
//! from and on what a cache purge actually removed. These are observed facts,
//! never optimistic UI claims: a purge reports the files the host deleted and
//! the bytes it freed, and an unpack names the exact source it read.

use serde::{Deserialize, Serialize};

/// Where one provider's real rule list was read from.
///
/// Every surface renders the origin it was handed; no surface invents a
/// provider payload when none of these sources produced one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderContentOrigin {
    /// `payload:` declared inline in the active profile's `rule-providers`.
    InlinePayload,
    /// The declaration's own local file (`type: file` or an explicit `path`).
    DeclaredFile,
    /// The kernel's downloaded provider cache (`<home>/rules/<md5(url)>`).
    KernelCacheFile,
    /// The running controller published the provider payload.
    ControllerPayload,
}

impl ProviderContentOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InlinePayload => "inline-payload",
            Self::DeclaredFile => "declared-file",
            Self::KernelCacheFile => "kernel-cache-file",
            Self::ControllerPayload => "controller-payload",
        }
    }
}

/// Honest result of one rule-provider cache purge.
///
/// The counts are the host's observed filesystem result; a host that removed
/// nothing reports zero rather than claiming a clean-up that never happened.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCachePurge {
    /// Cache directory the host purged from, when it has one.
    pub directory: Option<String>,
    pub files_removed: usize,
    pub bytes_freed: u64,
}

impl ProviderCachePurge {
    pub fn is_noop(&self) -> bool {
        self.files_removed == 0 && self.bytes_freed == 0
    }
}

/// Availability of the host's rule-provider cache location.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleProviderCacheState {
    /// The host has not reported its cache location yet.
    #[default]
    Unknown,
    /// The host exposes a cache directory; `file_count`/`total_bytes` are real.
    Ready,
    /// The host exposes a cache directory and it currently holds no files.
    Empty,
    /// The host has no rule-provider cache location at all.
    Unsupported,
    /// Reading the cache location failed; `failure` carries the reason.
    Failed,
}

impl RuleProviderCacheState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Ready => "ready",
            Self::Empty => "empty",
            Self::Unsupported => "unsupported",
            Self::Failed => "failed",
        }
    }
}

/// The observed rule-provider cache location, published to both surfaces.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleProviderCacheSnapshot {
    pub state: RuleProviderCacheState,
    /// Absolute path of the kernel's provider cache directory.
    pub directory: Option<String>,
    /// Regular files currently cached (never directories).
    pub file_count: usize,
    /// Sum of the cached files' lengths in bytes.
    pub total_bytes: u64,
    pub failure: Option<String>,
}

impl RuleProviderCacheSnapshot {
    pub fn ready(directory: impl Into<String>, file_count: usize, total_bytes: u64) -> Self {
        let state = if file_count == 0 {
            RuleProviderCacheState::Empty
        } else {
            RuleProviderCacheState::Ready
        };
        Self {
            state,
            directory: Some(directory.into()),
            file_count,
            total_bytes,
            failure: None,
        }
    }

    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self {
            state: RuleProviderCacheState::Unsupported,
            directory: None,
            file_count: 0,
            total_bytes: 0,
            failure: Some(reason.into()),
        }
    }

    pub fn failed(reason: impl Into<String>) -> Self {
        Self {
            state: RuleProviderCacheState::Failed,
            directory: None,
            file_count: 0,
            total_bytes: 0,
            failure: Some(reason.into()),
        }
    }

    /// Whether the host exposes a real cache location.
    pub fn is_available(&self) -> bool {
        matches!(
            self.state,
            RuleProviderCacheState::Ready | RuleProviderCacheState::Empty
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_classifies_empty_and_ready_from_observed_counts() {
        let empty =
            RuleProviderCacheSnapshot::ready("/home/u/.config/mihomo-rs/configs/rules", 0, 0);
        assert_eq!(empty.state, RuleProviderCacheState::Empty);
        assert!(empty.is_available());
        let ready =
            RuleProviderCacheSnapshot::ready("/home/u/.config/mihomo-rs/configs/rules", 3, 4096);
        assert_eq!(ready.state, RuleProviderCacheState::Ready);
        assert_eq!(ready.file_count, 3);
        assert_eq!(ready.total_bytes, 4096);
        assert!(ready.is_available());
    }

    #[test]
    fn unsupported_and_failed_are_not_available() {
        let unsupported = RuleProviderCacheSnapshot::unsupported("no cache dir");
        assert_eq!(unsupported.state, RuleProviderCacheState::Unsupported);
        assert!(!unsupported.is_available());
        let failed = RuleProviderCacheSnapshot::failed("permission denied");
        assert_eq!(failed.state, RuleProviderCacheState::Failed);
        assert!(!failed.is_available());
        assert_eq!(
            RuleProviderCacheSnapshot::default().state,
            RuleProviderCacheState::Unknown
        );
        assert!(!RuleProviderCacheSnapshot::default().is_available());
    }

    #[test]
    fn purge_reports_real_counts_and_noop() {
        let purged = ProviderCachePurge {
            directory: Some("/tmp/rules".to_owned()),
            files_removed: 4,
            bytes_freed: 8192,
        };
        assert!(!purged.is_noop());
        assert!(ProviderCachePurge::default().is_noop());
    }

    #[test]
    fn origins_are_stable_tokens() {
        assert_eq!(
            ProviderContentOrigin::InlinePayload.as_str(),
            "inline-payload"
        );
        assert_eq!(
            ProviderContentOrigin::DeclaredFile.as_str(),
            "declared-file"
        );
        assert_eq!(
            ProviderContentOrigin::KernelCacheFile.as_str(),
            "kernel-cache-file"
        );
        assert_eq!(
            ProviderContentOrigin::ControllerPayload.as_str(),
            "controller-payload"
        );
        assert_eq!(RuleProviderCacheState::Ready.as_str(), "ready");
    }
}
