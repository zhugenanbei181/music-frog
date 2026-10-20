//! Editor owner.

use crate::types::app::SnapshotDiffMode;
use crate::types::dns::{AdvancedValidationState, FakeIpFormDraft, TunFormDraft};
use crate::types::editor::{EditorLazyState, GeoDataStatus};
use crate::types::options::{EditorPane, MrsProviderDetail};
use crate::types::rules::{ProviderUnpackState, RuleRenderItem};
use iced::widget::text_editor;
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_contract::dns::FakeIpMappingPool;
use infiltrator_contract::dns_form::DnsWorkbenchForm;
use infiltrator_contract::dns_latency::DnsLatencyReport;
use infiltrator_contract::dns_leak::DnsLeakReport;
use infiltrator_contract::dns_self_heal::DnsSelfHealSnapshot;
use infiltrator_contract::editor_viewport::EditorViewport;
use infiltrator_contract::mrs_acceleration::MrsAccelerationSnapshot;
use infiltrator_contract::profile_document::ProfileDocumentSnapshot;
use infiltrator_contract::provider_cache::{KernelEtagSupportSnapshot, RuleProviderCacheSnapshot};
use infiltrator_contract::rule_edit::LogicalDraft;
use infiltrator_contract::rule_provider_snapshot::RuleProviderSnapshot;
use infiltrator_contract::rule_tracer::DecisionChainSnapshot;
use infiltrator_contract::rules_workspace::RulesJsonSection;
use infiltrator_contract::snapshot_history::SnapshotHistorySnapshot;
use infiltrator_contract::stun_probe::StunProbeReport;
use infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot;
use infiltrator_domain::rules::RuleProviderDiff;
use infiltrator_presentation::dns_hosts_editor::DnsHostsEditor;
use infiltrator_presentation::dns_page_model::{DnsOperation, DnsPageModel};
use infiltrator_presentation::profile_edit_session::ProfileEditSession;
use infiltrator_presentation::rule_form_binding::RuleFormBinding;
use infiltrator_presentation::rule_list_editor::RuleListEditor;
use infiltrator_presentation::rule_statistics_workbench::RuleStatisticsWorkbench;
use infiltrator_presentation::rule_trace_actions::RuleTraceActions;
use infiltrator_presentation::rules_page_model::RulesPageModel;
use infiltrator_presentation::script_workbench::ScriptWorkbench;
use infiltrator_presentation::snapshot_restore_workbench::SnapshotRestoreWorkbench;
use infiltrator_presentation::subscription_filter_editor::SubscriptionFilterEditor;
use lurker_core_contract::error::Failure;
use lurker_core_contract::profile_document_identity::ProfileDocumentIdentity;
use lurker_core_contract::profile_protection::ProfileWriteAuthorization;
use std::path::PathBuf;

/// The stage of the snapshot-diff modal.
///
/// The modal is one process: it is closed, it is loading the shared diff, it
/// shows the loaded diff, or it shows the typed failure. The stage carries the
/// selected snapshot and the loaded diff, so an open modal without a snapshot,
/// a stale selection while the modal is closed, or a failure next to a loaded
/// diff are no longer representable.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SnapshotDiffModalState {
    /// The modal is closed and no snapshot is selected.
    #[default]
    Closed,
    /// The modal is open and the shared diff is still loading.
    Loading { snapshot_id: String },
    /// DUAL-09-08: the real snapshot-vs-current diff rendered by the modal.
    /// Computed through the shared `SnapshotApplication`, never fabricated.
    Loaded {
        snapshot_id: String,
        diff: YamlAstDiffSnapshot,
    },
    /// The modal is open and the load failed with the typed reason.
    Failed { snapshot_id: String, reason: String },
}

impl SnapshotDiffModalState {
    /// The snapshot the open modal is about.
    pub fn snapshot_id(&self) -> Option<&str> {
        match self {
            SnapshotDiffModalState::Closed => None,
            SnapshotDiffModalState::Loading { snapshot_id }
            | SnapshotDiffModalState::Loaded { snapshot_id, .. }
            | SnapshotDiffModalState::Failed { snapshot_id, .. } => Some(snapshot_id),
        }
    }

