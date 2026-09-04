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
    AdvancedEditMode, AdvancedValidationState, DnsFormDraft, DnsTab, FakeIpFormDraft, TunFormDraft,
};
use crate::types::editor::EditorLazyState;
use crate::types::perf::PerfSnapshot;
use crate::types::rules::{RuleRenderItem, RulesJsonTab, RulesTab};
use crate::types::runtime::{
    RebuildFlowState, RuntimePatchSnapshot, RuntimeStatus, RuntimeStreamState,
};
use iced::Theme;
use iced::widget::text_editor;
use infiltrator_core::rules::RuleEntry;
use infiltrator_desktop::runtime::MihomoRuntime;
use infiltrator_desktop::tun_service::ServiceModeStatus;
use mihomo_api::types::{ConnectionSnapshot, TrafficData};
use mihomo_config::profile::Profile;
use mihomo_version::manager::VersionInfo;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
/// 内核运行时域:内核进程句柄、生命周期状态、代理模式/系统代理/自启、
/// 节点与分组运行控制、批量测速与内核版本管理(UI-002)。
pub struct RuntimeState {
    pub runtime: Option<Arc<MihomoRuntime>>,
    pub runtime_generation: u64,
    pub lifecycle_token: u64,
    pub status: RuntimeStatus,
    pub proxies: HashMap<String, mihomo_api::proxy::types::Proxy>,
    pub is_loading_proxies: bool,
    pub filtered_groups: Vec<(String, Vec<String>)>,
    pub proxy_filter: String,
    pub proxy_sort_by_delay: bool,
    pub proxy_delay_sort: String,
    pub runtime_delay_test_url: String,
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
    pub tun_service_status: Option<ServiceModeStatus>,
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
    pub installed_kernels: Vec<VersionInfo>,
    pub latest_core_version: Option<String>,
    pub core_channel: String,
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
    pub custom_node_name_input: String,
    pub custom_node_server_input: String,
    pub custom_node_port_input: String,
    pub custom_node_type_input: String,
    pub custom_node_uuid_input: String,
    pub custom_node_sni_input: String,
    pub custom_node_exported_uri: Option<String>,
    pub network_roaming: crate::types::runtime::NetworkRoamingState,
    pub pac_manager: crate::types::app::PacManagerConfig,
    pub latency_radar: crate::types::runtime::LatencyRadarState,
    pub apply_guard: crate::types::runtime::ApplyTransactionGuardState,
    pub lan_sharing: crate::types::app::LanSharingConfig,
    pub tun_stack_config: crate::types::dns::TunStackConfig,
}

/// 订阅与档案域:Profile 列表、订阅导入/更新、WebDAV 同步与应用设置保存(UI-002)。
pub struct ProfileState {
    pub profiles: Vec<Profile>,
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
    pub subscription_user_agent: String,
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
    pub aggregator_result_summary: Option<String>,
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
    pub rules_tracer_result: Option<(usize, String, String)>,
    pub rules_providers_expanded: bool,
    pub rules_render_cache: Vec<RuleRenderItem>,
    pub rules_filtered_indices: Vec<usize>,
    pub rules_heavy_ready: bool,
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
    pub dns_form: DnsFormDraft,
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
    pub proxy_providers: Vec<mihomo_api::types::ProxyProvider>,
    pub rule_providers: Vec<mihomo_api::types::RuleProvider>,
    pub is_loading_providers: bool,
    pub script_sandbox: crate::types::editor::ScriptSandboxState,
    pub snapshot_diff_modal_open: bool,
    pub snapshot_diff_selected_id: Option<String>,
    pub subrule_draft: crate::types::rules::SubRuleDraft,
    pub geodata_status: crate::types::editor::GeoDataStatus,
    pub rule_hit_audit: crate::types::rules::RuleHitAuditState,
    pub provider_unpack: crate::types::rules::ProviderUnpackState,
    pub dns_nameservers: Vec<String>,
    pub dns_fallback_servers: Vec<String>,
    pub dns_enhanced_mode: String,
    pub is_saving_dns: bool,
    pub is_saving_fake_ip: bool,
    pub is_saving_tun: bool,
    pub editor_content: text_editor::Content,
    pub editor_path: Option<PathBuf>,
    pub editor_path_setting: String,
    pub profile_snapshots: Vec<infiltrator_core::history::SnapshotMeta>,
    pub is_loading_snapshots: bool,
    pub is_restoring_snapshot: bool,
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
    pub inspecting_rule_provider_diff: Option<infiltrator_core::rules::RuleProviderDiff>,
    pub is_loading_rule_provider_diff: bool,
}

/// 诊断域:流量/内存/连接/日志运行态快照与性能 HUD 测量(UI-002)。
pub struct DiagnosticsState {
    pub traffic: Option<TrafficData>,
    pub traffic_history: VecDeque<(u64, u64)>,
    pub memory: Option<mihomo_api::types::MemoryData>,
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
    pub pcap_state: crate::types::runtime::PcapCaptureState,
    pub speedtest_result: crate::types::perf::SpeedtestResult,
    pub crash_watchdog: crate::types::doctor::CrashWatchdogState,
    pub log_filter: crate::types::runtime::LogFilterState,
    pub connection_grouping_mode: crate::types::runtime::ConnectionGroupingMode,
}

/// 外壳域:导航路由、语言/主题、全局错误与 Toast、托盘、Admin 管理端、
/// 任务计数与 demo 捕获标记(UI-002)。
pub struct ShellState {
    pub current_route: Route,
    pub history: crate::types::app::RouteHistory,
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
    pub confirmation: Option<ConfirmAction>,
    pub is_factory_resetting: bool,
    pub theme: Theme,
    pub demo: bool,
    pub capture_marker: Option<PathBuf>,
    pub capture_marker_written: std::sync::atomic::AtomicBool,
    pub command_palette_open: bool,
    pub command_query: String,
    pub command_selected_index: usize,
    pub mini_hud_mode: bool,
    pub always_on_top: bool,
    pub hotkeys_config: Vec<crate::types::app::HotkeyBinding>,
    pub uwp_loopback: crate::types::app::UwpLoopbackState,
}

pub struct AppState {
    pub runtime: RuntimeState,
    pub profile: ProfileState,
    pub editor: ConfigEditorState,
    pub diag: DiagnosticsState,
    pub shell: ShellState,
    pub app_routing: crate::types::app_routing::AppRoutingState,
}

impl AppState {
    /// Single choke point for `error_msg`: raw error chains can embed
    /// subscription query tokens or the controller secret, so the text is
    /// redacted here before any view can render it (CORE-001).
    pub fn set_error(&mut self, source: impl std::fmt::Display) {
        self.shell.error_msg = Some(crate::utils::sanitize_ui_text(&source.to_string()));
    }
}
