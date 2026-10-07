//! Editor owner.

use crate::types::app::SnapshotDiffMode;
use crate::types::dns::{
    AdvancedEditMode, AdvancedValidationState, DnsTab, FakeIpFormDraft, TunFormDraft,
};
use crate::types::editor::{EditorLazyState, GeoDataStatus};
use crate::types::options::{EditorPane, MrsProviderDetail};
use crate::types::rules::{ProviderUnpackState, RuleRenderItem};
use iced::widget::text_editor;
use infiltrator_application::dns_hosts_editor::DnsHostsEditor;
use infiltrator_application::profile_edit_session::ProfileEditSession;
use infiltrator_application::rule_form_binding::RuleFormBinding;
use infiltrator_application::rule_list_editor::RuleListEditor;
use infiltrator_application::rule_statistics_workbench::RuleStatisticsWorkbench;
use infiltrator_application::rule_trace_actions::RuleTraceActions;
use infiltrator_application::script_workbench::ScriptWorkbench;
use infiltrator_application::snapshot_restore_workbench::SnapshotRestoreWorkbench;
use infiltrator_application::subscription_filter_editor::SubscriptionFilterEditor;
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_contract::dns::FakeIpMappingPool;
use infiltrator_contract::dns_form::DnsWorkbenchForm;
use infiltrator_contract::dns_latency::DnsLatencyReport;
use infiltrator_contract::dns_leak::DnsLeakReport;
use infiltrator_contract::dns_self_heal::DnsSelfHealSnapshot;
use infiltrator_contract::editor_viewport::EditorViewport;
use infiltrator_contract::mrs_acceleration::MrsAccelerationSnapshot;
use infiltrator_contract::profile_document::ProfileDocumentSnapshot;
use infiltrator_contract::provider_cache::{
    KernelEtagSupportSnapshot, ProviderCacheFingerprint, RuleProviderCacheSnapshot,
};
use infiltrator_contract::rule_edit::LogicalDraft;
use infiltrator_contract::rule_tracer::DecisionChainSnapshot;
use infiltrator_contract::rules_workspace::{RulesJsonSection, RulesTab};
use infiltrator_contract::snapshot_history::SnapshotHistorySnapshot;
use infiltrator_contract::stun_probe::StunProbeReport;
use infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot;
use infiltrator_domain::rules::RuleProviderDiff;
use infiltrator_domain::runtime::{ProxyProvider, RuleProvider};
use std::collections::HashMap;
use std::path::PathBuf;

