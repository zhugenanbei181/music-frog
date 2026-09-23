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

/// One real content fingerprint of a provider's local cache file.
///
/// This is a *file* observation, never an HTTP validator: the client sees the
/// bytes on disk, not the `ETag`/`If-None-Match` exchange the kernel performs
/// inside its own downloader.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderFileFingerprint {
    /// File length in bytes, as observed.
    pub size_bytes: u64,
    /// Lowercase hex SHA-256 of the file's bytes, as observed.
    pub sha256: String,
    /// Last-modified time in Unix seconds, when the host's filesystem exposes
    /// it. `None` means the host could not read a timestamp — never a guess.
    #[serde(default)]
    pub modified_unix_secs: Option<i64>,
}

impl ProviderFileFingerprint {
    /// Whether two fingerprints carry the same file content.
    ///
    /// The verdict is content-based: a timestamp that advanced while the bytes
    /// stayed identical (a re-download of unchanged content) is not reported
    /// as a content change.
    pub fn same_content(&self, other: &Self) -> bool {
        self.size_bytes == other.size_bytes && self.sha256 == other.sha256
    }
}

/// How a fresh fingerprint compares to the one this client observed before.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderFingerprintChange {
    /// This client has not observed a fingerprint for this provider yet.
    FirstSeen,
    /// Same size and same digest as the previous observation.
    Unchanged,
    /// Size or digest differs from the previous observation.
    Changed,
}

impl ProviderFingerprintChange {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FirstSeen => "first-seen",
            Self::Unchanged => "unchanged",
            Self::Changed => "changed",
        }
    }
}

/// DUAL-11-05: one client-visible observation of a provider's local cache file.
///
/// The observation deliberately carries the local facts only (`size`, SHA-256,
/// last-modified) and what changed between two observations by *this* client.
/// It is not an HTTP cache validator, and no surface may render it as a kernel
/// download that was skipped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCacheFingerprint {
    /// Name of the provider this file belongs to.
    pub provider: String,
    /// Absolute path of the observed file.
    pub path: String,
    pub change: ProviderFingerprintChange,
    pub current: ProviderFileFingerprint,
    /// The fingerprint this client observed previously, when it had one.
    #[serde(default)]
    pub previous: Option<ProviderFileFingerprint>,
}

impl ProviderCacheFingerprint {
    /// The stable comparison token both surfaces render.
    pub fn change_token(&self) -> &'static str {
        self.change.as_str()
    }

    /// Compare a fresh fingerprint against a previously observed one.
    pub fn compare(
        previous: Option<&ProviderFileFingerprint>,
        current: &ProviderFileFingerprint,
    ) -> ProviderFingerprintChange {
        match previous {
            None => ProviderFingerprintChange::FirstSeen,
            Some(previous) if previous.same_content(current) => {
                ProviderFingerprintChange::Unchanged
            }
            Some(_) => ProviderFingerprintChange::Changed,
        }
    }
}

/// DUAL-11-05: the kernel's real `etag-support` capability, as declared by the
/// active profile.
///
/// mihomo reads a **top-level** `etag-support` boolean (its `General.ETagSupport`,
/// whose `DefaultRawConfig` value is `true`) that gates its `ETag` /
/// `If-None-Match` conditional-request cache for downloaded resources, rule and
/// proxy providers included. The client can read the declaration; it cannot
/// read the per-request `304` outcome, which stays inside the kernel's
/// downloader and is never returned by the controller.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum KernelEtagSupportState {
    /// `etag-support: true` — the kernel performs conditional requests.
    Enabled,
    /// `etag-support: false` — conditional requests are turned off.
    Disabled,
    /// The active profile declares no `etag-support` key (mihomo's own default
    /// then applies). Never rendered as an explicit on/off claim.
    #[default]
    NotDeclared,
}

impl KernelEtagSupportState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::NotDeclared => "not-declared",
        }
    }
}

/// DUAL-11-05: the kernel's declared `etag-support` capability for one read.
///
/// This is a *declaration* fact read from the active profile, not an HTTP
/// cache-validator result: `declared` is the raw boolean the profile carried,
/// and `state` is the shared classification both surfaces render.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelEtagSupportSnapshot {
    pub state: KernelEtagSupportState,
    /// The raw boolean declared under the profile's top-level `etag-support`
    /// key, when present. `None` means the key was absent (state is
    /// `NotDeclared`), so no value is invented.
    #[serde(default)]
    pub declared: Option<bool>,
}