    /// The loaded shared diff, when the modal shows it.
    pub fn diff(&self) -> Option<&YamlAstDiffSnapshot> {
        match self {
            SnapshotDiffModalState::Loaded { diff, .. } => Some(diff),
            SnapshotDiffModalState::Closed
            | SnapshotDiffModalState::Loading { .. }
            | SnapshotDiffModalState::Failed { .. } => None,
        }
    }

    /// The typed failure, when the load failed.
    pub fn failure(&self) -> Option<&str> {
        match self {
            SnapshotDiffModalState::Failed { reason, .. } => Some(reason),
            SnapshotDiffModalState::Closed
            | SnapshotDiffModalState::Loading { .. }
            | SnapshotDiffModalState::Loaded { .. } => None,
        }
    }

    /// True while the shared diff is loading.
    pub fn is_loading(&self) -> bool {
        matches!(self, SnapshotDiffModalState::Loading { .. })
    }

    /// True when the modal is open in any stage.
    pub fn is_open(&self) -> bool {
        !matches!(self, SnapshotDiffModalState::Closed)
    }
}

/// DNS server latency probing phase.
///
/// The probe phase is owned by the shared DNS page model
/// (`DnsOperation::ProbeLatency`); the surface keeps no private copy.

/// Load phase for profile snapshot history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapshotsLoadPhase {
    #[default]
    Idle,
    Loading,
}

impl SnapshotsLoadPhase {
    pub const fn is_loading(self) -> bool {
        matches!(self, Self::Loading)
    }
}

/// Manual backup phase for profile snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapshotBackupPhase {
    #[default]
    Idle,
    BackingUp,
}

impl SnapshotBackupPhase {
    pub const fn is_backing_up(self) -> bool {
        matches!(self, Self::BackingUp)
    }
}

/// Manual prune phase for profile snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapshotPrunePhase {
    #[default]
    Idle,
    Pruning,
}

impl SnapshotPrunePhase {
    pub const fn is_pruning(self) -> bool {
        matches!(self, Self::Pruning)
    }
}

/// Scan phase for MRS binary rule-set files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MrsScanPhase {
    #[default]
    Idle,
    Scanning,
}

impl MrsScanPhase {
    pub const fn is_scanning(self) -> bool {
        matches!(self, Self::Scanning)
    }
}

/// Load phase for inspecting rule provider diff.
///
/// The diff-load phase is owned by the shared rules page model
/// (`RulesOperation::LoadProviderDiff`); the surface keeps no private copy.

