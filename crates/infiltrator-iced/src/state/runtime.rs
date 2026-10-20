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
use lurker_core_contract::page::PageData;
use lurker_core_contract::routing_intent::{RoutingIntent, RoutingIntentSnapshot};
use lurker_core_contract::session::SessionToken;
use lurker_ui_presentation::proxy_group_order_editor::ProxyGroupOrderEditor;
use lurker_ui_presentation::routing_intent_actions::RoutingIntentActions;
use std::collections::{BTreeMap, HashSet};
use std::mem;
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

/// The single typed stage of the shared core lifecycle command.
///
/// The command used to be encoded by two independent options
/// (`lifecycle_pending` and `lifecycle_failure`), so "a command is in flight"
/// and "the last command failed" could be combined into a state the product
/// never meant to show. One stage makes the illegal combination
/// unrepresentable.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CoreLifecycleStage {
    /// No lifecycle command is in flight and the last one succeeded.
    #[default]
    Idle,
    /// A lifecycle command is in flight for the given action.
    Running { action: CoreControlAction },
    /// The last lifecycle command failed.
    Failed { failure: Failure },
}

impl CoreLifecycleStage {
    /// Whether a lifecycle command is currently in flight.
    pub const fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }

    /// The failure of the last lifecycle command, when it failed.
    pub fn failure(&self) -> Option<&Failure> {
        match self {
            Self::Failed { failure } => Some(failure),
            Self::Idle | Self::Running { .. } => None,
        }
    }
}

/// Why a lifecycle transition was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreLifecycleRefusal {
    /// A lifecycle command is still in flight for the given action.
    AlreadyRunning { action: CoreControlAction },
    /// No lifecycle command is in flight for the given action.
    NotRunning { action: CoreControlAction },
}

/// The single typed stage of an in-flight runtime configuration patch.
///
/// The patch used to be encoded by an `Option<RuntimePatchSnapshot>` whose
/// name read as a stage marker, so "a patch is in flight" and "no patch is in
/// flight" shared one field. One stage keeps the restore snapshot attached to
/// the running patch instead of leaving it in a separate optional slot.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum RuntimePatchStage {
    /// No runtime configuration patch is in flight.
    #[default]
    Idle,
    /// A runtime configuration patch is in flight; `snapshot` restores the
    /// previous values when the patch is rejected.
    Applying { snapshot: RuntimePatchSnapshot },
}

impl RuntimePatchStage {
    /// Whether a runtime configuration patch is currently in flight.
    pub const fn is_applying(&self) -> bool {
        matches!(self, Self::Applying { .. })
    }
}

/// Why a runtime patch transition was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimePatchRefusal {
    /// A runtime configuration patch is still in flight.
    AlreadyApplying,
}

/// The single typed stage of the tracked outbound inspection probe.
///
/// The probe used to be encoded by three independent options, so "a probe is
/// in flight", "the last probe failed" and "probing is unavailable" could be
/// combined into states the product never meant to show. One stage keeps the
/// exclusive stages exclusive: a running probe never carries a failure and a
/// blocked probe never runs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum OutboundProbeStage {
    /// No probe is in flight and the last one did not fail.
    #[default]
    Idle,
    /// A probe command is in flight; `token` matches the in-flight command.
    Running { token: u64 },
    /// The last probe failed.
    Failed { failure: Failure },
    /// Probing is unavailable for the inspected outbound. `failure` keeps the
    /// failure of the interrupted attempt, so it is shown again when the
    /// availability recovers.
    Blocked {
        availability: OutboundProbeActionAvailability,
        failure: Option<Failure>,
    },
}

