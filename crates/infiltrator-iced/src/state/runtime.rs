//! Runtime owner.
use crate::state::core_operations::{CoreDownloadState, CoreUpdateCheckState};
use crate::state::outbound_inventory::OutboundGroupView;
use crate::state::proxy_preferences::ProxyPreferenceState;
use crate::state::proxy_probe::{OutboundProbeActionAvailability, OutboundProbeBatchState};

use crate::state::proxy_search::ProxySearchState;
use crate::types::app::{LanSecurityConfig, LanSharingConfig, PacManagerConfig};
use crate::types::dns::TunStackConfig;
use crate::types::runtime::{ConfigurationSaveState, RuntimePatchSnapshot, RuntimeStatus};
use infiltrator_application::system_proxy_application::SystemProxyApplication;
use infiltrator_contract::active_exit::ActiveExitSnapshot;
use infiltrator_contract::controller::ControllerAuthSnapshot;
use infiltrator_contract::core_control::CoreControlAction;
use infiltrator_contract::ipv6::Ipv6RoutingSnapshot;
use infiltrator_contract::mtu::MtuNegotiationSnapshot;
use infiltrator_contract::network_roaming::NetworkRoamingSnapshot;
use infiltrator_contract::offline_startup::OfflineStartupSnapshot;
use infiltrator_contract::port_conflict::PortConflictSnapshot;
use infiltrator_contract::privileged_network::PrivilegedNetworkSnapshot;
use infiltrator_contract::protocol_fidelity::ProtocolStudioSnapshot;
use infiltrator_contract::reconnect_mask::ReconnectMaskSnapshot;
use infiltrator_contract::resources::CoreResourceSnapshot;
use infiltrator_contract::runtime_control::RuntimeControlSnapshot;
use infiltrator_contract::runtime_settings::RuntimeDelaySortOrder;
use infiltrator_contract::service_mode::ServiceModeSnapshot;
use infiltrator_contract::subscription_quota::SubscriptionQuotaSnapshot;
use infiltrator_contract::surface_snapshot::PageData;
use infiltrator_contract::system_proxy::{SystemProxyRecoverySnapshot, SystemProxySnapshot};
use infiltrator_contract::system_toggle::SystemToggleSnapshot;
use infiltrator_contract::traffic_scale::TrafficScaleSnapshot;
use infiltrator_contract::traffic_topology::TrafficTopologySnapshot;
use infiltrator_contract::traffic_waveform::TrafficWaveformSnapshot;
use infiltrator_contract::version::{
    CoreArtifactVerification, CoreVersionSnapshot, InstalledCoreVersion,
};
use infiltrator_contract::vpn::{VpnSessionSnapshot, VpnSurfaceObservation};
use infiltrator_ports::host_runtime::{HostRuntime, TunServiceStatus};
use infiltrator_ports::privileged_network::PrivilegedNetworkPort;
use infiltrator_ports::rule_provider_cache::RuleProviderCachePort;
use infiltrator_ports::system_proxy::SystemProxyPort;
use infiltrator_presentation::protocol_form::ProtocolInputs;
use infiltrator_presentation::runtime_probe_settings_editor::RuntimeProbeSettingsEditor;
use lurker_core_contract::error::Failure;
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::lifecycle::CoreLifecycleSnapshot;
use lurker_core_contract::outbound::{
    details::OutboundDetailsSnapshot,
    identity::OutboundId,
    inventory::{OutboundInventorySnapshot, OutboundProbeSnapshot},
    preferences::{OutboundSortOrder, OutboundUiPreferencesSnapshot},
    probe::OutboundSelectionRequest,
};
use lurker_core_contract::routing_intent::{RoutingIntent, RoutingIntentSnapshot};
use lurker_core_contract::session::SessionToken;
use lurker_ui_presentation::proxy_group_order_editor::ProxyGroupOrderEditor;
use lurker_ui_presentation::routing_intent_actions::RoutingIntentActions;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use zhuge_ui_core::text::SearchTextRun;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TunServiceInstallState {
    #[default]
    Idle,
    Installing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CustomNodeSaveState {
    #[default]
    Idle,
    Saving,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProbeSettingsDialogState {
    #[default]
    Closed,
    Editing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProxyGroupOrderDialogState {
    #[default]
    Closed,
    Editing,
}

/// 内核运行时域:内核进程句柄、生命周期状态、路由意图/系统代理/自启、
/// 节点与分组运行控制、批量测速与内核版本管理(UI-002)。
pub struct RuntimeState {
    pub runtime: Option<Arc<dyn HostRuntime>>,
    pub host_composition_failure: Option<Failure>,
    pub runtime_generation: u64,
    /// Session identity paired with `runtime_generation`; delayed UI results
    /// must match both before changing a projection.
    pub core_session_token: Option<SessionToken>,
    /// Exact shared lifecycle read model; `status` remains a coarse legacy
    /// rendering compatibility field for existing Iced widgets.
    pub core_lifecycle: CoreLifecycleSnapshot,
    pub mtu: MtuNegotiationSnapshot,
    pub ipv6_routing: Ipv6RoutingSnapshot,
    pub vpn: VpnSessionSnapshot,
    /// Owner-authored VPN observation from the canonical cross-surface read model.
    pub vpn_observation: VpnSurfaceObservation,
    /// Shared control state used by sidebar and settings quick toggles.
    pub system_toggles: SystemToggleSnapshot,
    pub privileged_network: PrivilegedNetworkSnapshot,
    pub privileged_network_port: Option<Arc<dyn PrivilegedNetworkPort>>,
    /// DUAL-11-06/07: the kernel's local rule-provider files, mirrored from
    /// the host runtime so the unpack/purge handlers never touch a path the
    /// host did not hand them.
    pub rule_provider_cache_port: Option<Arc<dyn RuleProviderCachePort>>,
    pub traffic_waveform: TrafficWaveformSnapshot,
    pub traffic_scale: TrafficScaleSnapshot,
    pub traffic_topology: TrafficTopologySnapshot,
    /// Core reload/reconnect graceful degradation mask shared with Bevy; the
    /// overview banner renders it while the core restarts or reconnects.
    pub reconnect_mask: ReconnectMaskSnapshot,
    pub active_exit: ActiveExitSnapshot,
    pub subscription_quota: SubscriptionQuotaSnapshot,
    pub system_proxy: SystemProxySnapshot,
    pub system_proxy_recovery: SystemProxyRecoverySnapshot,
    /// Retained independently of the running core so a system proxy can be
    /// disabled during shutdown/cleanup without a live Mihomo runtime.
    pub system_proxy_port: Option<Arc<dyn SystemProxyPort>>,
    pub system_proxy_application: Option<SystemProxyApplication>,
    pub system_proxy_last_repair_count: u64,
    pub lifecycle_token: u64,
    pub lifecycle_pending: Option<CoreControlAction>,
    pub lifecycle_failure: Option<Failure>,
    pub status: RuntimeStatus,
    /// Canonical engine-neutral inventory used by the Iced proxy page.
    pub outbound_inventory: PageData<OutboundInventorySnapshot>,
    /// Typed metadata aligned with the inventory scope for details, health and probing.
    pub outbound_details: PageData<OutboundDetailsSnapshot>,
    /// Engine-scoped preferences observed from the shared owner.
    pub outbound_ui_preferences: PageData<OutboundUiPreferencesSnapshot>,
    pub outbound_inspecting: Option<OutboundId>,
    pub outbound_probe_result: Option<OutboundProbeSnapshot>,
    pub outbound_probe_pending: Option<u64>,
    pub outbound_probe_failure: Option<Failure>,
    pub outbound_probe_blocked: Option<OutboundProbeActionAvailability>,
    pub outbound_probe_token: u64,
    pub outbound_probe_batch: OutboundProbeBatchState,
    pub outbound_probe_batch_token: u64,
    pub filtered_groups: Vec<(String, Vec<String>)>,
    pub(crate) proxy_groups: Vec<OutboundGroupView>,
    pub proxy_name_runs: BTreeMap<String, Vec<SearchTextRun>>,
    pub proxy_search: ProxySearchState,
    pub proxy_preferences: ProxyPreferenceState,
    pub proxy_filter: String,
    /// Product outbound page sorting comes from the typed Core preference contract.
    pub outbound_sort_order: OutboundSortOrder,
    /// Runtime diagnostic ordering, separate from outbound-page preferences.
    pub runtime_panel_delay_sort_order: RuntimeDelaySortOrder,
    pub runtime_probe_url_draft: String,
    pub runtime_probe_settings_editor: RuntimeProbeSettingsEditor,
    pub probe_settings_dialog: ProbeSettingsDialogState,
    pub runtime_probe_timeout_draft: String,
    pub runtime_selected_group: String,
    pub runtime_selected_proxy: String,
    pub runtime_outbound_selection: Option<(CoreScope, OutboundSelectionRequest)>,
    pub runtime_connection_filter: String,
    pub runtime_connection_sort: String,
    pub runtime_prev_upload_total: Option<u64>,
    pub runtime_prev_download_total: Option<u64>,
    pub runtime_prev_snapshot_at: Option<Instant>,
    pub pending_runtime_patch: Option<RuntimePatchSnapshot>,
    pub runtime_patch_token: u64,
    pub runtime_auto_refresh: bool,
    pub runtime_poll_tick: u64,
    pub routing_intent: Option<RoutingIntent>,
    pub routing_intent_state: RoutingIntentSnapshot,
    pub routing_intent_actions: RoutingIntentActions,
    pub runtime_control: RuntimeControlSnapshot,
    pub tun_enabled: Option<bool>,
    pub tun_service_status: Option<TunServiceStatus>,
    pub tun_service_install_state: TunServiceInstallState,
    pub system_proxy_enabled: bool,
    pub system_proxy_pending: bool,
    pub autostart_enabled: bool,
    pub filter_alive_only: bool,
    pub favorite_proxies: HashSet<String>,
    pub proxy_compact_view: bool,
    pub is_adding_custom_node: bool,
    pub new_node_type: String,
    pub new_node_name: String,
    pub new_node_server: String,
    pub new_node_port: String,
    pub new_node_credential: String,
    pub new_node_cipher: String,
    pub new_node_tls: bool,
    pub installed_kernels: Vec<InstalledCoreVersion>,
    pub core_channel: String,
    /// Shared online probe result for Stable, Alpha and Meta-Core.
    pub core_versions: CoreVersionSnapshot,
    pub core_integrity: CoreArtifactVerification,
    pub controller_auth: ControllerAuthSnapshot,
    pub service_mode: ServiceModeSnapshot,
    pub port_conflicts: PortConflictSnapshot,
    pub core_resources: CoreResourceSnapshot,
    pub offline_startup: OfflineStartupSnapshot,
    pub core_download: CoreDownloadState,
    pub core_update_check: CoreUpdateCheckState,
    pub configuration_save_state: ConfigurationSaveState,
    pub group_order_editor: ProxyGroupOrderEditor,
    pub group_order_dialog: ProxyGroupOrderDialogState,
    pub custom_node_modal_open: bool,
    pub custom_node_uri_input: String,
    /// DUAL-05: the shared protocol studio snapshot (typed draft, derived
    /// report, URI preview and the empirically computed URI fidelity gaps).
    /// The modal is a pure projection of this; Iced keeps no second protocol
    /// fact source.
    pub custom_node_studio: ProtocolStudioSnapshot,
    pub custom_node_inputs: ProtocolInputs,
    pub custom_node_save_state: CustomNodeSaveState,
    pub network_roaming: NetworkRoamingSnapshot,
    pub pac_manager: PacManagerConfig,
    pub lan_sharing: LanSharingConfig,
    pub lan_sharing_committed: LanSharingConfig,
    pub lan_sharing_dirty: bool,
    pub lan_security: LanSecurityConfig,
    pub lan_security_committed: LanSecurityConfig,
    pub lan_security_dirty: bool,
    pub tun_stack_config: TunStackConfig,
}