/// 配置编辑器域:Rules / Providers / Sniffer / DNS / Fake-IP / TUN 的 JSON 与
/// 表单双模式编辑状态、脏标记、懒加载与校验(UI-002)。
pub struct ConfigEditorState {
    pub tun_stack: String,
    pub tun_auto_route: bool,
    pub tun_strict_route: bool,
    pub sniffer_enabled: bool,
    pub rule_list: RuleListEditor,
    pub rules_filter: String,
    pub rules_loaded_once: bool,
    /// The one page model of the rules page: the active tab, the active JSON
    /// section, the JSON document drafts and the eight operation phases.
    pub rules_model: RulesPageModel,
    pub rules_page: usize,
    pub rules_page_size: usize,
    /// DUAL-11-08: scroll offset (logical px) of the rules list viewport, fed
    /// back by the list's `on_scroll`. Drives the render window.
    pub rules_scroll_offset_px: f32,
    /// DUAL-11-08: measured height of the rules list viewport, or the shared
    /// declared fallback until the scrollable has reported one.
    pub rules_viewport_px: f32,
    pub rule_trace: RuleTraceActions,
    pub rules_tracer_input: String,
    /// DUAL-12-10: simulated inbound source IP typed into the tracer sandbox.
    /// It is submitted atomically with the query through the shared command owner, so the
    /// Inbound decision stage reflects the same environment on both surfaces.
    pub rules_tracer_src_ip: String,
    /// Decision chain replayed by the shared rule tracer engine. No UI-local
    /// second source of truth: hosts with a composed port share the query
    /// state the surface reader projects; demo data is an explicit fixture.
    pub rules_tracer_chain: Option<DecisionChainSnapshot>,
    /// DUAL-12-08: shared reverse-apply gate for the currently traced rule,
    /// consumed from the surface read model's `can_reverse_apply`.
    pub rules_tracer_can_reverse_apply: bool,
    /// DUAL-12-08: shared suggested replacement outbound, seeded into the
    /// chooser. Never a fabricated group name.
    pub rules_tracer_suggested_target: Option<String>,
    /// DUAL-12-08: outbound target typed into the reverse-apply chooser.
    pub rules_tracer_override_target: String,
    pub rules_providers_expanded: bool,
    pub rules_render_cache: Vec<RuleRenderItem>,
    pub rules_filtered_indices: Vec<usize>,
    pub rules_heavy_ready: bool,
    /// DUAL-11-08: publish cap of the shared rules read model (0 = uncapped)
    /// and the rules it dropped, when the published view is truncated. The
    /// editor list itself is loaded in full from the profile.
    pub rule_publish_limit: usize,
    pub rule_publish_omitted: Option<usize>,
    /// DUAL-11-07: the observed kernel rule-provider cache location.
    pub rule_provider_cache: RuleProviderCacheSnapshot,
    /// DUAL-11-05: the kernel's real `etag-support` capability declared by the
    /// active profile (a top-level key; mihomo defaults it to `true`). The
    /// provider view renders the declaration, never a `304` outcome.
    pub rule_etag_support: KernelEtagSupportSnapshot,
    /// DUAL-11-03: shared MRS binary acceleration read model, projected from
    /// the surface reader. The providers tab renders this, never a local
    /// fabricated rule-set list.
    pub mrs_acceleration: MrsAccelerationSnapshot,
    pub rule_providers_json_content: text_editor::Content,
    pub proxy_providers_json_content: text_editor::Content,
    pub sniffer_json_content: text_editor::Content,
    /// Exact source revision bound to each JSON editor buffer.
    pub rule_providers_json_identity: Option<ProfileDocumentIdentity>,
    pub proxy_providers_json_identity: Option<ProfileDocumentIdentity>,
    pub sniffer_json_identity: Option<ProfileDocumentIdentity>,
    pub rule_providers_editor_state: EditorLazyState,
    pub proxy_providers_editor_state: EditorLazyState,
    pub sniffer_editor_state: EditorLazyState,
    pub dns_json_content: text_editor::Content,
    pub fake_ip_json_content: text_editor::Content,
    pub tun_json_content: text_editor::Content,
    pub dns_json_cache: String,
    pub fake_ip_json_cache: String,
    pub tun_json_cache: String,
    pub dns_editor_state: EditorLazyState,
    pub fake_ip_editor_state: EditorLazyState,
    pub tun_editor_state: EditorLazyState,
    /// The one page model of the DNS page: the active partition, the
    /// form/JSON edit mode and the four write/probe phases.
    pub dns_model: DnsPageModel,
    pub dns_heavy_ready: bool,
    pub advanced_configs_loaded_once: bool,
    pub dns_json_dirty: bool,
    pub fake_ip_json_dirty: bool,
    pub tun_json_dirty: bool,
    pub dns_form: DnsWorkbenchForm,
    pub fake_ip_form: FakeIpFormDraft,
    pub tun_form: TunFormDraft,
    pub dns_form_dirty: bool,
    pub fake_ip_form_dirty: bool,
    pub tun_form_dirty: bool,
    pub advanced_validation: AdvancedValidationState,
    pub new_rule_type: String,
    pub rule_form_binding: RuleFormBinding,
    pub new_rule_payload: String,
    pub new_rule_target: String,
    pub rule_providers: Vec<RuleProviderSnapshot>,
    pub script_sandbox: ScriptWorkbench,
    pub snapshot_restore: SnapshotRestoreWorkbench,
    pub script_code_content: text_editor::Content,
    pub script_yaml_content: text_editor::Content,
    /// The snapshot-diff modal is one process with one stage.
    pub snapshot_diff_modal: SnapshotDiffModalState,
    pub snapshot_diff_mode: SnapshotDiffMode,
    /// DUAL-09-09: two-step rollback confirmation; the first click arms.
    /// DUAL-09-12: session-local unlock for direct edits of a protected
    /// remote subscription. The application still enforces the rule.
    pub profile_write_authorization: ProfileWriteAuthorization,
    pub subrule_draft: LogicalDraft,
    pub geodata_status: GeoDataStatus,
    pub rule_hit_audit: RuleStatisticsWorkbench,
    pub provider_unpack: ProviderUnpackState,
    pub dns_nameservers: Vec<String>,
    pub dns_fallback_servers: Vec<String>,
    pub dns_enhanced_mode: String,
    /// DUAL-14-06: observed Fake-IP bindings from the shared read model.
    pub dns_fake_ip_pool: FakeIpMappingPool,
    /// DUAL-14-06: the local search box filter (view-local, not a fact).
    pub dns_fake_ip_query: String,
    /// DUAL-14-10: the shared latency probe report (real per-server results).
    pub dns_latency: DnsLatencyReport,
    /// DUAL-14-08: the shared cross-source DNS leak report.
    pub dns_leak: DnsLeakReport,
    /// DUAL-14-09 (re-scoped): the shared STUN UDP-egress report of this host.
    pub dns_stun: StunProbeReport,
    /// DUAL-14-13: the shared DNS self-heal observation.
    pub dns_self_heal: DnsSelfHealSnapshot,
    /// DUAL-14-10: a probe started from this surface is in flight.
    pub dns_hosts_editor: DnsHostsEditor,
    pub editor_content: text_editor::Content,
    pub document_session: ProfileEditSession,
    pub mixin_session: ProfileEditSession,
    pub document_latest: Option<(PathBuf, ProfileDocumentSnapshot)>,
    pub document_load: Option<(u64, PathBuf)>,
    pub mixin_load: Option<(u64, String)>,
    pub next_editor_read: u64,
    /// DUAL-09-02/13: the shared viewport window the profile editor renders.
    /// The widget owns its pixel scroll; this mirrors its published scroll
    /// deltas through the shared clamp and follows the caret, which is what
    /// keeps the line-number gutter and the rendered window in step.
    pub profile_viewport: EditorViewport,
    /// Same window model for the Mixin overlay pane.
    pub mixin_viewport: EditorViewport,
    pub editor_path: Option<PathBuf>,
    pub editor_path_setting: String,
    /// DUAL-09-06/07: the shared snapshot history (entries + shared prune
    /// view) for the profile being edited. The surface never re-derives the
    /// retention policy locally.
    pub snapshot_history: Option<SnapshotHistorySnapshot>,
    pub snapshots_load_phase: SnapshotsLoadPhase,
    /// DUAL-09-06: manual "back up now" in flight.
    pub snapshot_backup_phase: SnapshotBackupPhase,
    /// DUAL-09-07: requested retention for the manual prune control.
    pub snapshot_prune_keep: usize,
    /// DUAL-09-07: manual prune in flight.
    pub snapshot_prune_phase: SnapshotPrunePhase,
    /// DUAL-09-11: the last apply transaction the host core recorded.
    pub apply_transaction: Option<ApplyTransactionSnapshot>,
    pub editor_pane: EditorPane,
    pub mixin_content: text_editor::Content,
    pub mixin_loaded_for: Option<String>,
    /// DUAL-09-14: the shared surface filter draft (`SubscriptionFilterDraft`),
    /// parsed and persisted by the same application use-case both surfaces call.
    pub filter_editor: SubscriptionFilterEditor,
    pub filter_load: Option<(u64, String)>,
    pub next_filter_load: u64,
    pub mrs_details: Vec<MrsProviderDetail>,
    pub mrs_scan_phase: MrsScanPhase,
    pub syntax_error: Option<String>,
    pub syntax_error_line: Option<usize>,
    pub inspecting_rule_provider_diff: Option<RuleProviderDiff>,
}