impl OutboundProbeStage {
    /// Whether a probe command is currently in flight.
    pub const fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }

    /// The failure of the last probe attempt, when it failed.
    pub fn failure(&self) -> Option<&Failure> {
        match self {
            Self::Failed { failure } => Some(failure),
            Self::Blocked { failure, .. } => failure.as_ref(),
            Self::Idle | Self::Running { .. } => None,
        }
    }

    /// The availability that blocks probing, when the probe is blocked.
    pub const fn blocked_availability(&self) -> Option<OutboundProbeActionAvailability> {
        match self {
            Self::Blocked { availability, .. } => Some(*availability),
            Self::Idle | Self::Running { .. } | Self::Failed { .. } => None,
        }
    }
}

/// Why an outbound probe transition was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutboundProbeRefusal {
    /// A probe command is already in flight.
    AlreadyRunning { token: u64 },
    /// No probe command is in flight for the given token.
    NotRunning { token: u64 },
}

/// Why a system proxy transition was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemProxyRefusal {
    /// A system proxy change is already in flight.
    AlreadyPending,
}

/// The single typed stage of the Iced system-proxy toggle.
///
/// The toggle used to be encoded by two independent booleans, so "the proxy
/// is enabled" and "a change is in flight" could be combined into a state the
/// product never meant to show, and the rejected change had to guess the
/// previous value by flipping the flag. One stage keeps the four legal stages
/// exclusive and makes every move a typed transition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SystemProxyStage {
    /// The system proxy is disabled and no change is in flight.
    #[default]
    Disabled,
    /// The system proxy is enabled and no change is in flight.
    Enabled,
    /// A change that enables the system proxy is in flight.
    Enabling,
    /// A change that disables the system proxy is in flight.
    Disabling,
}

impl SystemProxyStage {
    /// The stage of a settled system proxy.
    pub const fn from_enabled(enabled: bool) -> Self {
        if enabled { Self::Enabled } else { Self::Disabled }
    }

    /// Whether the system proxy is on, including while the change that turns
    /// it on is in flight.
    pub const fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled | Self::Enabling)
    }

    /// Whether a system proxy change is in flight.
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Enabling | Self::Disabling)
    }
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
    /// The single typed stage of the shared core lifecycle command.
    pub lifecycle: CoreLifecycleStage,
    pub status: RuntimeStatus,
    /// Canonical engine-neutral inventory used by the Iced proxy page.
    pub outbound_inventory: PageData<OutboundInventorySnapshot>,
    /// Typed metadata aligned with the inventory scope for details, health and probing.
    pub outbound_details: PageData<OutboundDetailsSnapshot>,
    /// Engine-scoped preferences observed from the shared owner.
    pub outbound_ui_preferences: PageData<OutboundUiPreferencesSnapshot>,
    pub outbound_inspecting: Option<OutboundId>,
    /// The last accepted measurement of the tracked outbound probe. It is a
    /// retained fact, not a stage: it survives every probe stage.
    pub outbound_probe_measurement: Option<OutboundProbeSnapshot>,
    /// The single typed stage of the tracked outbound inspection probe.
    pub outbound_probe: OutboundProbeStage,
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
    pub runtime_prev_upload_total: Option<u64>,
    pub runtime_prev_download_total: Option<u64>,
    pub runtime_prev_snapshot_at: Option<Instant>,
    pub runtime_patch_stage: RuntimePatchStage,
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
    /// The single typed stage of the Iced system-proxy toggle.
    pub system_proxy_stage: SystemProxyStage,
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

impl RuntimeState {
    /// Begin a lifecycle command.
    ///
    /// A second command is refused while one is still in flight: the running
    /// stage is handed back instead of being overwritten, so "a command is in
    /// flight" and "the last command failed" can never both hold.
    pub fn begin_lifecycle(&mut self, action: CoreControlAction) -> Result<(), CoreLifecycleRefusal> {
        if let CoreLifecycleStage::Running { action: running } = self.lifecycle {
            return Err(CoreLifecycleRefusal::AlreadyRunning { action: running });
        }
        self.lifecycle = CoreLifecycleStage::Running { action };
        Ok(())
    }

