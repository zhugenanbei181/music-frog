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
use infiltrator_domain::profiles::ProfileInfo;
use infiltrator_domain::proxy::Proxy;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_domain::runtime::{
    ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider, TrafficData,
};
use infiltrator_domain::snapshots::SnapshotMeta;
use infiltrator_ports::host_runtime::{HostRuntime, TunServiceStatus};
use infiltrator_contract::version::{
    CoreArtifactVerification, CoreVersionSnapshot, InstalledCoreVersion,
};
use infiltrator_contract::controller::ControllerAuthSnapshot;
use infiltrator_contract::service_mode::ServiceModeSnapshot;
use infiltrator_contract::port_conflict::PortConflictSnapshot;
use infiltrator_contract::resources::CoreResourceSnapshot;
use infiltrator_contract::offline_startup::OfflineStartupSnapshot;
use infiltrator_contract::snapshot::CoreLifecycleSnapshot;
use infiltrator_contract::mtu::MtuNegotiationSnapshot;
use infiltrator_contract::system_proxy::SystemProxySnapshot;
use infiltrator_contract::system_proxy::SystemProxyRecoverySnapshot;
use infiltrator_contract::system_toggle::SystemToggleSnapshot;
use infiltrator_contract::privileged_network::PrivilegedNetworkSnapshot;
use infiltrator_ports::privileged_network::PrivilegedNetworkPort;
use infiltrator_ports::system_proxy::SystemProxyPort;
use infiltrator_application::system_proxy_application::SystemProxyApplication;
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
    pub active_exit: infiltrator_contract::active_exit::ActiveExitSnapshot,
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
    pub proxy_providers: Vec<ProxyProvider>,
    pub rule_providers: Vec<RuleProvider>,
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
    pub profile_snapshots: Vec<SnapshotMeta>,
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
        self.runtime.proxy_mode = snapshot.core.proxy_mode.map(|mode| mode.to_wire().to_owned());
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
        self.runtime.active_exit = snapshot.active_exit.clone();
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
        self.diag.memory = snapshot
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
}
