//! AppState 域拆分(UI-002):原先 161 个平铺字段收敛为五个语义域结构体。
//!
//! 域边界(与 `TODO.md` UI-002 对应):
//! - [`RuntimeState`] 内核与运行控制;[`ProfileState`] 订阅/档案/同步;
//! - [`ConfigEditorState`] 全部配置编辑器;[`DiagnosticsState`] 运行态诊断与性能;
//! - [`ShellState`] 导航/语言/主题/托盘/Admin/demo 等外壳关注点。
//!
//! 视图层只读这些域做纯渲染投影;update 层按域定位字段。

use crate::tray::SharedTrayEventReceiver;
use crate::tray::spec::TrayController;
use crate::types::app::{ConfirmAction, Route, ToastStatus, Transition};
use crate::types::dns::{
    AdvancedEditMode, AdvancedValidationState, DnsTab, FakeIpFormDraft, TunFormDraft,
};
use crate::types::editor::EditorLazyState;
use crate::types::perf::PerfSnapshot;
use crate::types::rules::{RuleRenderItem, RulesJsonTab, RulesTab};
use crate::types::runtime::{
    RebuildFlowState, RuntimePatchSnapshot, RuntimeStatus, RuntimeStreamState,
};
use iced::Theme;
use iced::widget::text_editor;
use infiltrator_application::system_proxy_application::SystemProxyApplication;
use infiltrator_contract::controller::ControllerAuthSnapshot;
use infiltrator_contract::mtu::MtuNegotiationSnapshot;
use infiltrator_contract::offline_startup::OfflineStartupSnapshot;
use infiltrator_contract::port_conflict::PortConflictSnapshot;
use infiltrator_contract::privileged_network::PrivilegedNetworkSnapshot;
use infiltrator_contract::resources::CoreResourceSnapshot;
use infiltrator_contract::service_mode::ServiceModeSnapshot;
use infiltrator_contract::snapshot::CoreLifecycleSnapshot;
use infiltrator_contract::system_proxy::SystemProxyRecoverySnapshot;
use infiltrator_contract::system_proxy::SystemProxySnapshot;
use infiltrator_contract::system_toggle::SystemToggleSnapshot;
use infiltrator_contract::version::{
    CoreArtifactVerification, CoreVersionSnapshot, InstalledCoreVersion,
};
use infiltrator_domain::profiles::ProfileInfo;
use infiltrator_domain::proxy::Proxy;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_domain::runtime::{
    ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider, TrafficData,
};
use infiltrator_domain::snapshots::SnapshotMeta;
use infiltrator_ports::host_runtime::{HostRuntime, TunServiceStatus};
use infiltrator_ports::privileged_network::PrivilegedNetworkPort;
use infiltrator_ports::system_proxy::SystemProxyPort;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
/// 内核运行时域:内核进程句柄、生命周期状态、代理模式/系统代理/自启、
/// 节点与分组运行控制、批量测速与内核版本管理(UI-002)。
pub struct RuntimeState {
    pub runtime: Option<Arc<dyn HostRuntime>>,
    pub runtime_generation: u64,
    /// Session identity paired with `runtime_generation`; delayed UI results
    /// must match both before changing a projection.
    pub core_session_token: Option<infiltrator_contract::session::SessionToken>,
    /// Exact shared lifecycle read model; `status` remains a coarse legacy
    /// rendering compatibility field for existing Iced widgets.
    pub core_lifecycle: CoreLifecycleSnapshot,
    pub mtu: MtuNegotiationSnapshot,
    pub ipv6_routing: infiltrator_contract::ipv6::Ipv6RoutingSnapshot,
    pub vpn: infiltrator_contract::vpn::VpnSessionSnapshot,
    /// Shared control state used by sidebar and settings quick toggles.
    pub system_toggles: SystemToggleSnapshot,
    pub privileged_network: PrivilegedNetworkSnapshot,
    pub privileged_network_port: Option<Arc<dyn PrivilegedNetworkPort>>,
    pub traffic_waveform: infiltrator_contract::traffic_waveform::TrafficWaveformSnapshot,
    pub traffic_scale: infiltrator_contract::traffic_scale::TrafficScaleSnapshot,
    pub traffic_topology: infiltrator_contract::traffic_topology::TrafficTopologySnapshot,
    /// Core reload/reconnect graceful degradation mask shared with Bevy; the
    /// overview banner renders it while the core restarts or reconnects.
    pub reconnect_mask: infiltrator_contract::reconnect_mask::ReconnectMaskSnapshot,
    pub active_exit: infiltrator_contract::active_exit::ActiveExitSnapshot,
    pub subscription_quota: infiltrator_contract::subscription_quota::SubscriptionQuotaSnapshot,
    pub system_proxy: SystemProxySnapshot,
    pub system_proxy_recovery: SystemProxyRecoverySnapshot,
    /// Retained independently of the running core so a system proxy can be
    /// disabled during shutdown/cleanup without a live Mihomo runtime.
    pub system_proxy_port: Option<Arc<dyn SystemProxyPort>>,
    pub system_proxy_application: Option<SystemProxyApplication>,
    pub system_proxy_last_repair_count: u64,
    pub lifecycle_token: u64,
    pub status: RuntimeStatus,
    pub proxies: HashMap<String, Proxy>,
    pub is_loading_proxies: bool,
    pub filtered_groups: Vec<(String, Vec<String>)>,
    pub proxy_filter: String,
    pub proxy_sort_by_delay: bool,
    pub proxy_delay_sort: String,
    pub runtime_delay_test_url: String,
    /// DUAL-06-03: user-typed speedtest target URL shown in the speedtest card.
    /// Empty means "use the shared engine default"; the typed value is passed
    /// into `run_scope` / `probe_node` so the port owns the effective fact.
    pub runtime_speedtest_url: String,
    pub runtime_delay_timeout_ms: String,
    pub runtime_testing_delay_proxy: String,
    pub runtime_testing_all_delays: bool,
    pub runtime_selected_group: String,
    pub runtime_selected_proxy: String,
    pub runtime_connection_filter: String,
    pub runtime_connection_sort: String,
    pub runtime_prev_upload_total: Option<u64>,
    pub runtime_prev_download_total: Option<u64>,
    pub runtime_prev_snapshot_at: Option<Instant>,
    pub pending_runtime_patch: Option<RuntimePatchSnapshot>,
    pub runtime_patch_token: u64,
    pub runtime_auto_refresh: bool,
    pub runtime_poll_tick: u64,
    pub proxy_mode: Option<String>,
    /// Whether the running core reports a top-level `script:` block
    /// (`GET /configs` → `script`). Gates the Script mode entry points.
    pub script_block_present: bool,
    pub tun_enabled: Option<bool>,
    pub tun_service_status: Option<TunServiceStatus>,
    pub is_installing_tun_service: bool,
    pub system_proxy_enabled: bool,
    pub system_proxy_pending: bool,
    pub autostart_enabled: bool,
    pub filter_alive_only: bool,
    pub favorite_proxies: std::collections::HashSet<String>,
    pub proxy_compact_view: bool,
    pub inspecting_proxy: Option<String>,
    pub is_adding_custom_node: bool,
    pub new_node_type: String,
    pub new_node_name: String,
    pub new_node_server: String,
    pub new_node_port: String,
    pub new_node_credential: String,
    pub new_node_cipher: String,
    pub new_node_tls: bool,
    pub installed_kernels: Vec<InstalledCoreVersion>,
    pub latest_core_version: Option<String>,
    pub core_channel: String,
    /// Shared online probe result for Stable, Alpha and Meta-Core.
    pub core_versions: CoreVersionSnapshot,
    pub core_integrity: CoreArtifactVerification,
    pub controller_auth: ControllerAuthSnapshot,
    pub service_mode: ServiceModeSnapshot,
    pub port_conflicts: PortConflictSnapshot,
    pub core_resources: CoreResourceSnapshot,
    pub offline_startup: OfflineStartupSnapshot,
    pub download_progress: f32,
    pub download_stats: Option<crate::types::app::CoreDownloadProgress>,
    pub core_download_token: u64,
    pub core_download_cancel: Option<Arc<AtomicBool>>,
    pub is_downloading_core: bool,
    pub is_checking_update: bool,
    pub rebuild_flow: RebuildFlowState,
    pub proxy_groups_expanded: Option<Vec<String>>,
    pub proxy_group_order: Vec<String>,
    pub custom_node_modal_open: bool,
    pub custom_node_uri_input: String,
    /// DUAL-05: the shared protocol studio snapshot (typed draft, derived
    /// report, URI preview and the empirically computed URI fidelity gaps).
    /// The modal is a pure projection of this; Iced keeps no second protocol
    /// fact source.
    pub custom_node_studio: infiltrator_contract::protocol_fidelity::ProtocolStudioSnapshot,
    pub network_roaming: crate::types::runtime::NetworkRoamingState,
    pub pac_manager: crate::types::app::PacManagerConfig,
    pub latency_radar: crate::types::runtime::LatencyRadarState,
    pub apply_guard: crate::types::runtime::ApplyTransactionGuardState,
    pub lan_sharing: crate::types::app::LanSharingConfig,
    pub lan_sharing_committed: crate::types::app::LanSharingConfig,
    pub lan_sharing_dirty: bool,
    pub lan_security: crate::types::app::LanSecurityConfig,
    pub lan_security_committed: crate::types::app::LanSecurityConfig,
    pub lan_security_dirty: bool,
    pub tun_stack_config: crate::types::dns::TunStackConfig,
}