    /// Settle the in-flight lifecycle command.
    ///
    /// A reply for an action that is not the running one is refused and the
    /// running stage is left untouched.
    pub fn settle_lifecycle(
        &mut self,
        action: CoreControlAction,
        result: Result<(), Failure>,
    ) -> Result<(), CoreLifecycleRefusal> {
        match self.lifecycle {
            CoreLifecycleStage::Running { action: running } if running == action => {}
            _ => return Err(CoreLifecycleRefusal::NotRunning { action }),
        }
        self.lifecycle = match result {
            Ok(()) => CoreLifecycleStage::Idle,
            Err(failure) => CoreLifecycleStage::Failed { failure },
        };
        Ok(())
    }

    /// Cancel the in-flight lifecycle command. A settled failure stays
    /// visible, exactly like the old pending/failure pair did.
    pub fn cancel_lifecycle(&mut self) {
        if self.lifecycle.is_running() {
            self.lifecycle = CoreLifecycleStage::Idle;
        }
    }

    /// Report a lifecycle failure that no command produced, such as a failed
    /// host composition.
    pub fn fail_lifecycle(&mut self, failure: Failure) {
        self.lifecycle = CoreLifecycleStage::Failed { failure };
    }

    /// Begin a runtime configuration patch.
    ///
    /// A second patch is refused while one is in flight: the running stage is
    /// handed back instead of being overwritten.
    pub fn begin_runtime_patch(
        &mut self,
        snapshot: RuntimePatchSnapshot,
    ) -> Result<(), RuntimePatchRefusal> {
        if self.runtime_patch_stage.is_applying() {
            return Err(RuntimePatchRefusal::AlreadyApplying);
        }
        self.runtime_patch_stage = RuntimePatchStage::Applying { snapshot };
        Ok(())
    }

    /// Settle the in-flight runtime patch; the applied values stay.
    pub fn settle_runtime_patch(&mut self) {
        self.runtime_patch_stage = RuntimePatchStage::Idle;
    }

    /// Take the restore snapshot of the in-flight runtime patch and return to
    /// idle.
    pub fn take_runtime_patch_snapshot(&mut self) -> Option<RuntimePatchSnapshot> {
        match mem::replace(&mut self.runtime_patch_stage, RuntimePatchStage::Idle) {
            RuntimePatchStage::Applying { snapshot } => Some(snapshot),
            RuntimePatchStage::Idle => None,
        }
    }

    /// Begin an outbound probe.
    ///
    /// A second probe is refused while one is in flight: the running stage is
    /// handed back instead of being overwritten.
    pub fn begin_outbound_probe(&mut self, token: u64) -> Result<(), OutboundProbeRefusal> {
        if let OutboundProbeStage::Running { token: running } = self.outbound_probe {
            return Err(OutboundProbeRefusal::AlreadyRunning { token: running });
        }
        self.outbound_probe = OutboundProbeStage::Running { token };
        Ok(())
    }

    /// End the in-flight outbound probe. A reply for another probe is refused
    /// and the running stage stays untouched.
    pub fn end_outbound_probe(&mut self, token: u64) -> Result<(), OutboundProbeRefusal> {
        match self.outbound_probe {
            OutboundProbeStage::Running { token: running } if running == token => {}
            _ => return Err(OutboundProbeRefusal::NotRunning { token }),
        }
        self.outbound_probe = OutboundProbeStage::Idle;
        Ok(())
    }

    /// Abandon the in-flight outbound probe and lift the block. The last
    /// failure and the accepted measurement stay visible.
    pub fn abandon_outbound_probe(&mut self) {
        if self.outbound_probe.is_running() {
            self.outbound_probe = OutboundProbeStage::Idle;
        }
        self.clear_outbound_probe_block();
    }

    /// Record the accepted measurement of a finished outbound probe.
    pub fn accept_outbound_probe_measurement(&mut self, measurement: OutboundProbeSnapshot) {
        self.outbound_probe_measurement = Some(measurement);
    }