impl KernelEtagSupportSnapshot {
    /// Classify the raw `etag-support` declaration read from the profile.
    pub fn from_declared(declared: Option<bool>) -> Self {
        let state = match declared {
            Some(true) => KernelEtagSupportState::Enabled,
            Some(false) => KernelEtagSupportState::Disabled,
            None => KernelEtagSupportState::NotDeclared,
        };
        Self { state, declared }
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

    #[test]
    fn etag_support_classifies_the_raw_declaration_without_inventing_one() {
        let enabled = KernelEtagSupportSnapshot::from_declared(Some(true));
        assert_eq!(enabled.state, KernelEtagSupportState::Enabled);
        assert_eq!(enabled.declared, Some(true));
        assert_eq!(enabled.state.as_str(), "enabled");

        let disabled = KernelEtagSupportSnapshot::from_declared(Some(false));
        assert_eq!(disabled.state, KernelEtagSupportState::Disabled);
        assert_eq!(disabled.declared, Some(false));
        assert_eq!(disabled.state.as_str(), "disabled");

        // No `etag-support` key: the state is NotDeclared and the raw value is
        // absent. The kernel's own default is never published as a declaration.
        let absent = KernelEtagSupportSnapshot::from_declared(None);
        assert_eq!(absent.state, KernelEtagSupportState::NotDeclared);
        assert_eq!(absent.declared, None);
        assert_eq!(absent.state.as_str(), "not-declared");
        assert_eq!(
            KernelEtagSupportSnapshot::default().state,
            KernelEtagSupportState::NotDeclared
        );

        let json = serde_json::to_value(absent).expect("serialise");
        assert_eq!(json["state"], "NotDeclared");
        assert!(json["declared"].is_null());
        let restored: KernelEtagSupportSnapshot =
            serde_json::from_value(json).expect("deserialise");
        assert_eq!(restored, absent);
    }

    fn fingerprint(
        size_bytes: u64,
        sha256: &str,
        modified: Option<i64>,
    ) -> ProviderFileFingerprint {
        ProviderFileFingerprint {
            size_bytes,
            sha256: sha256.to_owned(),
            modified_unix_secs: modified,
        }
    }

    #[test]
    fn fingerprint_comparison_is_content_based_and_tokenised() {
        let first = fingerprint(1_024, "aa", Some(1_700_000_000));
        assert_eq!(
            ProviderCacheFingerprint::compare(None, &first),
            ProviderFingerprintChange::FirstSeen
        );
        assert_eq!(
            ProviderCacheFingerprint::compare(Some(&first), &first),
            ProviderFingerprintChange::Unchanged
        );
        // A re-download of identical bytes moves the timestamp only: the
        // content verdict stays unchanged instead of inventing a rewrite.
        let retimed = fingerprint(1_024, "aa", Some(1_700_000_900));
        assert_eq!(
            ProviderCacheFingerprint::compare(Some(&first), &retimed),
            ProviderFingerprintChange::Unchanged
        );
        let resized = fingerprint(2_048, "aa", Some(1_700_000_000));
        assert_eq!(
            ProviderCacheFingerprint::compare(Some(&first), &resized),
            ProviderFingerprintChange::Changed
        );
        let rewritten = fingerprint(1_024, "bb", Some(1_700_000_000));
        assert_eq!(
            ProviderCacheFingerprint::compare(Some(&first), &rewritten),
            ProviderFingerprintChange::Changed
        );
        assert_eq!(ProviderFingerprintChange::FirstSeen.as_str(), "first-seen");
        assert_eq!(ProviderFingerprintChange::Unchanged.as_str(), "unchanged");
        assert_eq!(ProviderFingerprintChange::Changed.as_str(), "changed");
    }

    #[test]
    fn fingerprint_observation_serialises_its_source_and_change() {
        let observation = ProviderCacheFingerprint {
            provider: "ads".to_owned(),
            path: "/home/u/.config/mihomo-rs/rules/8f14e45fceea167a5a36dedd4bea2543".to_owned(),
            change: ProviderFingerprintChange::Changed,
            current: fingerprint(4_096, "cc", Some(1_700_000_100)),
            previous: Some(fingerprint(2_048, "bb", Some(1_699_999_000))),
        };
        assert_eq!(observation.change_token(), "changed");
        let json = serde_json::to_value(&observation).expect("serialise");
        assert_eq!(json["change"], "Changed");
        assert_eq!(json["current"]["sha256"], "cc");
        assert_eq!(json["previous"]["size_bytes"], 2_048);
        let restored: ProviderCacheFingerprint = serde_json::from_value(json).expect("deserialise");
        assert_eq!(restored, observation);
    }

    #[test]
    fn fingerprint_observation_omits_a_previous_value_and_tolerates_missing_mtime() {
        let observation = ProviderCacheFingerprint {
            provider: "geo".to_owned(),
            path: "/cache/rules/deadbeef".to_owned(),
            change: ProviderFingerprintChange::FirstSeen,
            current: fingerprint(12, "dd", None),
            previous: None,
        };
        let json = serde_json::to_value(&observation).expect("serialise");
        assert!(json.get("previous").is_none() || json["previous"].is_null());
        assert!(json["current"]["modified_unix_secs"].is_null());
        assert!(observation.current.modified_unix_secs.is_none());
    }
}