/// 订阅与档案域:Profile 列表、订阅导入/更新、WebDAV 同步与应用设置保存(UI-002)。
pub struct ProfileState {
    pub profiles: Vec<ProfileInfo>,
    pub profiles_filter: String,
    pub is_loading_profiles: bool,
    pub import_url: String,
    pub import_name: String,
    pub import_activate: bool,
    pub is_importing: bool,
    pub local_import_path: String,
    pub local_import_name: String,
    pub local_import_activate: bool,
    pub is_importing_local: bool,
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
    pub is_saving_subscription: bool,
    pub is_updating_subscription_now: bool,
    pub webdav_url: String,
    pub webdav_user: String,
    pub webdav_pass: String,
    pub webdav_enabled: bool,
    pub webdav_sync_interval_mins: String,
    pub webdav_sync_on_startup: bool,
    pub is_syncing: bool,
    pub sync_progress: Option<crate::types::app::SyncProgress>,
    pub sync_conflicts: Vec<crate::types::app::SyncConflict>,
    pub is_testing_webdav: bool,
    pub sync_cancel: Option<Arc<AtomicBool>>,
    /// 0.20: 周期同步标记 —— 只有 `TickWebDavSync` 发起的同步链才发系统通知
    /// （手动上传/下载不发）。TickWebDavSync 置位，SyncFinished 处理后清除。
    pub sync_from_tick: bool,
    pub is_saving_app_settings: bool,
    pub is_saving_profile: bool,
    pub restart_after_profile_reset: bool,
    pub sync_diff: Option<crate::types::options::SyncDiffState>,
    pub is_loading_sync_diff: bool,
    pub is_applying_sync_diff: bool,
    pub aggregator_modal_open: bool,
    pub aggregator_selected_profiles: Vec<String>,
    pub aggregator_name_input: String,
    /// DUAL-08: the last shared aggregation preview (never built locally).
    pub aggregator_report: Option<infiltrator_contract::aggregator::AggregationReport>,
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
    /// reload the kernel when the host owns a managed runtime).
    pub aggregator_activate_after_create: bool,
    /// DUAL-08-08: free-text regex rename rules (`模式 => 替换`).
    pub aggregator_renames: String,
    /// DUAL-08-10: custom group name being typed.
    pub aggregator_custom_name: String,
    /// DUAL-08-10: custom group member keywords being typed.
    pub aggregator_custom_keywords: String,
    /// DUAL-08-10: appended custom groups awaiting the next preview/save.
    pub aggregator_custom_groups: Vec<infiltrator_contract::aggregator::AggregationCustomGroup>,
    /// DUAL-08-13: persisted template library (never built locally).
    pub aggregator_templates: Vec<infiltrator_contract::aggregator::AggregationTemplate>,
    /// DUAL-08-13: name typed for the "save as template" action.
    pub aggregator_template_name: String,
    pub is_aggregating: bool,
    pub encrypted_backup: crate::types::options::EncryptedBackupState,
    pub quota_schedule: crate::types::options::QuotaScheduleState,
}

