//! Encrypted backup export, verification, local/WebDAV delivery, and scoped restore contract (FLPARITY-013).
//!
//! Establishes the authoritative single-owner model for versioned backup bundles,
//! authenticated encryption verification, local/WebDAV delivery targets, and
//! `ConfigOnly` vs `FullData` restore strategies (FlClash v0.9.2 parity).

use serde::{Deserialize, Serialize};

/// Supported delivery targets for encrypted backup bundles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackupDeliveryTarget {
    /// Local file system package (`.encpkg`).
    #[default]
    LocalFile,
    /// Remote WebDAV storage endpoint.
    WebDav,
}

impl BackupDeliveryTarget {
    /// Localized Chinese label.
    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::LocalFile => "本地加密归档",
            Self::WebDav => "WebDAV 云端归档",
        }
    }

    /// Localized English label.
    pub const fn label_en(self) -> &'static str {
        match self {
            Self::LocalFile => "Local Encrypted Archive",
            Self::WebDav => "WebDAV Cloud Archive",
        }
    }
}

/// Restore scope strategy matching FlClash v0.9.2 (`ConfigOnly` vs `FullData`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackupRestoreScope {
    /// Restore application settings and global mixin/override rules only; keep existing local profiles.
    ConfigOnly,
    /// Restore full data including settings, mixin/override rules, and all profile documents.
    #[default]
    FullData,
}

impl BackupRestoreScope {
    /// Localized Chinese label.
    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::ConfigOnly => "仅恢复应用配置",
            Self::FullData => "恢复全部配置与订阅数据",
        }
    }

    /// Localized English label.
    pub const fn label_en(self) -> &'static str {
        match self {
            Self::ConfigOnly => "Restore Configuration Only",
            Self::FullData => "Restore Full Data & Profiles",
        }
    }

    /// Predicate indicating whether profile documents are included in this restore scope.
    pub const fn includes_profiles(self) -> bool {
        matches!(self, Self::FullData)
    }
}

/// Typed verification or delivery failure reasons for encrypted backup operations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupVerificationFailure {
    /// Passphrase does not meet the minimum length requirement.
    PassphraseTooShort { min_len: usize },
    /// Decryption failed due to wrong passphrase or AEAD authentication tag mismatch.
    WrongPassphraseOrAuthTag,
    /// Bundle framing or JSON structure is corrupted or truncated.
    CorruptedPayload { detail: String },
    /// Bundle major schema version is incompatible with this client.
    IncompatibleSchemaVersion {
        bundle_version: String,
        supported_major: u32,
    },
    /// Decrypted payload SHA-256 digest does not match the manifest digest.
    IntegrityDigestMismatch {
        expected_sha256: String,
        actual_sha256: String,
    },
    /// Target read/write transport failed.
    DeliveryFailed {
        target: BackupDeliveryTarget,
        detail: String,
    },
    /// Restore transaction failed mid-flight and rolled back without mutating live state.
    PartialRestoreAborted { stage: String, detail: String },
}

impl BackupVerificationFailure {
    /// Human-readable Chinese diagnostic summary.
    pub fn summary_zh(&self) -> String {
        match self {
            Self::PassphraseTooShort { min_len } => {
                format!("加密口令长度不足（至少需要 {min_len} 个字符）")
            }
            Self::WrongPassphraseOrAuthTag => {
                "解密失败：口令错误或加密认证标签校验未通过".to_string()
            }
            Self::CorruptedPayload { detail } => format!("备份归档已损坏或格式无效：{detail}"),
            Self::IncompatibleSchemaVersion {
                bundle_version,
                supported_major,
            } => format!(
                "备份归档版本不兼容：归档版本 {bundle_version}（当前支持主版本 v{supported_major}）"
            ),
            Self::IntegrityDigestMismatch {
                expected_sha256,
                actual_sha256,
            } => format!("完整性摘要校验失败：期望 {expected_sha256}，实际 {actual_sha256}"),
            Self::DeliveryFailed { target, detail } => {
                format!("{}传输失败：{detail}", target.label_zh())
            }
            Self::PartialRestoreAborted { stage, detail } => {
                format!("恢复在阶段 [{stage}] 中止并已原子回滚：{detail}")
            }
        }
    }
}

