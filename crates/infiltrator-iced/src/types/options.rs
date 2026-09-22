//! Profile-options and merge-domain types shared by the Iced handlers:
//! the mixin editor pane, the subscription filter draft, MRS rule-provider
//! details and the sync-conflict key-level diff state.

use infiltrator_domain::mrs::MrsMetadata;
use std::collections::HashMap;
use std::path::PathBuf;

/// Which document the Editor page is showing for the opened profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorPane {
    #[default]
    Profile,
    Mixin,
    Filter,
    Script,
}

/// Parsed MRS header details for one rule provider, paired with the source
/// file so the view can show both the metadata and the failure cause.
#[derive(Debug, Clone)]
pub struct MrsProviderDetail {
    pub name: String,
    pub behavior: String,
    pub file: Option<PathBuf>,
    pub metadata: Option<MrsMetadata>,
    pub errors: Vec<String>,
}

impl MrsProviderDetail {
    /// Short summary chip used by the providers list: `MRS v1 · 1234 rules`.
    pub fn summary(&self) -> String {
        match &self.metadata {
            Some(meta) => format!(
                "MRS v{} · {} rules · {:?}",
                meta.version, meta.rule_count, meta.behavior
            ),
            None => self
                .errors
                .first()
                .cloned()
                .unwrap_or_else(|| "未解析到 MRS 元数据".to_string()),
        }
    }
}

/// Computed local-vs-remote top-level diff for one sync conflict, fed by
/// `infiltrator_domain::sync::diff_yaml_configs` and rendered as
/// per-key pick rows on the Sync page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncDiffBundle {
    pub profile: String,
    pub remote_path: PathBuf,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<(String, String, String)>,
}

impl SyncDiffBundle {
    /// Every key that participates in the merge decision.
    pub fn all_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = Vec::new();
        keys.extend(self.added.iter().cloned());
        keys.extend(self.removed.iter().cloned());
        keys.extend(self.modified.iter().map(|(key, _, _)| key.clone()));
        keys
    }

    pub fn key_entries(&self) -> Vec<(String, SyncDiffKeyKind)> {
        let mut entries: Vec<(String, SyncDiffKeyKind)> = Vec::new();
        for key in &self.added {
            entries.push((key.clone(), SyncDiffKeyKind::Added));
        }
        for key in &self.removed {
            entries.push((key.clone(), SyncDiffKeyKind::Removed));
        }
        for (key, _, _) in &self.modified {
            entries.push((key.clone(), SyncDiffKeyKind::Modified));
        }
        entries
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDiffKeyKind {
    Added,
    Removed,
    Modified,
}

impl SyncDiffKeyKind {
    /// Locale key for the badge label; views resolve it via `Localizer`.
    pub fn label_key(self) -> &'static str {
        match self {
            SyncDiffKeyKind::Added => "diff_kind_added",
            SyncDiffKeyKind::Removed => "diff_kind_removed",
            SyncDiffKeyKind::Modified => "diff_kind_modified",
        }
    }
}

/// Open merge session: the computed diff plus the per-key decision
/// (`true` = adopt the remote value, `false` = keep local).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncDiffState {
    pub bundle: SyncDiffBundle,
    pub picks: HashMap<String, bool>,
}

impl SyncDiffState {
    pub fn new(bundle: SyncDiffBundle) -> Self {
        let picks = bundle
            .all_keys()
            .into_iter()
            .map(|key| (key, false))
            .collect();
        Self { bundle, picks }
    }
}

/// State for AES-256 encrypted configuration backup (.encpkg) export and import.
#[derive(Debug, Clone, Default)]
pub struct EncryptedBackupState {
    pub passphrase: String,
    pub last_exported_path: Option<String>,
    pub is_processing: bool,
}

/// State for subscription quota monitoring, expiry alerts, and cron scheduling.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct QuotaScheduleState {
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub remaining_percent: f64,
    pub expiry_timestamp: Option<u64>,
    pub warning_tier: String,
    pub cron_interval_hours: u32,
}