/// 配置编辑器域:Rules / Providers / Sniffer / DNS / Fake-IP / TUN 的 JSON 与
/// 表单双模式编辑状态、脏标记、懒加载与校验(UI-002)。
pub struct ConfigEditorState {
    pub tun_stack: String,
    pub tun_auto_route: bool,
    pub tun_strict_route: bool,
    pub sniffer_enabled: bool,
    pub rules: Vec<RuleEntry>,
    pub rules_filter: String,
    pub is_loading_rules: bool,
    pub rules_loaded_once: bool,
    pub is_saving_rules: bool,
    pub rules_dirty: bool,
    pub rules_tab: RulesTab,
    pub rules_json_tab: RulesJsonTab,
    pub rules_page: usize,
    pub rules_page_size: usize,
    pub rules_tracer_input: String,
    /// DUAL-12-10: simulated inbound source IP typed into the tracer sandbox.
    /// It is pushed into the shared tracer engine via `RuleTracerPort` so the
    /// Inbound decision stage reflects the same environment on both surfaces.
    pub rules_tracer_src_ip: String,
    /// Decision chain replayed by the shared rule tracer engine. No UI-local
    /// second source of truth: hosts with a composed port share the query
    /// state the surface reader projects; hostless demo runs trace the same
    /// pure application directly.
    pub rules_tracer_chain: Option<infiltrator_contract::rule_tracer::DecisionChainSnapshot>,
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
    /// DUAL-11-08: publish cap of the shared rules read model (0 = uncapped)
    /// and the rules it dropped, when the published view is truncated. The
    /// editor list itself is loaded in full from the profile.
    pub rule_publish_limit: usize,
    pub rule_publish_omitted: Option<usize>,
    /// DUAL-11-03: shared MRS binary acceleration read model, projected from
    /// the surface reader. The providers tab renders this, never a local
    /// fabricated rule-set list.
    pub mrs_acceleration: infiltrator_contract::mrs_acceleration::MrsAccelerationSnapshot,
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
    pub dns_form: infiltrator_contract::dns_form::DnsWorkbenchForm,
    pub fake_ip_form: FakeIpFormDraft,
    pub tun_form: TunFormDraft,
    pub dns_form_dirty: bool,
    pub fake_ip_form_dirty: bool,
    pub tun_form_dirty: bool,
    pub advanced_validation: AdvancedValidationState,
    pub new_rule_type: String,
    pub new_rule_payload: String,
    pub new_rule_target: String,
    pub is_adding_rule: bool,
    pub proxy_providers: Vec<ProxyProvider>,
    pub rule_providers: Vec<RuleProvider>,
    pub is_loading_providers: bool,
    pub script_sandbox: crate::types::editor::ScriptSandboxState,
    pub snapshot_diff_modal_open: bool,
    pub snapshot_diff_selected_id: Option<String>,
    /// DUAL-09-08: the real snapshot-vs-current diff rendered by the modal.
    /// Computed through the shared `SnapshotApplication`, never fabricated.
    pub snapshot_diff: Option<infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot>,
    pub snapshot_diff_mode: crate::types::app::SnapshotDiffMode,
    pub snapshot_diff_loading: bool,
    pub snapshot_diff_error: Option<String>,
    /// DUAL-09-09: two-step rollback confirmation; the first click arms.
    pub snapshot_diff_rollback_armed: bool,
    /// DUAL-09-12: session-local unlock for direct edits of a protected
    /// remote subscription. The application still enforces the rule.
    pub profile_protection_override: bool,
    pub subrule_draft: infiltrator_contract::rule_edit::LogicalDraft,
    pub geodata_status: crate::types::editor::GeoDataStatus,
    pub rule_hit_audit: crate::types::rules::RuleHitAuditState,
    pub provider_unpack: crate::types::rules::ProviderUnpackState,
    pub dns_nameservers: Vec<String>,
    pub dns_fallback_servers: Vec<String>,
    pub dns_enhanced_mode: String,
    /// DUAL-14-06: observed Fake-IP bindings from the shared read model.
    pub dns_fake_ip_pool: infiltrator_contract::dns::FakeIpMappingPool,
    /// DUAL-14-06: the local search box filter (view-local, not a fact).
    pub dns_fake_ip_query: String,
    /// DUAL-14-10: the shared latency-probe availability fact.
    pub dns_latency: infiltrator_contract::dns::DnsLatencyStatus,
    /// DUAL-14-11: the shared `dns.hosts` draft (one row per address).
    pub dns_hosts: Vec<infiltrator_contract::dns::DnsHostEntry>,
    pub dns_hosts_address: String,
    pub dns_hosts_domain: String,
    pub dns_hosts_dirty: bool,
    pub is_saving_dns_hosts: bool,
    pub is_saving_dns: bool,
    pub is_saving_fake_ip: bool,
    pub is_saving_tun: bool,
    pub editor_content: text_editor::Content,
    pub editor_path: Option<PathBuf>,
    pub editor_path_setting: String,
    pub profile_snapshots: Vec<SnapshotMeta>,
    pub is_loading_snapshots: bool,
    pub is_restoring_snapshot: bool,
    /// DUAL-09-09: the snapshot whose restore has been armed but not confirmed.
    pub pending_restore_snapshot: Option<PathBuf>,
    pub editor_pane: crate::types::options::EditorPane,
    pub mixin_content: text_editor::Content,
    pub mixin_loaded_for: Option<String>,
    pub is_saving_mixin: bool,
    pub filter_draft: crate::types::options::FilterDraft,
    pub filter_loaded_for: Option<String>,
    pub is_saving_filter: bool,
    pub mrs_details: Vec<crate::types::options::MrsProviderDetail>,
    pub is_scanning_mrs: bool,
    pub syntax_error: Option<String>,
    pub syntax_error_line: Option<usize>,
    pub inspecting_rule_provider_diff: Option<infiltrator_domain::rules::RuleProviderDiff>,
    pub is_loading_rule_provider_diff: bool,
}

