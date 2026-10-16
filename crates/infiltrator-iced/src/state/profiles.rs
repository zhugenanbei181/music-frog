//! Profiles owner.

use crate::types::app::{SyncConflict, SyncProgress};
use crate::types::options::{EncryptedBackupState, SyncDiffState};
use infiltrator_contract::aggregator::{
    AggregationCustomGroup, AggregationReport, AggregationTemplate,
};
use infiltrator_domain::profiles::ProfileInfo;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Loading phase for the profiles list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProfileListLoadPhase {
    #[default]
    Idle,
    Loading,
}

impl ProfileListLoadPhase {
    pub const fn is_loading(self) -> bool {
        matches!(self, Self::Loading)
    }
}

/// Import operation phase for remote URL or local file subscription import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProfileImportPhase {
    #[default]
    Idle,
    Importing,
}

impl ProfileImportPhase {
    pub const fn is_importing(self) -> bool {
        matches!(self, Self::Importing)
    }
}

/// Save phase for subscription schedule and fetch settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubscriptionSavePhase {
    #[default]
    Idle,
    Saving,
}

impl SubscriptionSavePhase {
    pub const fn is_saving(self) -> bool {
        matches!(self, Self::Saving)
    }
}

/// In-flight update phase for refreshing subscription profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubscriptionUpdatePhase {
    #[default]
    Idle,
    Updating,
}

impl SubscriptionUpdatePhase {
    pub const fn is_updating(self) -> bool {
        matches!(self, Self::Updating)
    }
}

/// WebDAV synchronization operation phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WebDavSyncPhase {
    #[default]
    Idle,
    Syncing,
}

impl WebDavSyncPhase {
    pub const fn is_syncing(self) -> bool {
        matches!(self, Self::Syncing)
    }
}

/// WebDAV credential and connection testing phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WebDavTestPhase {
    #[default]
    Idle,
    Testing,
}

impl WebDavTestPhase {
    pub const fn is_testing(self) -> bool {
        matches!(self, Self::Testing)
    }
}

/// Save phase for application-level settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppSettingsSavePhase {
    #[default]
    Idle,
    Saving,
}

impl AppSettingsSavePhase {
    pub const fn is_saving(self) -> bool {
        matches!(self, Self::Saving)
    }
}

/// Load phase for WebDAV sync diff comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyncDiffLoadPhase {
    #[default]
    Idle,
    Loading,
}

impl SyncDiffLoadPhase {
    pub const fn is_loading(self) -> bool {
        matches!(self, Self::Loading)
    }
}

/// Apply phase for resolving and committing WebDAV sync diffs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyncDiffApplyPhase {
    #[default]
    Idle,
    Applying,
}

impl SyncDiffApplyPhase {
    pub const fn is_applying(self) -> bool {
        matches!(self, Self::Applying)
    }
}

/// Aggregation computation phase for template creation and preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProfileAggregatorPhase {
    #[default]
    Idle,
    Aggregating,
}

impl ProfileAggregatorPhase {
    pub const fn is_aggregating(self) -> bool {
        matches!(self, Self::Aggregating)
    }
}

/// 订阅与档案域:Profile 列表、订阅导入/更新、WebDAV 同步与应用设置保存(UI-002)。
pub struct ProfileState {
    pub profiles: Vec<ProfileInfo>,
    pub profiles_filter: String,
    pub list_load_phase: ProfileListLoadPhase,
    pub import_url: String,
    pub import_name: String,
    pub import_activate: bool,
    pub url_import_phase: ProfileImportPhase,
    pub local_import_path: String,
    pub local_import_name: String,
    pub local_import_activate: bool,
    pub local_import_phase: ProfileImportPhase,
    pub subscription_profile_name: String,
    pub subscription_url: String,
    pub subscription_auto_update_enabled: bool,
    pub subscription_update_interval_hours: String,
    /// DUAL-07-03: optional cron expression for the selected profile.
    pub subscription_cron_expression: String,
    pub subscription_user_agent: String,
    pub subscription_insecure_skip_verify: bool,
    /// DUAL-07-09: reload the core after a successful update of this profile.
    pub subscription_auto_reload_core: bool,
    pub subscription_save_phase: SubscriptionSavePhase,
    pub subscription_update_phase: SubscriptionUpdatePhase,
    pub webdav_url: String,
    pub webdav_user: String,
    pub webdav_pass: String,
    pub webdav_enabled: bool,
    pub webdav_sync_interval_mins: String,
    pub webdav_sync_on_startup: bool,
    pub webdav_sync_phase: WebDavSyncPhase,
    pub sync_progress: Option<SyncProgress>,
    pub sync_conflicts: Vec<SyncConflict>,
    pub webdav_test_phase: WebDavTestPhase,
    pub sync_cancel: Option<Arc<AtomicBool>>,
    /// 0.20: 周期同步标记 —— 只有 `TickWebDavSync` 发起的同步链才发系统通知
    /// （手动上传/下载不发）。TickWebDavSync 置位，SyncFinished 处理后清除。
    pub sync_from_tick: bool,
    pub app_settings_save_phase: AppSettingsSavePhase,
    pub restart_after_profile_reset: bool,
    pub sync_diff: Option<SyncDiffState>,
    pub sync_diff_load_phase: SyncDiffLoadPhase,
    pub sync_diff_apply_phase: SyncDiffApplyPhase,
    pub aggregator_modal_open: bool,
    pub aggregator_selected_profiles: Vec<String>,
    pub aggregator_name_input: String,
    /// DUAL-08: the last shared aggregation preview (never built locally).
    pub aggregator_report: Option<AggregationReport>,
    /// DUAL-08-02: drop fingerprint-identical nodes across sources.
    pub aggregator_deduplicate: bool,
    /// DUAL-08-03: normalise names so geo clustering can bucket them.
    pub aggregator_geo_cluster: bool,
    /// DUAL-08-04/08-05: synthesize region url-test groups + master cascade.
    pub aggregator_generate_groups: bool,
    /// Strip emoji characters from node names before grouping.
    pub aggregator_remove_emojis: bool,
    /// DUAL-08-09: drop nodes failing the required-field precheck.
    pub aggregator_availability_precheck: bool,
    /// DUAL-08-12: make the generated profile the active profile (and hot
    /// reload the kernel when the host composes a Core-owner profile port).
    pub aggregator_activate_after_create: bool,
    /// DUAL-08-08: free-text regex rename rules (`模式 => 替换`).
    pub aggregator_renames: String,
    /// DUAL-08-10: custom group name being typed.
    pub aggregator_custom_name: String,
    /// DUAL-08-10: custom group member keywords being typed.
    pub aggregator_custom_keywords: String,
    /// DUAL-08-10: appended custom groups awaiting the next preview/save.
    pub aggregator_custom_groups: Vec<AggregationCustomGroup>,
    /// DUAL-08-13: persisted template library (never built locally).
    pub aggregator_templates: Vec<AggregationTemplate>,
    /// DUAL-08-13: name typed for the "save as template" action.
    pub aggregator_template_name: String,
    pub aggregator_phase: ProfileAggregatorPhase,
    pub encrypted_backup: EncryptedBackupState,
}
