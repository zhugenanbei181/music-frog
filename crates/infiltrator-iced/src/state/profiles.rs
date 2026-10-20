//! Profiles owner.

use crate::types::app::{SyncConflict, SyncProgress};
use crate::types::options::{EncryptedBackupState, SyncDiffState};
use infiltrator_contract::aggregator::{
    AggregationCustomGroup, AggregationReport, AggregationTemplate,
};
use infiltrator_domain::profiles::ProfileInfo;
use infiltrator_presentation::profiles_page_model::{ProfilesOperation, ProfilesPageModel};
use infiltrator_presentation::sync_page_model::SyncPageModel;
use lurker_core_contract::error::Failure;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// 订阅与档案域:Profile 列表、订阅导入/更新、WebDAV 同步与应用设置保存(UI-002)。
pub struct ProfileState {
    pub profiles: Vec<ProfileInfo>,
    /// The one page model of the profiles page: the list filter and the ten
    /// operation phases.
    pub profiles_model: ProfilesPageModel,
    /// The one page model of the sync page: the observation fold, the
    /// conflict projection and the two operation phases.
    pub sync_model: SyncPageModel,
    pub import_url: String,
    pub import_name: String,
    pub import_activate: bool,
    pub local_import_path: String,
    pub local_import_name: String,
    pub local_import_activate: bool,
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
    pub webdav_url: String,
    pub webdav_user: String,
    pub webdav_pass: String,
    pub webdav_enabled: bool,
    pub webdav_sync_interval_mins: String,
    pub webdav_sync_on_startup: bool,
    pub sync_progress: Option<SyncProgress>,
    pub sync_conflicts: Vec<SyncConflict>,
    pub sync_cancel: Option<Arc<AtomicBool>>,
    /// 0.20: 周期同步标记 —— 只有 `TickWebDavSync` 发起的同步链才发系统通知
    /// （手动上传/下载不发）。TickWebDavSync 置位，SyncFinished 处理后清除。
    pub sync_from_tick: bool,
    pub restart_after_profile_reset: bool,
    pub sync_diff: Option<SyncDiffState>,
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
    pub encrypted_backup: EncryptedBackupState,
}

impl ProfileState {
    /// Begin one profiles page operation and return its submission token.
    pub fn begin_operation(&mut self, operation: ProfilesOperation) -> u64 {
        self.profiles_model.operation_mut(operation).begin()
    }

    /// Settle one profiles page operation with the owner's answer.
    ///
    /// The token is read from the shared phase, so a superseded answer moves
    /// nothing.
    pub fn settle_operation(&mut self, operation: ProfilesOperation, result: Result<(), Failure>) {
        let phase = self.profiles_model.operation_mut(operation);
        let Some(token) = phase.token() else {
            return;
        };
        match result {
            Ok(()) => {
                phase.succeed(token);
            }
            Err(failure) => {
                phase.fail(token, failure);
            }
        }
    }

    /// Return one profiles page operation to the idle phase.
    pub fn reset_operation(&mut self, operation: ProfilesOperation) {
        self.profiles_model.operation_mut(operation).reset();
    }

    /// Whether one profiles page operation is still in flight.
    pub fn operation_running(&self, operation: ProfilesOperation) -> bool {
        self.profiles_model.operation(operation).is_running()
    }
}