/// 诊断域:流量/内存/连接/日志运行态快照与性能 HUD 测量(UI-002)。
pub struct DiagnosticsState {
    pub traffic: Option<TrafficData>,
    pub traffic_history: VecDeque<(u64, u64)>,
    pub memory: Option<MemoryData>,
    pub public_ip: Option<String>,
    pub public_ip_provider: Option<String>,
    pub public_ip_checked_at: Option<String>,
    pub public_ip_error: Option<String>,
    pub connections: Option<ConnectionSnapshot>,
    /// Connections list pagination (mirrors the rules page pattern): the
    /// view renders only the current window so multi-thousand-connection
    /// snapshots never build thousands of widgets at once.
    pub connections_page: usize,
    pub connections_page_size: usize,
    pub logs: VecDeque<String>,
    pub log_level: String,
    pub fps: u32,
    pub last_frame_time: Instant,
    /// Adapter-local phase for the Overview topology flow strip. The shared
    /// snapshot remains the only source of whether a flow exists.
    pub topology_flow_phase: f32,
    pub perf_snapshot: PerfSnapshot,
    pub perf_panel_visible: bool,
    pub perf_nav_started_at: Option<Instant>,
    pub perf_nav_route: Option<Route>,
    pub logs_stream_state: RuntimeStreamState,
    pub traffic_stream_state: RuntimeStreamState,
    pub connections_stream_state: RuntimeStreamState,
    pub doctor: crate::types::doctor::DoctorPanelState,
    pub inspecting_connection_id: Option<String>,
    pub dns_leak_probe: Option<crate::types::dns::DnsLeakReport>,
    pub is_probing_dns_leak: bool,
    /// Honest per-target report of the last Fake-IP / OS DNS cache flush,
    /// consumed from the shared DNS page read model (DUAL-14-07).
    pub dns_cache_flush: infiltrator_contract::dns::DnsCacheFlushReport,
    pub pcap_state: crate::types::runtime::PcapCaptureState,
    /// Canonical speedtest read model published by the shared application
    /// engine. This replaces the former UI-local fabricated metrics: the view
    /// renders this snapshot, and `RunSpeedtest` intents drive the engine.
    pub speedtest: infiltrator_contract::speedtest::SpeedtestSnapshot,
    /// DUAL-06-13: whether the per-node speedtest detail modal is open. Pure
    /// view state; the modal reads the shared `speedtest` snapshot above.
    pub speedtest_detail_open: bool,
    /// Overview card display order from the shared `OverviewLayoutSnapshot`.
    /// Local render projection of the shared contract; the view assembles its
    /// reorderable cards in this order.
    pub overview_card_order: Vec<infiltrator_contract::overview_layout::OverviewCardKind>,
    pub crash_watchdog: crate::types::doctor::CrashWatchdogState,
    pub log_filter: crate::types::runtime::LogFilterState,
    pub connection_grouping_mode: infiltrator_domain::connection_view::ConnectionGroupingMode,
    /// DUAL-13-11: byte-change tracking that backs idle detection.
    pub connection_activity: infiltrator_domain::connection_activity::ConnectionActivityTracker,
    /// Configured idle timeout in seconds (one of the shared choices).
    pub connection_idle_timeout_secs: u64,
    /// Idle connections identified by the last sweep, if one has run.
    pub last_idle_sweep: Option<usize>,
}