/// Authoritative receipt for the most recent backup or restore operation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackupOperationReceipt {
    /// No backup or restore operation has been executed in this session.
    #[default]
    Idle,
    /// Encrypted backup bundle was verified and exported to the target destination.
    ExportSucceeded {
        target: BackupDeliveryTarget,
        destination: String,
        bundle_version: String,
        sha256_digest: String,
        profile_count: usize,
        exported_at_unix_ms: u64,
    },
    /// Encrypted backup bundle was decrypted, verified, and atomically restored.
    RestoreSucceeded {
        target: BackupDeliveryTarget,
        scope: BackupRestoreScope,
        restored_profiles: usize,
        restored_at_unix_ms: u64,
    },
    /// Operation failed verification or delivery; existing state remained untouched.
    Failed {
        failure: BackupVerificationFailure,
        attempted_at_unix_ms: u64,
    },
}

impl BackupOperationReceipt {
    /// Semantic badge tone for UI surfaces.
    pub const fn badge_tone(&self) -> &'static str {
        match self {
            Self::Idle => "neutral",
            Self::ExportSucceeded { .. } | Self::RestoreSucceeded { .. } => "success",
            Self::Failed { .. } => "danger",
        }
    }
}

/// Versioned metadata summary of a verified backup bundle.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BackupBundleHeader {
    /// Semantic schema version (e.g. `"1.0.0"`).
    pub schema_version: String,
    /// Creation timestamp in Unix milliseconds.
    pub created_at_unix_ms: u64,
    /// Client version that produced the bundle.
    pub client_version: String,
    /// Hex-encoded SHA-256 digest of the canonical payload.
    pub sha256_digest: String,
    /// Number of profile documents stored inside the bundle.
    pub profile_count: usize,
}

/// Read-model snapshot of the encrypted backup and restore owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupRestoreSnapshot {
    /// Monotonic state revision.
    pub revision: u64,
    /// Currently selected delivery target.
    pub selected_target: BackupDeliveryTarget,
    /// Currently selected restore scope.
    pub selected_scope: BackupRestoreScope,
    /// Last operation receipt.
    pub last_receipt: BackupOperationReceipt,
    /// Known verified bundle headers available for restore.
    pub available_bundles: Vec<BackupBundleHeader>,
}

impl Default for BackupRestoreSnapshot {
    fn default() -> Self {
        Self {
            revision: 0,
            selected_target: BackupDeliveryTarget::LocalFile,
            selected_scope: BackupRestoreScope::FullData,
            last_receipt: BackupOperationReceipt::Idle,
            available_bundles: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_and_scope_labels_and_predicates_are_consistent() {
        assert_eq!(BackupDeliveryTarget::LocalFile.label_zh(), "本地加密归档");
        assert_eq!(
            BackupDeliveryTarget::WebDav.label_en(),
            "WebDAV Cloud Archive"
        );
        assert!(!BackupRestoreScope::ConfigOnly.includes_profiles());
        assert!(BackupRestoreScope::FullData.includes_profiles());
    }

    #[test]
    fn failure_summary_and_receipt_tones_cover_all_variants() {
        let short = BackupVerificationFailure::PassphraseTooShort { min_len: 6 };
        assert!(short.summary_zh().contains("6"));
        let incompatible = BackupVerificationFailure::IncompatibleSchemaVersion {
            bundle_version: "2.1.0".to_string(),
            supported_major: 1,
        };
        assert!(incompatible.summary_zh().contains("2.1.0"));

        let idle = BackupOperationReceipt::Idle;
        assert_eq!(idle.badge_tone(), "neutral");
        let failed = BackupOperationReceipt::Failed {
            failure: BackupVerificationFailure::WrongPassphraseOrAuthTag,
            attempted_at_unix_ms: 100,
        };
        assert_eq!(failed.badge_tone(), "danger");
    }
}
