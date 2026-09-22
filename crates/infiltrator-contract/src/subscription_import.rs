//! Shared contract for subscription lifecycle: multi-channel imports,
//! conditional requests, and update reports.

use serde::{Deserialize, Serialize};

/// Import channel for adding a subscription or local configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionImportChannel {
    /// Remote HTTP/HTTPS URL.
    Url,
    /// Local filesystem file path (*.yaml, *.yml, *.json, *.txt).
    LocalFile,
    /// Raw text / URL from system clipboard.
    Clipboard,
}

/// Recognized subscription payload format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionFormat {
    ClashYaml,
    Base64VmessVless,
    ShadowsocksUri,
    TrojanUri,
    SingBoxJson,
    Unknown,
}

/// Draft parameters submitted when importing a subscription.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionImportDraft {
    pub channel: SubscriptionImportChannel,
    pub name: String,
    pub source: String,
    pub activate_after_import: bool,
    pub custom_user_agent: Option<String>,
    pub insecure_skip_verify: bool,
    pub auto_reload_core: bool,
    pub cron_expression: Option<String>,
    pub update_interval_hours: Option<u32>,
}

impl SubscriptionImportDraft {
    pub fn new(
        channel: SubscriptionImportChannel,
        name: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            channel,
            name: name.into(),
            source: source.into(),
            activate_after_import: false,
            custom_user_agent: None,
            insecure_skip_verify: false,
            auto_reload_core: true,
            cron_expression: None,
            update_interval_hours: Some(24),
        }
    }
}

/// Import status of a subscription import operation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionImportStatus {
    #[default]
    Idle,
    Validating,
    Importing,
    Success,
    Failed,
}

/// Read model snapshot representing the current import workbench state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SubscriptionImportSnapshot {
    pub channel: Option<SubscriptionImportChannel>,
    pub status: SubscriptionImportStatus,
    pub suggested_name: Option<String>,
    pub detected_format: Option<SubscriptionFormat>,
    pub detected_node_count: usize,
    pub last_error: Option<String>,
    pub progress_percent: Option<f32>,
}

/// Outcome of an individual subscription update cycle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionUpdateOutcome {
    /// 200 OK: New content downloaded, validated, and committed.
    Updated { new_bytes: usize, node_count: usize },
    /// 304 Not Modified: Server confirmed configuration is unchanged. Zero traffic used.
    NotModified {
        etag: Option<String>,
        last_modified: Option<String>,
    },
    /// Update failed after network retries.
    Failed { error: String, attempts: usize },
}

/// Traffic metadata advertised via `subscription-userinfo` header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionQuotaFacts {
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub total_bytes: u64,
    pub expire_at_unix: Option<i64>,
}

/// Comprehensive report emitted after a subscription update attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionUpdateReport {
    pub profile_name: String,
    pub outcome: SubscriptionUpdateOutcome,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub quota: Option<SubscriptionQuotaFacts>,
    pub usage_warning: bool,
    pub expiry_warning: bool,
    pub reloaded_core: bool,
    pub backed_up: bool,
}

/// Batch update summary for all configured subscriptions.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionBatchReport {
    pub total: usize,
    pub updated: usize,
    pub not_modified: usize,
    pub failed: usize,
    pub skipped: usize,
    pub outcomes: Vec<SubscriptionUpdateReport>,
}

/// DUAL-07-01: outcome of importing one document through a specific channel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionImportReport {
    pub profile_name: String,
    pub channel: SubscriptionImportChannel,
    pub format: SubscriptionFormat,
    pub node_count: usize,
    pub content_bytes: usize,
}

/// Deduplication strategy a surface can pick in the filter editor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionFilterDedup {
    #[default]
    Disabled,
    KeepFirst,
    KeepLast,
    AppendIndex,
}

/// DUAL-07-08: contract mirror of the per-profile node-keyword filter a
/// surface edits and submits through the shared command bus.
///
/// The domain owns the runtime `FilterRule`; this draft only carries the
/// surface-editable fields as free text (comma/newline separated keywords, a
/// `pattern => replacement` rename list and a dedupe index). It stays
/// serializable so it can ride both the command bus and the surface snapshot.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", default)]
pub struct SubscriptionFilterDraft {
    pub include: String,
    pub exclude: String,
    pub exclude_types: String,
    pub renames: String,
    pub dedup_index: usize,
}

impl SubscriptionFilterDraft {
    /// True when the draft would not reshape the document, so the surfaces can
    /// render an "inactive" state and the pipeline can be skipped.
    pub fn is_empty(&self) -> bool {
        [
            &self.include,
            &self.exclude,
            &self.exclude_types,
            &self.renames,
        ]
        .iter()
        .all(|value| value.trim().is_empty())
            && self.dedup_index == 0
    }
}