/// 外壳域:导航路由、语言/主题、全局错误与 Toast、托盘、Admin 管理端、
/// 任务计数与 demo 捕获标记(UI-002)。
pub struct ShellState {
    pub current_route: Route,
    pub history: crate::types::app::RouteHistory,
    /// Shared 4-tier responsive viewport projection. Updated from
    /// [`crate::types::message::Message::WindowResized`]; both the sidebar form
    /// and page grids derive from this single source.
    pub viewport: infiltrator_contract::responsive_viewport::ResponsiveViewportSnapshot,
    pub error_msg: Option<String>,
    pub transition: Transition,
    pub lang: String,
    pub tray_controller: Option<Box<dyn TrayController>>,
    pub tray_events: Option<SharedTrayEventReceiver>,
    pub admin_enabled: bool,
    pub admin_port: u16,
    pub admin_port_input: String,
    pub admin_server: crate::admin_server::AdminServerManager,
    pub admin_shared: crate::admin_server::AdminSharedRuntime,
    pub admin_commands: Option<crate::admin_server::SharedAdminCommandReceiver>,
    pub is_admin: bool,
    /// 0.20 OS 系统通知总开关（订阅自动更新 / WebDAV 周期同步 / 内核错误），
    /// 镜像 `AppSettings.notifications_enabled`；关闭时
    /// [`crate::notify`] 零开销短路。
    pub notifications_enabled: bool,
    pub close_to_tray: bool,
    pub system_proxy_bypass: String,
    pub last_task_id: usize,
    /// Cooldown for stream-driven tray refreshes (download/sync progress)
    /// so the D-Bus menu is rebuilt at most once per second.
    pub tray_refresh_cooldown: Option<std::time::Instant>,
    pub toasts: Vec<(String, ToastStatus)>,
    /// Stable ids parallel to [`Self::toasts`]: dismissal is by id, so an
    /// evicted toast's expiry task can never remove a neighbour.
    pub toast_ids: Vec<u64>,
    /// Shared dedup gate (`ToastPolicy`), applied at the single toast
    /// ingestion point.
    pub toast_gate: infiltrator_contract::toast::ToastGate,
    /// Monotonic clock baseline for toast admission timestamps.
    pub toast_epoch: std::time::Instant,
    pub next_toast_id: u64,
    pub confirmation: Option<ConfirmAction>,
    pub is_factory_resetting: bool,
    pub theme: Theme,
    pub demo: bool,
    pub capture_marker: Option<PathBuf>,
    pub capture_marker_written: std::sync::atomic::AtomicBool,
    pub command_palette_open: bool,
    pub command_query: String,
    pub command_selected_index: usize,
    /// Shared command-palette catalogue (DUAL-15-05). Rebuilt from the stored
    /// profile list whenever the palette opens so both surfaces list the same
    /// entries plus the same live profile rows.
    pub command_catalogue: infiltrator_contract::command_catalogue::CommandCatalogue,
    /// Persisted Mini HUD placement (shared geometry, DUAL-15-04).
    pub mini_hud_placement: infiltrator_contract::mini_hud::MiniHudPlacement,
    /// Mini HUD drag anchor: the cursor's widget-local point and the placement
    /// captured when the drag started.
    pub mini_hud_drag_anchor: Option<MiniHudDragAnchor>,
    /// The host's monitor rectangle once resolved, used for clamping and edge
    /// snapping the HUD placement.
    pub mini_hud_display: Option<infiltrator_contract::mini_hud::MiniHudDisplay>,
    /// The host window id once resolved (single-window desktop app), needed
    /// to move/level the Mini HUD window.
    pub window_id: Option<iced::window::Id>,
    pub mini_hud_mode: bool,
    pub always_on_top: bool,
    /// Shared appearance preference (pinned skin or system follow).
    pub theme_preference: infiltrator_contract::theme::ThemePreference,
    /// Latest OS appearance signal (`true` = the OS prefers dark).
    pub system_prefers_dark: bool,
    /// Shared global-shortcut registry (product defaults until settings load).
    pub shortcut_registry: infiltrator_contract::shortcuts::ShortcutRegistry,
    /// Action awaiting the next captured chord, if any.
    pub hotkey_capture: Option<infiltrator_contract::shortcuts::ShortcutAction>,
    pub uwp_loopback: crate::types::app::UwpLoopbackState,
}

/// Drag state for the Mini HUD window: the cursor's widget-local anchor and
/// the placement mirror when the drag began. The actual window move is issued
/// through the host window task; this only tracks the delta math.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MiniHudDragAnchor {
    pub origin: (f32, f32),
    pub placement: infiltrator_contract::mini_hud::MiniHudPlacement,
}

impl ShellState {
    /// Apply an appearance preference: store it and repaint the resolved skin
    /// against the latest OS signal (shared `ThemePreference` resolution).
    pub fn apply_theme_preference(
        &mut self,
        preference: infiltrator_contract::theme::ThemePreference,
    ) {
        self.theme_preference = preference;
        self.theme =
            crate::view::theme::theme_for_skin(preference.resolve(self.system_prefers_dark));
    }
}

