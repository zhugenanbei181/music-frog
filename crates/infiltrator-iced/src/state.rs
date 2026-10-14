//! AppState 域拆分(UI-002):原先 161 个平铺字段收敛为五个语义域结构体。
//!
//! 域边界(与 `TODO.md` UI-002 对应):
//! - [`RuntimeState`] 内核与运行控制;[`ProfileState`] 订阅/档案/同步;
//! - [`ConfigEditorState`] 全部配置编辑器;[`DiagnosticsState`] 运行态诊断与性能;
//! - [`ShellState`] 导航/语言/主题/托盘/Admin/demo 等外壳关注点。
//!
//! 视图层只读这些域做纯渲染投影;update 层按域定位字段。

use crate::command_ingress::{CoreCommandIngress, ObservedCoreScope};
use crate::state::diagnostics::DiagnosticsState;
use crate::state::editor::ConfigEditorState;
use crate::state::profiles::ProfileState;
use crate::state::runtime::RuntimeState;
use crate::state::shell::ShellState;
use crate::surface::{SurfaceBridge, SurfaceModel};
use crate::types::app::UwpAppItem;
use crate::types::app_routing::AppRoutingState;
use crate::types::message::Message;
use crate::types::runtime::{RuntimeStatus, RuntimeStreamState};
use crate::utils::sanitize_ui_text;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::core_application::ScopedCoreCommandClient;
use infiltrator_contract::command_catalogue::{CommandCatalogue, ProfileChoice};
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::dns_form::DnsWorkbenchForm;
use infiltrator_contract::error::{ErrorCode, Failure, InfiltratorError};
use infiltrator_contract::logs::LogStreamState;
use infiltrator_contract::mini_hud::{MiniHudReadModel, MiniHudWaveformStrip};
use infiltrator_contract::outbound::identity::SnapshotRevision;
use infiltrator_contract::outbound::preferences::OutboundFavoriteState;
use infiltrator_contract::outbound::preferences::OutboundGroupExpansion;
use infiltrator_contract::outbound::preferences::OutboundUiPreferenceIntent;
use infiltrator_contract::outbound::{
    details::{OutboundDetail, OutboundDetailsSnapshot},
    identity::OutboundId,
    inventory::{OutboundInventorySnapshot, OutboundInventoryState, SelectionAvailability},
    preferences::{
        OutboundHealthFilter, OutboundLayout, OutboundUiPreferencesSnapshot,
        OutboundUiPreferencesValues,
    },
    probe::OutboundSelectionRequest,
};
use infiltrator_contract::pac::PacServiceState;
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::runtime_control::RuntimeControlStatus;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
use infiltrator_contract::surface_snapshot::{
    PageData, PageStatus, SurfaceOrigin, SurfaceSnapshot,
};
use infiltrator_contract::system_proxy::SystemProxyStatus;
use infiltrator_desktop::product::DesktopProductSessionHandle;
use infiltrator_domain::runtime::TrafficData;
use infiltrator_presentation::command_palette_projection::filtered_indices;
use infiltrator_presentation::routing_intent_projection;
use infiltrator_presentation::system_toggle_projection;
use infiltrator_shared::fuzzy_search::pinyin_fuzzy_match;
use infiltrator_shared::locales::{Lang, Localizer};
use std::sync::Arc;

pub mod core_control;
pub mod core_operations;
pub mod diagnostics;
pub mod editor;
mod editor_observation;
pub mod language;
pub mod outbound_inventory;
pub mod profiles;
pub mod proxy_group_order;
pub mod proxy_preferences;
pub mod proxy_probe;
pub mod proxy_probe_settings;
pub mod proxy_search;
pub mod runtime;
pub mod shell;
mod surface_snapshot;
mod traffic;