/// 配置编辑器域:Rules / Providers / Sniffer / DNS / Fake-IP / TUN 的 JSON 与
/// 表单双模式编辑状态、脏标记、懒加载与校验(UI-002)。
pub struct ConfigEditorState {
    pub tun_stack: String,
    pub tun_auto_route: bool,
    pub tun_strict_route: bool,
    pub sniffer_enabled: bool,
    pub rule_list: RuleListEditor,
    pub rules_filter: String,
    pub is_loading_rules: bool,
    pub rules_loaded_once: bool,
    pub is_saving_rules: bool,
    /// DUAL-11-14: the shared workspace partition vocabulary.
    pub rules_tab: RulesTab,
    pub rules_json_tab: RulesJsonSection,
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
    /// DUAL-11-04: provider source URLs declared in the active profile, keyed
    /// by provider name and projected from the shared surface read model.
    pub rule_provider_source_urls: HashMap<String, String>,
    /// DUAL-11-05: declared automatic-refresh intervals (seconds) keyed by
    /// provider name. The kernel owns the schedule and the conditional cache.
    pub rule_provider_intervals: HashMap<String, u64>,
    /// DUAL-11-05: the client's local cache-file fingerprint observations keyed
    /// by provider name. These are local file reads compared with the previous
    /// observation, never HTTP `ETag`/`304` results.
    pub rule_provider_fingerprints: HashMap<String, ProviderCacheFingerprint>,
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
    pub rule_providers_json_cache: String,
    pub proxy_providers_json_cache: String,
    pub sniffer_json_cache: String,
    pub rule_providers_editor_state: EditorLazyState,
    pub proxy_providers_editor_state: EditorLazyState,
    pub sniffer_editor_state: EditorLazyState,
    pub rule_providers_json_dirty: bool,
    pub proxy_providers_json_dirty: bool,
    pub sniffer_json_dirty: bool,
    pub is_saving_rule_providers_json: bool,
    pub is_saving_proxy_providers_json: bool,
    pub is_saving_sniffer_json: bool,
    pub is_updating_geo_databases: bool,
    pub dns_json_content: text_editor::Content,
    pub fake_ip_json_content: text_editor::Content,
    pub tun_json_content: text_editor::Content,
    pub dns_json_cache: String,
    pub fake_ip_json_cache: String,
    pub tun_json_cache: String,
    pub dns_editor_state: EditorLazyState,
    pub fake_ip_editor_state: EditorLazyState,
    pub tun_editor_state: EditorLazyState,
    pub dns_tab: DnsTab,
    pub dns_mode: AdvancedEditMode,
    pub fake_ip_mode: AdvancedEditMode,
    pub tun_mode: AdvancedEditMode,
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
    pub is_adding_rule: bool,
    pub proxy_providers: Vec<ProxyProvider>,
    pub rule_providers: Vec<RuleProvider>,
    pub is_loading_providers: bool,
    pub script_sandbox: ScriptWorkbench,
    pub snapshot_restore: SnapshotRestoreWorkbench,
    pub script_code_content: text_editor::Content,
    pub script_yaml_content: text_editor::Content,
    pub snapshot_diff_modal_open: bool,
    pub snapshot_diff_selected_id: Option<String>,
    /// DUAL-09-08: the real snapshot-vs-current diff rendered by the modal.
    /// Computed through the shared `SnapshotApplication`, never fabricated.
    pub snapshot_diff: Option<YamlAstDiffSnapshot>,
    pub snapshot_diff_mode: SnapshotDiffMode,
    pub snapshot_diff_loading: bool,
    pub snapshot_diff_error: Option<String>,
    /// DUAL-09-09: two-step rollback confirmation; the first click arms.
    /// DUAL-09-12: session-local unlock for direct edits of a protected
    /// remote subscription. The application still enforces the rule.
    pub profile_protection_override: bool,
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
    pub is_probing_dns_latency: bool,
    pub dns_hosts_editor: DnsHostsEditor,
    pub is_saving_dns: bool,
    pub is_saving_fake_ip: bool,
    pub is_saving_tun: bool,
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
    pub is_loading_snapshots: bool,
    /// DUAL-09-06: manual "back up now" in flight.
    pub is_backing_up_snapshot: bool,
    /// DUAL-09-07: requested retention for the manual prune control.
    pub snapshot_prune_keep: usize,
    /// DUAL-09-07: manual prune in flight.
    pub is_pruning_snapshots: bool,
    /// DUAL-09-11: the last apply transaction the host core recorded.
    pub apply_transaction: Option<ApplyTransactionSnapshot>,
    pub editor_pane: EditorPane,
    pub mixin_content: text_editor::Content,
    pub mixin_loaded_for: Option<String>,
    pub is_saving_mixin: bool,
    /// DUAL-09-14: the shared surface filter draft (`SubscriptionFilterDraft`),
    /// parsed and persisted by the same application use-case both surfaces call.
    pub filter_editor: SubscriptionFilterEditor,
    pub filter_load: Option<(u64, String)>,
    pub next_filter_load: u64,
    pub mrs_details: Vec<MrsProviderDetail>,
    pub is_scanning_mrs: bool,
    pub syntax_error: Option<String>,
    pub syntax_error_line: Option<usize>,
    pub inspecting_rule_provider_diff: Option<RuleProviderDiff>,
    pub is_loading_rule_provider_diff: bool,
}