pub struct AppState {
    pub runtime: RuntimeState,
    pub profile: ProfileState,
    pub editor: ConfigEditorState,
    pub diag: DiagnosticsState,
    pub shell: ShellState,
    pub app_routing: crate::types::app_routing::AppRoutingState,
    /// Canonical cross-surface snapshot cache. Existing Elm fields are local
    /// render/update projections and must not become a second shared source.
    pub surface: crate::surface::SurfaceModel,
    /// Optional application pump bridge installed by a desktop/mobile
    /// composition root. `None` keeps the existing pull-free test/demo app.
    pub surface_bridge: Option<crate::surface::SurfaceBridge>,
    /// Host-provided process-exit cleanup callback. The UI knows only this
    /// zero-argument seam; OS signal and proxy/TUN details stay in the host.
    pub exit_cleanup: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl AppState {
    /// Rebuild the command catalogue from the stored profile list. Called when
    /// the palette opens and after the profile list changes, so the palette
    /// always lists the live profiles alongside the shared product commands.
    pub fn rebuild_command_catalogue(&mut self) {
        let profiles: Vec<infiltrator_contract::command_catalogue::ProfileChoice> = self
            .profile
            .profiles
            .iter()
            .map(|profile| {
                infiltrator_contract::command_catalogue::ProfileChoice::new(
                    profile.name.clone(),
                    profile.name.clone(),
                )
            })
            .collect();
        self.shell.command_catalogue =
            infiltrator_contract::command_catalogue::CommandCatalogue::with_profiles(&profiles);
    }

    /// Indices of the shared catalogue kept by the current query. The shared
    /// substring rule plus the Iced pinyin matcher; both surfaces derive the
    /// arrow-key order from the same shared catalogue.
    pub fn filtered_command_indices(&self) -> Vec<usize> {
        let catalogue = &self.shell.command_catalogue;
        let query = self.shell.command_query.trim();
        if query.is_empty() {
            return (0..catalogue.len()).collect();
        }
        let lang = infiltrator_shared::locales::Lang(&self.shell.lang);
        catalogue
            .entries()
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry.matches(query)
                    || infiltrator_shared::fuzzy_search::pinyin_fuzzy_match(
                        &infiltrator_shared::locales::Localizer::tr(&lang, entry.title_key),
                        query,
                    )
                    || infiltrator_shared::fuzzy_search::pinyin_fuzzy_match(
                        &infiltrator_shared::locales::Localizer::tr(
                            &lang,
                            entry.category.label_key(),
                        ),
                        query,
                    )
                    || infiltrator_shared::fuzzy_search::pinyin_fuzzy_match(&entry.id, query)
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// The Mini HUD's shared read model, assembled from the live projections:
    /// traffic rates, the controller-reported mode, the selected exit node and
    /// the shared system-toggle snapshot. Nothing here is a constant.
    pub fn mini_hud_read_model(&self) -> infiltrator_contract::mini_hud::MiniHudReadModel {
        let active_node = if !self.runtime.runtime_selected_proxy.is_empty() {
            self.runtime.runtime_selected_proxy.clone()
        } else if let Some(active) = self.profile.profiles.iter().find(|profile| profile.active) {
            active.name.clone()
        } else {
            String::new()
        };
        let mode_zh = self
            .runtime
            .proxy_mode
            .as_deref()
            .and_then(infiltrator_contract::command::ProxyMode::from_wire)
            .map(|mode| {
                infiltrator_shared::locales::Localizer::tr(
                    &infiltrator_shared::locales::Lang(&self.shell.lang),
                    &format!("mode_{}", mode.to_wire()),
                )
                .into_owned()
            })
            .unwrap_or_default();
        let (up, down) = self
            .diag
            .traffic
            .as_ref()
            .map(|traffic| (traffic.up, traffic.down))
            .unwrap_or((0, 0));
        infiltrator_contract::mini_hud::MiniHudReadModel {
            visible: self.shell.mini_hud_mode,
            mode_zh,
            exit_node: active_node,
            down_bytes_per_sec: down,
            up_bytes_per_sec: up,
            system_proxy: self.runtime.system_toggles.system_proxy.clone(),
            tun: self.runtime.system_toggles.tun.clone(),
            placement: self.shell.mini_hud_placement,
        }
    }

    /// Move an Overview card up/down using the shared layout operators, then
    /// store the resulting order. Reusing `OverviewLayoutSnapshot` guarantees
    /// the Iced surface applies the exact same swap semantics as Bevy.
    pub fn move_overview_card(
        &mut self,
        kind: infiltrator_contract::overview_layout::OverviewCardKind,
        up: bool,
    ) {
        let mut layout = infiltrator_contract::overview_layout::OverviewLayoutSnapshot::new(
            self.diag.overview_card_order.clone(),
        );
        let changed = if up {
            layout.move_up(kind)
        } else {
            layout.move_down(kind)
        };
        if changed {
            self.diag.overview_card_order = layout.order;
        }
    }

    /// Apply the shared page read model as a monotonic render cache. The
    /// existing Elm fields remain toolkit-local projections; stale host
    /// events cannot overwrite a newer shared revision.
    pub fn apply_shared_surface_snapshot(
        &mut self,
        snapshot: infiltrator_contract::surface_snapshot::SurfaceSnapshot,
    ) -> bool {
        if !self.surface.apply(snapshot.clone()) {
            return false;
        }
        self.runtime.proxy_mode = snapshot
            .core
            .proxy_mode
            .map(|mode| mode.to_wire().to_owned());
        self.runtime.status = RuntimeStatus::from_core_snapshot(&snapshot.core);
        self.runtime.core_lifecycle = snapshot.core.lifecycle_snapshot();
        self.runtime.mtu = snapshot.mtu.clone();
        if let Some(settings) = snapshot.pages.settings.data.as_ref() {
            self.runtime.ipv6_routing = settings.ipv6_routing;
        }
        self.runtime.system_proxy = snapshot.system_proxy.clone();
        self.runtime.system_proxy_recovery = snapshot.system_proxy_recovery.clone();
        self.runtime.network_roaming = snapshot.network_roaming.clone();
        self.runtime.vpn = snapshot.vpn.clone();
        self.runtime.privileged_network = snapshot.privileged_network.clone();
        self.runtime.traffic_waveform = snapshot.traffic_waveform.clone();
        self.runtime.traffic_scale = snapshot.traffic_scale.clone();
        self.runtime.traffic_topology = snapshot.traffic_topology.clone();
        self.runtime.reconnect_mask = snapshot.reconnect_mask.clone();
        self.runtime.active_exit = snapshot.active_exit.clone();
        self.runtime.subscription_quota = snapshot.subscription_quota.clone();
        self.diag.speedtest = snapshot.speedtest.clone();
        if let Some(dns) = snapshot.pages.dns.data.as_ref() {
            // Canonical cross-surface DNS workbench input: re-seed the form
            // from the shared read model only while the user has no unsaved
            // edits, so a poll never clobbers typing.
            if !self.editor.dns_form_dirty && !self.editor.dns_json_dirty {
                self.editor.dns_form =
                    infiltrator_contract::dns_form::DnsWorkbenchForm::from_snapshot(dns);
            }
            // DUAL-14-06: the observed Fake-IP bindings are a shared fact.
            self.editor.dns_fake_ip_pool = dns.fake_ip_pool.clone();
            // DUAL-14-10: the latency-probe availability is a shared fact.
            self.editor.dns_latency = dns.latency;
            // DUAL-14-11: re-seed the hosts draft while it has no pending edit.
            if !self.editor.dns_hosts_dirty && !self.editor.is_saving_dns_hosts {
                self.editor.dns_hosts = dns.hosts.clone();
            }
            // The read model carries the honest last flush report for the
            // shared host application; a local report from this session is
            // never downgraded back to `NotRequested`.
            if dns.cache_flush.is_requested() {
                self.diag.dns_cache_flush = dns.cache_flush.clone();
            }
        }
        if let Some(rules_page) = snapshot.pages.rules.data.as_ref() {
            self.editor.rule_hit_audit.audit = rules_page.tracer.hit_audit.clone();
            // DUAL-12-08: consume the shared reverse-apply facts; the chooser
            // is gated and seeded from the surface read model, not guessed.
            self.editor.rules_tracer_can_reverse_apply = rules_page.tracer.can_reverse_apply;
            self.editor.rules_tracer_suggested_target =
                rules_page.tracer.suggested_override_target.clone();
            // DUAL-11-03 / 11-04: the MRS acceleration read model and the
            // declared provider source URLs are shared facts, never UI-local.
            self.editor.mrs_acceleration = rules_page.mrs_acceleration.clone();
            self.editor.rule_provider_source_urls = rules_page
                .providers
                .iter()
                .filter_map(|provider| {
                    provider
                        .source_url
                        .as_ref()
                        .map(|url| (provider.name.clone(), url.clone()))
                })
                .collect();
            self.editor.rule_provider_intervals = rules_page
                .providers
                .iter()
                .filter_map(|provider| {
                    provider
                        .refresh_interval_secs
                        .map(|secs| (provider.name.clone(), secs))
                })
                .collect();
            // DUAL-11-08: the publish cap and the omitted count are shared
            // facts; the editor keeps its full profile list and says so.
            self.editor.rule_publish_limit = rules_page.rule_publish_limit;
            self.editor.rule_publish_omitted = rules_page
                .is_truncated()
                .then(|| rules_page.omitted_rule_count());
        }
        self.diag.overview_card_order = snapshot.overview_layout.order.clone();
        self.runtime.system_toggles =
            infiltrator_application::system_toggle_application::SystemToggleApplication::from_surface(
                &snapshot,
            );
        if matches!(
            &snapshot.system_proxy.status,
            infiltrator_contract::system_proxy::SystemProxyStatus::Enabled
                | infiltrator_contract::system_proxy::SystemProxyStatus::Disabled
        ) {
            self.runtime.system_proxy_enabled = snapshot.system_proxy.is_enabled();
        }
        self.runtime.runtime_generation = snapshot.core.generation;
        self.runtime.core_session_token = snapshot.core.session_token;
        self.runtime.core_versions = snapshot.versions.clone();
        self.runtime.core_integrity = snapshot.versions.verification.clone();
        if let Some(settings) = snapshot.pages.settings.data.as_ref() {
            self.editor.tun_auto_route = settings.tun_auto_route;
            self.editor.tun_strict_route = settings.tun_strict_route;
            let mut committed = self.runtime.lan_sharing_committed.clone();
            committed.allow_lan = settings.allow_lan;
            committed.mixed_port = settings.mixed_port;
            committed.bind_address = settings.lan_bind_address.clone();
            self.runtime.lan_sharing_committed = committed.clone();
            if !self.runtime.lan_sharing_dirty {
                self.runtime.lan_sharing = committed;
            }

            let mut security_committed = self.runtime.lan_security_committed.clone();
            security_committed.allowed_ips = settings.lan_security.allowed_ips.join(", ");
            security_committed.disallowed_ips = settings.lan_security.disallowed_ips.join(", ");
            security_committed.skip_auth_prefixes =
                settings.lan_security.skip_auth_prefixes.join(", ");
            security_committed.authentication_enabled =
                settings.lan_security.authentication_enabled;
            security_committed.authentication_user_count =
                settings.lan_security.authentication_user_count;
            if let Some(username) = settings.lan_security.authentication_username.as_ref() {
                security_committed.auth_username = username.clone();
            }
            security_committed.auth_password.clear();
            self.runtime.lan_security_committed = security_committed.clone();
            if !self.runtime.lan_security_dirty {
                self.runtime.lan_security = security_committed;
                self.runtime.lan_sharing.acl_whitelist_cidrs =
                    settings.lan_security.allowed_ips.join(", ");
            }
        }
        if !self.shell.uwp_loopback.is_scanning
            && let Some(app_routing) = snapshot.pages.app_routing.data.as_ref()
        {
            let uwp = &app_routing.uwp_loopback;
            self.shell.uwp_loopback.availability = uwp.availability.clone();
            self.shell.uwp_loopback.revision = uwp.revision;
            self.shell.uwp_loopback.apps = uwp
                .packages
                .iter()
                .map(|package| crate::types::app::UwpAppItem {
                    sid: package.sid.clone(),
                    display_name: package.display_name.clone(),
                    is_exempt: package.loopback_exempt,
                })
                .collect();
            self.shell.uwp_loopback.is_scanning = false;
        }
        if let Some(settings) = snapshot.pages.settings.data.as_ref() {
            let pac = &mut self.runtime.pac_manager;
            pac.snapshot = settings.pac.clone();
            if !pac.dirty {
                pac.bypass_subnets = settings.pac.bypass_domains.join(", ");
                match &settings.pac.state {
                    infiltrator_contract::pac::PacServiceState::Running { url } => {
                        pac.is_pac_mode_active = true;
                        pac.pac_url = url.clone();
                    }
                    infiltrator_contract::pac::PacServiceState::Disabled
                    | infiltrator_contract::pac::PacServiceState::Unavailable { .. } => {
                        pac.is_pac_mode_active = false;
                        pac.pac_url.clear();
                    }
                }
            }
        }
        self.runtime.controller_auth = snapshot.controller_auth;
        self.runtime.service_mode = snapshot.service_mode;
        self.runtime.port_conflicts = snapshot.port_conflicts.clone();
        self.runtime.core_resources = snapshot.resources.clone();
        self.runtime.offline_startup = snapshot.offline_startup.clone();
        self.diag.crash_watchdog.shared = snapshot.core.watchdog.clone();
        self.diag.crash_watchdog.last_crash_summary = snapshot
            .core
            .watchdog
            .last_error
            .as_ref()
            .map(|failure| crate::utils::sanitize_ui_text(&failure.message));
        self.diag.traffic = Some(infiltrator_domain::runtime::TrafficData {
            up: snapshot.core.upload_bps.max(0.0) as u64,
            down: snapshot.core.download_bps.max(0.0) as u64,
        });
        self.diag.memory =
            snapshot
                .core
                .memory_bytes
                .map(|in_use| infiltrator_domain::runtime::MemoryData {
                    in_use,
                    os_limit: 0,
                });
        true
    }

    /// Attach the host-composed shared surface input before the Iced runtime
    /// starts. The update loop only receives typed `Message` values afterward.
    pub fn attach_surface_bridge(&mut self, bridge: crate::surface::SurfaceBridge) {
        self.surface_bridge = Some(bridge);
    }

    pub fn attach_exit_cleanup(&mut self, cleanup: Arc<dyn Fn() + Send + Sync>) {
        self.exit_cleanup = Some(cleanup);
    }

    /// Single choke point for `error_msg`: raw error chains can embed
    /// subscription query tokens or the controller secret, so the text is
    /// redacted here before any view can render it (CORE-001).
    pub fn set_error(&mut self, source: impl std::fmt::Display) {
        self.shell.error_msg = Some(crate::utils::sanitize_ui_text(&source.to_string()));
    }

    /// DUAL-09-12: the write classification of the profile open in the editor.
    /// This reads the same shared metadata the application guard enforces, so
    /// the banner, the disabled save button and the guard cannot disagree.
    pub fn edited_profile_write_protection(
        &self,
    ) -> infiltrator_contract::profile_protection::ProfileWriteProtection {
        use infiltrator_contract::profile_protection::ProfileWriteProtection;
        let Some(name) = self
            .editor
            .editor_path
            .as_ref()
            .and_then(|path| path.file_stem())
            .and_then(|stem| stem.to_str())
        else {
            return ProfileWriteProtection::Editable;
        };
        self.profile
            .profiles
            .iter()
            .find(|profile| profile.name == name)
            .map(|profile| {
                ProfileWriteProtection::from_subscription_url(
                    profile.subscription_url.as_deref().unwrap_or_default(),
                )
            })
            .unwrap_or(ProfileWriteProtection::Editable)
    }
}