    /// Record the failure of a finished outbound probe.
    pub fn record_outbound_probe_failure(&mut self, failure: Failure) {
        self.outbound_probe = OutboundProbeStage::Failed { failure };
    }

    /// Record that probing is unavailable for the inspected outbound. The
    /// failure of the interrupted attempt is kept, so it is shown again when
    /// the availability recovers.
    pub fn block_outbound_probe(&mut self, availability: OutboundProbeActionAvailability) {
        let failure = match &self.outbound_probe {
            OutboundProbeStage::Failed { failure } => Some(failure.clone()),
            OutboundProbeStage::Blocked { failure, .. } => failure.clone(),
            OutboundProbeStage::Idle | OutboundProbeStage::Running { .. } => None,
        };
        self.outbound_probe = OutboundProbeStage::Blocked {
            availability,
            failure,
        };
    }

    /// Lift the block; the interrupted failure becomes visible again.
    pub fn clear_outbound_probe_block(&mut self) {
        let failure = match &self.outbound_probe {
            OutboundProbeStage::Blocked { failure, .. } => failure.clone(),
            OutboundProbeStage::Idle
            | OutboundProbeStage::Running { .. }
            | OutboundProbeStage::Failed { .. } => return,
        };
        self.outbound_probe = match failure {
            Some(failure) => OutboundProbeStage::Failed { failure },
            None => OutboundProbeStage::Idle,
        };
    }

    /// Reset the outbound probe stage and the accepted measurement.
    pub fn reset_outbound_probe(&mut self) {
        self.outbound_probe = OutboundProbeStage::Idle;
        self.outbound_probe_measurement = None;
    }

    /// Begin a system proxy change.
    ///
    /// A second change is refused while one is in flight: the running stage is
    /// handed back instead of being overwritten.
    pub fn begin_system_proxy_change(&mut self, desired: bool) -> Result<(), SystemProxyRefusal> {
        if self.system_proxy_stage.is_pending() {
            return Err(SystemProxyRefusal::AlreadyPending);
        }
        self.system_proxy_stage = match desired {
            true => SystemProxyStage::Enabling,
            false => SystemProxyStage::Disabling,
        };
        Ok(())
    }

    /// Settle the in-flight system proxy change with the observed readback.
    pub fn settle_system_proxy_change(&mut self, enabled: bool) {
        self.system_proxy_stage = SystemProxyStage::from_enabled(enabled);
    }

    /// Observe the system proxy readback.
    ///
    /// An in-flight change owns the toggle until it settles, so the running
    /// stage is kept: the observation never cancels a change that is still in
    /// flight.
    pub fn observe_system_proxy(&mut self, enabled: bool) {
        if self.system_proxy_stage.is_pending() {
            return;
        }
        self.system_proxy_stage = SystemProxyStage::from_enabled(enabled);
    }

    /// Roll a rejected system proxy change back to the stage it interrupted.
    pub fn rollback_system_proxy_change(&mut self) {
        self.system_proxy_stage = match self.system_proxy_stage {
            SystemProxyStage::Enabling => SystemProxyStage::Disabled,
            SystemProxyStage::Disabling => SystemProxyStage::Enabled,
            SystemProxyStage::Disabled | SystemProxyStage::Enabled => self.system_proxy_stage,
        };
    }

    /// Cancel an in-flight system proxy change; the requested value stays in
    /// place, exactly like the old pending flag did.
    pub fn cancel_system_proxy_change(&mut self) {
        self.system_proxy_stage = match self.system_proxy_stage {
            SystemProxyStage::Enabling => SystemProxyStage::Enabled,
            SystemProxyStage::Disabling => SystemProxyStage::Disabled,
            SystemProxyStage::Disabled | SystemProxyStage::Enabled => self.system_proxy_stage,
        };
    }
}