impl ConfigEditorState {
    /// Begin one DNS page operation and return its submission token.
    pub fn begin_dns_operation(&mut self, operation: DnsOperation) -> u64 {
        self.dns_model.operation_mut(operation).begin()
    }

    /// Settle one DNS page operation with the owner's answer.
    ///
    /// The token is read from the shared phase, so a superseded answer moves
    /// nothing.
    pub fn settle_dns_operation(&mut self, operation: DnsOperation, result: Result<(), Failure>) {
        let phase = self.dns_model.operation_mut(operation);
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

    /// Return one DNS page operation to the idle phase.
    pub fn reset_dns_operation(&mut self, operation: DnsOperation) {
        self.dns_model.operation_mut(operation).reset();
    }

    /// Whether one DNS page operation is still in flight.
    pub fn dns_operation_running(&self, operation: DnsOperation) -> bool {
        self.dns_model.operation(operation).is_running()
    }

    /// The JSON document buffer of one section.
    pub fn rules_json_content(&self, section: RulesJsonSection) -> &text_editor::Content {
        match section {
            RulesJsonSection::RuleProviders => &self.rule_providers_json_content,
            RulesJsonSection::ProxyProviders => &self.proxy_providers_json_content,
            RulesJsonSection::Sniffer => &self.sniffer_json_content,
        }
    }

    /// The JSON document buffer of one section, for the fold that changes it.
    pub fn rules_json_content_mut(
        &mut self,
        section: RulesJsonSection,
    ) -> &mut text_editor::Content {
        match section {
            RulesJsonSection::RuleProviders => &mut self.rule_providers_json_content,
            RulesJsonSection::ProxyProviders => &mut self.proxy_providers_json_content,
            RulesJsonSection::Sniffer => &mut self.sniffer_json_content,
        }
    }

    /// The committed text of one JSON section.
    ///
    /// The baseline lives in the shared page model, next to the draft it
    /// belongs to: the surface keeps no second copy of the committed text.
    pub fn rules_json_baseline(&self, section: RulesJsonSection) -> &str {
        self.rules_model
            .json_buffer(section)
            .baseline()
            .map(String::as_str)
            .unwrap_or_default()
    }

    /// The lazy-load state of one JSON section.
    pub fn rules_json_editor_state(&self, section: RulesJsonSection) -> EditorLazyState {
        match section {
            RulesJsonSection::RuleProviders => self.rule_providers_editor_state,
            RulesJsonSection::ProxyProviders => self.proxy_providers_editor_state,
            RulesJsonSection::Sniffer => self.sniffer_editor_state,
        }
    }

    /// The lazy-load state of one JSON section, for the fold that changes it.
    pub fn rules_json_editor_state_mut(
        &mut self,
        section: RulesJsonSection,
    ) -> &mut EditorLazyState {
        match section {
            RulesJsonSection::RuleProviders => &mut self.rule_providers_editor_state,
            RulesJsonSection::ProxyProviders => &mut self.proxy_providers_editor_state,
            RulesJsonSection::Sniffer => &mut self.sniffer_editor_state,
        }
    }

    /// The source revision bound to one JSON section.
    pub fn rules_json_identity(
        &self,
        section: RulesJsonSection,
    ) -> Option<&ProfileDocumentIdentity> {
        match section {
            RulesJsonSection::RuleProviders => self.rule_providers_json_identity.as_ref(),
            RulesJsonSection::ProxyProviders => self.proxy_providers_json_identity.as_ref(),
            RulesJsonSection::Sniffer => self.sniffer_json_identity.as_ref(),
        }
    }

    /// The source revision bound to one JSON section, for the fold that changes it.
    pub fn rules_json_identity_mut(
        &mut self,
        section: RulesJsonSection,
    ) -> &mut Option<ProfileDocumentIdentity> {
        match section {
            RulesJsonSection::RuleProviders => &mut self.rule_providers_json_identity,
            RulesJsonSection::ProxyProviders => &mut self.proxy_providers_json_identity,
            RulesJsonSection::Sniffer => &mut self.sniffer_json_identity,
        }
    }
}