pub struct AppState {
    pub runtime: RuntimeState,
    pub profile: ProfileState,
    pub editor: ConfigEditorState,
    pub diag: DiagnosticsState,
    pub shell: ShellState,
    pub app_routing: AppRoutingState,
    /// Canonical cross-surface snapshot cache. Existing Elm fields are local
    /// render/update projections and must not become a second shared source.
    pub surface: SurfaceModel,
    /// Optional application pump bridge installed by a desktop/mobile
    /// composition root. `None` keeps the existing pull-free test/demo app.
    pub surface_bridge: Option<SurfaceBridge>,
    /// Host-injected neutral command facade; embedded/headless hosts need no desktop runtime.
    pub commands: Option<CoreApplication>,
    /// The one desktop product owner session shared by Iced update and admin adapters.
    pub(crate) product_session: Option<DesktopProductSessionHandle>,
    pub(crate) command_scope: ObservedCoreScope,
    /// Host-provided process-exit cleanup callback. The UI knows only this
    /// zero-argument seam; OS signal and proxy/TUN details stay in the host.
    pub exit_cleanup: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl AppState {
    /// Rebuild the command catalogue from the stored profile list. Called when
    /// the palette opens and after the profile list changes, so the palette
    /// always lists the live profiles alongside the shared product commands.
    pub fn rebuild_command_catalogue(&mut self) {
        let profiles: Vec<ProfileChoice> = self
            .profile
            .profiles
            .iter()
            .map(|profile| ProfileChoice::new(profile.name.clone(), profile.name.clone()))
            .collect();
        self.shell.command_catalogue = CommandCatalogue::with_profiles(&profiles);
        self.shell.command_selected_index = self
            .shell
            .command_selected_index
            .min(self.filtered_command_indices().len().saturating_sub(1));
    }

    /// Indices of the shared catalogue kept by the current query. The shared
    /// substring rule plus the Iced pinyin matcher; both surfaces derive the
    /// arrow-key order from the same shared catalogue.
    pub fn filtered_command_indices(&self) -> Vec<usize> {
        let lang = Lang(&self.shell.lang);
        filtered_indices(
            &self.shell.command_catalogue,
            &self.shell.command_query,
            &|key| lang.tr(key).into_owned(),
            &pinyin_fuzzy_match,
        )
    }

    /// The Mini HUD's shared read model, assembled from the live projections:
    /// traffic rates, the observed routing intent, the selected exit node and
    /// the shared system-toggle snapshot. Nothing here is a constant.
    pub fn mini_hud_read_model(&self) -> MiniHudReadModel {
        let active_node = if !self.runtime.runtime_selected_proxy.is_empty() {
            self.runtime.runtime_selected_proxy.clone()
        } else if let Some(active) = self.profile.profiles.iter().find(|profile| profile.active) {
            active.name.clone()
        } else {
            String::new()
        };
        let (up, down) = self
            .diag
            .traffic
            .as_ref()
            .map(|traffic| (traffic.up, traffic.down))
            .unwrap_or((0, 0));
        MiniHudReadModel {
            visible: self.shell.mini_hud_mode,
            routing_intent: self.runtime.routing_intent_state.clone(),
            exit_node: active_node,
            down_bytes_per_sec: down,
            up_bytes_per_sec: up,
            system_proxy: self.runtime.system_toggles.system_proxy.clone(),
            tun: self.runtime.system_toggles.tun.clone(),
            placement: self.shell.mini_hud_placement,
            waveform: MiniHudWaveformStrip::from_snapshot(&self.runtime.traffic_waveform),
        }
    }

    /// Attach the shared product owner and require scope-bound typed commands.
    /// Hosts and integration harnesses use the same entry point as desktop composition.
    pub fn attach_product_application(&mut self, application: CoreApplication) {
        self.commands = Some(application);
        self.enable_product_command_ingress();
    }

    pub(crate) fn attach_desktop_product_session(
        &mut self,
        session: DesktopProductSessionHandle,
    ) -> Result<(), Failure> {
        self.product_session = Some(session.clone());
        self.shell.admin_shared.set_product_session(session.clone());
        self.refresh_desktop_product_session()
    }

    pub(crate) fn refresh_desktop_product_session(&mut self) -> Result<(), Failure> {
        let session = self.product_session.clone().ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "Desktop product session is unavailable",
                true,
            )
        })?;
        let binding = match session.binding() {
            Ok(binding) => binding,
            Err(error) => {
                let failure = Failure::new(ErrorCode::NotReady, error.to_string(), true);
                self.commands = None;
                self.command_scope = ObservedCoreScope::Unavailable;
                self.runtime.core_lifecycle = CoreLifecycleSnapshot {
                    lifecycle: CoreLifecycle::Failed,
                    failure: Some(failure.clone()),
                    ..CoreLifecycleSnapshot::default()
                };
                self.runtime.status =
                    RuntimeStatus::Error(InfiltratorError::Config(failure.message.clone()));
                self.sync_runtime_slot(None);
                return Err(failure);
            }
        };
        self.attach_product_application(binding.application.as_ref().clone());
        self.runtime.core_lifecycle = binding.lifecycle;
        self.runtime.status = RuntimeStatus::from_core_snapshot(&binding.application.snapshot());
        self.sync_runtime_slot(Some(binding.host_runtime));
        let bridge = SurfaceBridge::from_pump(binding.surface_pump);
        if let Some(snapshot) = bridge.current_snapshot() {
            self.apply_shared_surface_snapshot(snapshot);
        }
        self.attach_surface_bridge(bridge);
        Ok(())
    }

    pub(crate) fn rebind_product_commands_to_scope(&mut self, scope: Option<CoreScope>) {
        if self.commands.is_some() {
            let observed = self.commands.as_ref().and_then(|application| {
                scope.map(|scope| {
                    ObservedCoreScope::Bound(ScopedCoreCommandClient::new(
                        application.clone(),
                        scope,
                    ))
                })
            });
            self.command_scope = observed.unwrap_or(ObservedCoreScope::Unavailable);
        }
    }

    pub(crate) fn command_ingress(&self) -> Option<CoreCommandIngress> {
        let application = self.commands.clone()?;
        Some(CoreCommandIngress::new(
            application,
            self.command_scope.clone(),
        ))
    }

    pub(crate) fn is_product_command_ingress(&self) -> bool {
        self.commands.is_some()
    }

    pub(crate) fn apply_outbound_inventory_observation(
        &mut self,
        observation: PageData<OutboundInventorySnapshot>,
    ) {
        if self.uses_outbound_surface()
            && let (Some(current), Some(next)) = (
                self.runtime.outbound_inventory.data.as_ref(),
                observation.data.as_ref(),
            )
            && current.scope() == next.scope()
            && next.revision() < current.revision()
        {
            return;
        }
        self.runtime.outbound_inventory = observation;
        self.refresh_outbound_inventory_projection();
        self.reconcile_outbound_inspection();
        self.refresh_outbound_probe_block();
    }

    pub(crate) fn apply_outbound_details_observation(
        &mut self,
        observation: PageData<OutboundDetailsSnapshot>,
    ) {
        if !self.uses_outbound_surface() {
            return;
        }
        if let (Some(current), Some(next)) = (
            self.runtime.outbound_details.data.as_ref(),
            observation.data.as_ref(),
        ) && (current.scope() != next.scope()
            || next.detail_revision() < current.detail_revision())
        {
            return;
        }
        if let (Some(inventory), Some(details)) = (
            self.runtime.outbound_inventory.data.as_ref(),
            observation.data.as_ref(),
        ) && inventory.scope() != details.scope()
        {
            return;
        }
        self.runtime.outbound_details = observation;
        self.refresh_outbound_inventory_projection();
        self.refresh_outbound_probe_block();
    }

    pub(crate) fn apply_outbound_ui_preferences_observation(
        &mut self,
        observation: PageData<OutboundUiPreferencesSnapshot>,
    ) {
        if !self.uses_outbound_surface() {
            return;
        }
        if let (Some(current), Some(next)) = (
            self.runtime.outbound_ui_preferences.data.as_ref(),
            observation.data.as_ref(),
        ) && (current.scope() != next.scope() || next.revision() < current.revision())
        {
            return;
        }
        if let (Some(inventory), Some(preferences)) = (
            self.runtime.outbound_inventory.data.as_ref(),
            observation.data.as_ref(),
        ) && inventory.scope() != preferences.scope()
        {
            return;
        }
        self.runtime.outbound_ui_preferences = observation;
        self.sync_outbound_ui_preference_fields();
        self.refresh_outbound_inventory_projection();
    }

    pub(crate) fn sync_outbound_ui_preference_fields(&mut self) {
        let Some(preferences) = self.runtime.outbound_ui_preferences.data.as_ref() else {
            return;
        };
        match &self.runtime.proxy_search.phase {
            crate::state::proxy_search::ProxySearchPhase::Uninitialized
            | crate::state::proxy_search::ProxySearchPhase::Synchronized { .. } => {
                self.runtime
                    .proxy_filter
                    .clone_from(&preferences.search_query);
            }
            crate::state::proxy_search::ProxySearchPhase::Dirty { .. }
            | crate::state::proxy_search::ProxySearchPhase::Pending { .. }
            | crate::state::proxy_search::ProxySearchPhase::Failed { .. } => {}
        }
        self.runtime.favorite_proxies = preferences
            .favorite_outbound_ids
            .iter()
            .map(|id| id.value().to_owned())
            .collect();
        self.runtime.filter_alive_only =
            preferences.health_filter == OutboundHealthFilter::HealthyOnly;
        self.runtime.proxy_compact_view = preferences.layout == OutboundLayout::CompactList;
        self.runtime.outbound_sort_order = preferences.sort_order;
    }

    pub(crate) fn apply_demo_outbound_ui_preference(
        &mut self,
        intent: OutboundUiPreferenceIntent,
    ) -> bool {
        if self.is_product_command_ingress() || !self.shell.demo {
            return false;
        }
        let Some(scope) = self.demo_outbound_scope() else {
            return false;
        };
        let Some(current) = self
            .runtime
            .outbound_ui_preferences
            .data
            .as_ref()
            .filter(|current| current.scope() == &scope)
            .cloned()
        else {
            return false;
        };
        let Some(inventory) = self
            .runtime
            .outbound_inventory
            .data
            .as_ref()
            .filter(|inventory| inventory.scope() == &scope)
        else {
            return false;
        };
        let OutboundInventoryState::Ready {
            entries,
            collections,
        } = inventory.inventory()
        else {
            return false;
        };
        let expected_inventory_revision = match &intent {
            OutboundUiPreferenceIntent::ReorderGroups {
                inventory_revision, ..
            }
            | OutboundUiPreferenceIntent::ResetGroupOrder { inventory_revision } => {
                Some(*inventory_revision)
            }
            _ => None,
        };
        if expected_inventory_revision.is_some_and(|expected| expected != inventory.revision()) {
            return false;
        }
        let is_observed = match &intent {
            OutboundUiPreferenceIntent::SetFavorite { outbound_id, .. } => {
                entries.iter().any(|entry| &entry.id == outbound_id)
            }
            OutboundUiPreferenceIntent::SetGroupExpansion { group_id, .. } => collections
                .iter()
                .any(|collection| &collection.id == group_id),
            OutboundUiPreferenceIntent::ReorderGroups { group_ids, .. } => group_ids
                .iter()
                .all(|id| collections.iter().any(|collection| collection.id == *id)),
            OutboundUiPreferenceIntent::SetSearchQuery(_)
            | OutboundUiPreferenceIntent::SetSortOrder(_)
            | OutboundUiPreferenceIntent::SetHealthFilter(_)
            | OutboundUiPreferenceIntent::SetLayout(_)
            | OutboundUiPreferenceIntent::ResetGroupOrder { .. } => true,
        };
        if !is_observed {
            return false;
        }
        let mut search_query = current.search_query.clone();
        let mut favorite_outbound_ids = current.favorite_outbound_ids.clone();
        let mut group_order = current.group_order.clone();
        let mut collapsed_group_ids = current.collapsed_group_ids.clone();
        let mut sort_order = current.sort_order;
        let mut health_filter = current.health_filter;
        let mut layout = current.layout;
        match intent {
            OutboundUiPreferenceIntent::SetSearchQuery(query) => {
                search_query = query;
                self.runtime.proxy_search.phase =
                    crate::state::proxy_search::ProxySearchPhase::Synchronized {
                        query: search_query.clone(),
                    };
            }
            OutboundUiPreferenceIntent::SetFavorite { outbound_id, state } => match state {
                OutboundFavoriteState::Pinned => {
                    if !favorite_outbound_ids.contains(&outbound_id) {
                        favorite_outbound_ids.push(outbound_id);
                    }
                }
                OutboundFavoriteState::Unpinned => {
                    favorite_outbound_ids.retain(|favorite| favorite != &outbound_id);
                }
            },
            OutboundUiPreferenceIntent::SetSortOrder(next) => {
                sort_order = next;
            }
            OutboundUiPreferenceIntent::SetHealthFilter(next) => {
                health_filter = next;
            }
            OutboundUiPreferenceIntent::SetLayout(next) => {
                layout = next;
            }
            OutboundUiPreferenceIntent::SetGroupExpansion {
                group_id,
                expansion,
            } => match expansion {
                OutboundGroupExpansion::Expanded => {
                    collapsed_group_ids.retain(|collapsed| collapsed != &group_id);
                }
                OutboundGroupExpansion::Collapsed => {
                    if !collapsed_group_ids.contains(&group_id) {
                        collapsed_group_ids.push(group_id);
                    }
                }
            },
            OutboundUiPreferenceIntent::ReorderGroups {
                group_ids: order, ..
            } => {
                group_order = order;
            }
            OutboundUiPreferenceIntent::ResetGroupOrder { .. } => {
                group_order.clear();
            }
        }
        let Ok(snapshot) = OutboundUiPreferencesSnapshot::new(
            scope,
            SnapshotRevision::new(current.revision().value().wrapping_add(1)),
            OutboundUiPreferencesValues {
                search_query,
                favorite_outbound_ids,
                group_order,
                collapsed_group_ids,
                sort_order,
                health_filter,
                layout,
            },
        ) else {
            return false;
        };
        self.apply_outbound_ui_preferences_observation(PageData::ready(snapshot));
        true
    }

    pub(crate) fn refresh_outbound_inventory_projection(&mut self) {
        if !self.uses_outbound_surface() {
            return;
        }
        self.runtime.proxy_groups = outbound_inventory::project_groups(
            &self.runtime.outbound_inventory,
            &self.runtime.outbound_details,
            &self.runtime.outbound_ui_preferences,
            &self.runtime.proxy_groups,
        );
        let (filtered, name_runs) = outbound_inventory::filtered_groups(
            &self.runtime.proxy_groups,
            &self.runtime.proxy_filter,
        );
        self.runtime.filtered_groups = filtered;
        self.runtime.proxy_name_runs = name_runs;
        self.sync_runtime_outbound_selection();
    }

    pub(crate) fn sync_runtime_outbound_selection(&mut self) {
        self.runtime.runtime_outbound_selection = None;
        let selected_group = self
            .runtime
            .proxy_groups
            .iter()
            .find(|group| {
                group.name == self.runtime.runtime_selected_group
                    && group.availability == SelectionAvailability::Selectable
            })
            .or_else(|| {
                self.runtime
                    .proxy_groups
                    .iter()
                    .find(|group| group.availability == SelectionAvailability::Selectable)
            });
        let Some(group) = selected_group else {
            self.runtime.runtime_selected_group.clear();
            self.runtime.runtime_selected_proxy.clear();
            return;
        };
        self.runtime.runtime_selected_group.clone_from(&group.name);
        self.runtime.runtime_selected_proxy = group.current().unwrap_or_default().to_owned();
    }

    pub(crate) fn outbound_detail(&self, id: &OutboundId) -> Option<OutboundDetail> {
        let scope = self.current_outbound_scope()?;
        let inventory = self.runtime.outbound_inventory.data.as_ref()?;
        if inventory.scope() != &scope {
            return None;
        }
        let OutboundInventoryState::Ready { entries, .. } = inventory.inventory() else {
            return None;
        };
        if !entries.iter().any(|entry| &entry.id == id) {
            return None;
        }
        let page = &self.runtime.outbound_details;
        if !matches!(page.status, PageStatus::Ready) {
            return None;
        }
        let details = page.data.as_ref()?;
        if details.scope() != &scope {
            return None;
        }
        details.detail(id).cloned()
    }

    pub(crate) fn outbound_is_observed(&self, id: &OutboundId) -> bool {
        let Some(scope) = self.current_outbound_scope() else {
            return false;
        };
        self.runtime
            .outbound_inventory
            .data
            .as_ref()
            .filter(|inventory| inventory.scope() == &scope)
            .is_some_and(|inventory| match inventory.inventory() {
                OutboundInventoryState::Ready { entries, .. } => {
                    entries.iter().any(|entry| &entry.id == id)
                }
                OutboundInventoryState::Unknown
                | OutboundInventoryState::Empty
                | OutboundInventoryState::Unsupported
                | OutboundInventoryState::Failed { .. } => false,
            })
    }

    fn reconcile_outbound_inspection(&mut self) {
        if self
            .runtime
            .outbound_inspecting
            .as_ref()
            .is_some_and(|id| !self.outbound_is_observed(id))
        {
            self.runtime.outbound_inspecting = None;
            self.runtime.outbound_probe_pending = None;
            self.runtime.outbound_probe_result = None;
            self.runtime.outbound_probe_failure = None;
            self.runtime.outbound_probe_blocked = None;
        }
    }

    /// Capture the accepted outbound scope and inventory revision in the UI
    /// message itself. A delayed click can therefore never be rebound to a
    /// newer core activation or inventory revision.
    pub(crate) fn outbound_selection_message(
        &self,
        collection_value: &str,
        outbound_value: &str,
    ) -> Option<Message> {
        let (scope, request) = self.outbound_selection_request(collection_value, outbound_value)?;
        Some(Message::SelectOutbound { scope, request })
    }

    pub(crate) fn outbound_selection_request_is_current(
        &self,
        scope: &CoreScope,
        request: &OutboundSelectionRequest,
    ) -> bool {
        if self.current_outbound_scope().as_ref() != Some(scope)
            || request.validate_scope(scope).is_err()
        {
            return false;
        }
        let Some(inventory) = self
            .runtime
            .outbound_inventory
            .data
            .as_ref()
            .filter(|inventory| inventory.scope() == scope)
            .filter(|inventory| inventory.revision() == request.expected_revision())
        else {
            return false;
        };
        let OutboundInventoryState::Ready { collections, .. } = inventory.inventory() else {
            return false;
        };
        collections.iter().any(|collection| {
            collection.id == *request.collection_id()
                && collection.availability == SelectionAvailability::Selectable
                && collection.member_ids.contains(request.outbound_id())
        })
    }

    pub(crate) fn prepare_runtime_outbound_selection_message(
        &self,
        collection_value: &str,
        outbound_value: &str,
    ) -> Option<Message> {
        let (scope, request) = self.outbound_selection_request(collection_value, outbound_value)?;
        Some(Message::PrepareRuntimeOutboundSelection { scope, request })
    }

    fn outbound_selection_request(
        &self,
        collection_value: &str,
        outbound_value: &str,
    ) -> Option<(CoreScope, OutboundSelectionRequest)> {
        // Product and explicit demo actions are both fenced to the scope in
        // the latest accepted SurfaceSnapshot. A fixture never invents a
        // scope when its typed surface observation is absent.
        let scope = self.current_outbound_scope()?;
        let inventory = self.runtime.outbound_inventory.data.as_ref()?;
        if inventory.scope() != &scope {
            return None;
        }
        let OutboundInventoryState::Ready {
            entries,
            collections,
        } = inventory.inventory()
        else {
            return None;
        };
        let collection = collections.iter().find(|collection| {
            collection.id.value() == collection_value
                && collection.availability == SelectionAvailability::Selectable
        })?;
        let outbound = entries
            .iter()
            .find(|entry| entry.id.value() == outbound_value)?;
        if !collection.member_ids.contains(&outbound.id) {
            return None;
        }
        let request = OutboundSelectionRequest::new(
            collection.id.clone(),
            outbound.id.clone(),
            inventory.revision(),
        )
        .ok()?;
        Some((scope, request))
    }

    fn current_outbound_scope(&self) -> Option<CoreScope> {
        if self.is_product_command_ingress() {
            return self.command_ingress()?.observed_scope();
        }
        self.demo_outbound_scope()
    }

    fn demo_outbound_scope(&self) -> Option<CoreScope> {
        if !self.shell.demo {
            return None;
        }
        let snapshot = self.surface.latest()?;
        (snapshot.origin == SurfaceOrigin::Demo)
            .then(|| snapshot.core.scope())
            .flatten()
    }

    fn uses_outbound_surface(&self) -> bool {
        self.is_product_command_ingress() || self.demo_outbound_scope().is_some()
    }

    fn rebind_product_command_scope(&mut self, snapshot: &SurfaceSnapshot) {
        if self.commands.is_some() {
            self.command_scope = self.observed_scope_for(snapshot);
        }
    }

    fn observed_scope_for(&self, snapshot: &SurfaceSnapshot) -> ObservedCoreScope {
        self.commands
            .as_ref()
            .and_then(|application| {
                snapshot
                    .core
                    .scope()
                    .map(|scope| ScopedCoreCommandClient::new(application.clone(), scope))
            })
            .map(ObservedCoreScope::Bound)
            .unwrap_or(ObservedCoreScope::Unavailable)
    }

    /// Attach the host-composed shared surface input before the Iced runtime
    /// starts. The update loop only receives typed `Message` values afterward.
    pub fn attach_surface_bridge(&mut self, bridge: SurfaceBridge) {
        self.surface_bridge = Some(bridge);
    }

    /// DUAL-09-12: the write classification of the profile open in the editor.
    /// This reads the same shared metadata the application guard enforces, so
    /// the banner, the disabled save button and the guard cannot disagree.
    pub fn edited_profile_write_protection(&self) -> ProfileWriteProtection {
        if let Some((_, document)) = &self.editor.document_latest
            && self.editor.document_session.source().is_some_and(|source| {
                source.document.profile_id == document.workspace_identity.document.profile_id
            })
        {
            return document.write_protection;
        }
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
