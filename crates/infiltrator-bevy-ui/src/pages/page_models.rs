//! The Bevy surface's handles on the shared page models.
//!
//! Every page's business state now lives in `infiltrator-presentation`; this
//! surface keeps exactly one handle per page and reads its facts from there.
//! Before this module the Bevy pages each re-derived the same answers — the
//! proxy match rule, the member order, the run-state tri-state, the JSON
//! "modified" flags, the operation phases — so two surfaces could disagree
//! about the same node, the same core or the same draft.
//!
//! Two kinds of handle live here:
//!
//! * the **live handle** (one `Resource` per page) that carries the state the
//!   user owns — the typed drafts, the selected tab, the operation phases —
//!   together with the owner facts the shared model last observed;
//! * the **fold** (`observe_*_model`) that reads one accepted snapshot into a
//!   shared model. The page projection and the live handle share this one
//!   fold, so the answer a page renders and the answer the page keeps can
//!   never come from two different rules.
//!
//! The fold is a *merge*, never a replacement: the shared models adopt the
//! owner's value as a new baseline and keep a pending user edit visible.

use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Res, ResMut, SystemParam};
use infiltrator_contract::surface::SurfaceKind;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_presentation::app_routing_page_model::AppRoutingPageModel;
use infiltrator_presentation::dns_page_model::DnsPageModel;
use infiltrator_presentation::doctor_page_model::DoctorPageModel;
use infiltrator_presentation::overview_page_model::{OverviewHost, OverviewPageModel};
use infiltrator_presentation::profiles_page_model::{ProfilesOperation, ProfilesPageModel};
use infiltrator_presentation::proxies_page_model::ProxiesPageModel;
use infiltrator_presentation::rules_page_model::RulesPageModel;
use infiltrator_presentation::settings_page_model::{SettingsPageModel, observe_settings_page};
use infiltrator_presentation::sync_page_model::SyncPageModel;
use lurker_core_contract::outbound::preferences::OutboundUiPreferencesSnapshot;
use lurker_core_contract::page::PageStatus;

/// The shared model of the overview page.
#[derive(Resource, Clone, Debug, Default)]
pub struct OverviewPageModelState(pub OverviewPageModel);

/// The shared model of the proxies page.
#[derive(Resource, Clone, Debug, Default)]
pub struct ProxiesPageModelState(pub ProxiesPageModel);

/// The shared model of the settings page.
#[derive(Resource, Clone, Debug, Default)]
pub struct SettingsPageModelState(pub SettingsPageModel);

/// The shared model of the rules page.
#[derive(Resource, Clone, Debug, Default)]
pub struct RulesPageModelState(pub RulesPageModel);

/// The shared model of the profiles page.
#[derive(Resource, Clone, Debug, Default)]
pub struct ProfilesPageModelState(pub ProfilesPageModel);

/// The shared model of the doctor page.
#[derive(Resource, Clone, Debug, Default)]
pub struct DoctorPageModelState(pub DoctorPageModel);

/// The shared model of the DNS page.
#[derive(Resource, Clone, Debug, Default)]
pub struct DnsPageModelState(pub DnsPageModel);

/// The shared model of the app-routing page.
#[derive(Resource, Clone, Debug, Default)]
pub struct AppRoutingPageModelState(pub AppRoutingPageModel);

/// The shared model of the sync page.
#[derive(Resource, Clone, Debug, Default)]
pub struct SyncPageModelState(pub SyncPageModel);

/// The host the overview page renders for.
///
/// The traffic mode depends on the host, and the host is an input of the
/// snapshot, not a compile-time decision: the same shared model answers the
/// same way on every surface.
pub fn overview_host(snapshot: &SurfaceSnapshot) -> OverviewHost {
    match snapshot.surface {
        SurfaceKind::BevyAndroid | SurfaceKind::SlintAndroid | SurfaceKind::AndroidCompose => {
            OverviewHost::Android
        }
        _ => OverviewHost::Desktop,
    }
}

/// The owner's outbound UI preferences, when the read model published them.
///
/// A preference snapshot from another engine scope is not this surface's
/// preference: it is ignored, exactly like every other scoped read.
pub fn observed_outbound_preferences(
    snapshot: &SurfaceSnapshot,
) -> Option<&OutboundUiPreferencesSnapshot> {
    let scope = snapshot.core.scope();
    matches!(
        snapshot.outbound_ui_preferences.status,
        PageStatus::Ready | PageStatus::Empty
    )
    .then_some(snapshot.outbound_ui_preferences.data.as_ref())
    .flatten()
    .filter(|preferences| scope.as_ref() == Some(preferences.scope()))
}

/// Fold the owner facts of the overview page into the shared model.
pub fn observe_overview_model(model: &mut OverviewPageModel, snapshot: &SurfaceSnapshot) {
    model.observe(overview_host(snapshot), snapshot);
}

/// Fold the owner facts of the proxies page into the shared model.
pub fn observe_proxies_model(model: &mut ProxiesPageModel, snapshot: &SurfaceSnapshot) {
    if let Some(preferences) = observed_outbound_preferences(snapshot) {
        model.observe_preferences(preferences);
    }
}

/// Fold the owner facts of the settings page into the shared model.
///
/// The LAN sharing readback is one typed observation: the shared fold reads
/// it from the owner's snapshot, so an unobserved runtime configuration leaves
/// the model unobserved instead of pretending the sharing is off.
pub fn observe_settings_model(model: &mut SettingsPageModel, snapshot: &SurfaceSnapshot) {
    // The LAN sharing / LAN security / PAC readbacks are folded by the shared
    // model itself, so this surface never rebuilds them from the individual
    // runtime-control fields.
    observe_settings_page(model, snapshot);
    // The Bevy command bus is fire-and-forget: the owner's next publication
    // is the only answer this surface can observe, so it ends a submitted
    // write instead of leaving it running forever.
    if model.lan_sharing_write_phase().is_running()
        && let Some(token) = model.lan_sharing_write_phase().token()
    {
        model.lan_sharing_write_phase_mut().succeed(token);
    }
    if model.lan_security_write_phase().is_running()
        && let Some(token) = model.lan_security_write_phase().token()
    {
        model.lan_security_write_phase_mut().succeed(token);
    }
    if model.pac_write_phase().is_running()
        && let Some(token) = model.pac_write_phase().token()
    {
        model.pac_write_phase_mut().succeed(token);
    }
}

/// Fold the owner facts of the rules page into the shared model.
pub fn observe_rules_model(model: &mut RulesPageModel, snapshot: &SurfaceSnapshot) {
    if let Some(page) = snapshot.pages.rules.data.as_ref() {
        model.observe_json_documents(&page.json_documents);
    }
}

/// Fold the owner facts of the profiles page into the shared model.
///
/// The Bevy command bus is fire-and-forget: the owner's next publication is
/// the only answer this surface can observe, so it ends a submitted phase
/// instead of leaving it running forever.
pub fn observe_profiles_model(model: &mut ProfilesPageModel) {
    for operation in ProfilesOperation::ALL {
        if model.operation(operation).is_running()
            && let Some(token) = model.operation(operation).token()
        {
            model.operation_mut(operation).succeed(token);
        }
    }
}

/// Fold the owner facts of the doctor page into the shared model.
pub fn observe_doctor_model(model: &mut DoctorPageModel, snapshot: &SurfaceSnapshot) {
    if let Some(page) = snapshot.pages.doctor.data.as_ref() {
        model.observe_page(page.clone());
    }
}

/// Fold the owner facts of the DNS page into the shared model.
pub fn observe_dns_model(model: &mut DnsPageModel, snapshot: &SurfaceSnapshot) {
    if let Some(page) = snapshot.pages.dns.data.as_ref() {
        model.observe_modes(page.enhanced_mode, page.filter_mode);
    }
}

/// Fold the owner facts of the app-routing page into the shared model.
pub fn observe_app_routing_model(model: &mut AppRoutingPageModel, snapshot: &SurfaceSnapshot) {
    if let Some(page) = snapshot.pages.app_routing.data.as_ref() {
        model.observe_page(page.clone());
    }
    // The Bevy command bus is fire-and-forget: the owner's next publication
    // is the only answer this surface can observe, so it ends a submitted
    // phase instead of leaving it running forever.
    if model.scan_phase().is_running()
        && let Some(token) = model.scan_phase().token()
    {
        model.scan_phase_mut().succeed(token);
    }
    if model.exemption_write_phase().is_running()
        && let Some(token) = model.exemption_write_phase().token()
    {
        model.exemption_write_phase_mut().succeed(token);
    }
}

#[derive(SystemParam)]
pub struct SharedPageModelsSyncParams<'w> {
    snapshot: Res<'w, LatestSurfaceSnapshot>,
    overview: ResMut<'w, OverviewPageModelState>,
    proxies: ResMut<'w, ProxiesPageModelState>,
    settings: ResMut<'w, SettingsPageModelState>,
    rules: ResMut<'w, RulesPageModelState>,
    profiles: ResMut<'w, ProfilesPageModelState>,
    doctor: ResMut<'w, DoctorPageModelState>,
    dns: ResMut<'w, DnsPageModelState>,
    app_routing: ResMut<'w, AppRoutingPageModelState>,
}

/// Read the latest accepted snapshot into the nine shared page models.
///
/// One fold for every page: the page projections and the live handles share
/// it, so a page can never render a fact the shared model does not hold.
pub fn sync_shared_page_models(params: SharedPageModelsSyncParams) {
    let SharedPageModelsSyncParams {
        snapshot,
        mut overview,
        mut proxies,
        mut settings,
        mut rules,
        mut profiles,
        mut doctor,
        mut dns,
        mut app_routing,
    } = params;
    let snapshot = &snapshot.0;
    observe_overview_model(&mut overview.0, snapshot);
    observe_proxies_model(&mut proxies.0, snapshot);
    observe_settings_model(&mut settings.0, snapshot);
    observe_rules_model(&mut rules.0, snapshot);
    observe_profiles_model(&mut profiles.0);
    observe_doctor_model(&mut doctor.0, snapshot);
    observe_dns_model(&mut dns.0, snapshot);
    observe_app_routing_model(&mut app_routing.0, snapshot);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::surface_demo::demo_snapshot;
    use infiltrator_contract::doctor::DoctorAction;
    use infiltrator_contract::rules_workspace::RulesJsonSection;
    use infiltrator_contract::sync::SyncStatus;
    use infiltrator_presentation::overview_page_model::OverviewRunState;
    use lurker_core_contract::error::{ErrorCode, Failure};
    use lurker_core_contract::identity::EngineId;
    use lurker_core_contract::lifecycle::CoreLifecycle;
    use lurker_core_contract::outbound::identity::OutboundId;
    use lurker_core_contract::outbound::preferences::OutboundSortOrder;

    #[test]
    fn the_overview_page_model_owns_the_run_state() {
        let snapshot = demo_snapshot();
        let mut model = OverviewPageModel::default();
        observe_overview_model(&mut model, &snapshot);
        assert!(
            matches!(model.run_state(), OverviewRunState::Running),
            "the shared model answers the run tri-state, not the surface"
        );

        let mut failed = snapshot.clone();
        failed.core.lifecycle = CoreLifecycle::Failed;
        failed.core.failure = Some(Failure::new(
            ErrorCode::NotReady,
            "the core refused the read",
            true,
        ));
        observe_overview_model(&mut model, &failed);
        match model.run_state() {
            OverviewRunState::Unavailable { reason } => {
                assert_eq!(reason, "the core refused the read");
            }
            other => panic!("a failed core is unavailable, got {other:?}"),
        }
    }

    #[test]
    fn the_proxies_page_model_owns_the_query_and_the_order() {
        let snapshot = demo_snapshot();
        let mut model = ProxiesPageModel::default();
        observe_proxies_model(&mut model, &snapshot);
        assert!(model.query().matches_all());
        assert_eq!(model.sort_order(), OutboundSortOrder::LatencyAscending);
        assert!(!model.healthy_only());
        assert!(
            !model.is_favorite(
                &OutboundId::new(
                    EngineId::new("mihomo").expect("the demo engine ID is valid"),
                    "HK-01"
                )
                .expect("the demo outbound ID is valid")
            )
        );
    }

    #[test]
    fn the_settings_page_model_owns_the_drafts_and_the_intents() {
        let snapshot = demo_snapshot();
        let mut model = SettingsPageModel::default();
        observe_settings_model(&mut model, &snapshot);
        assert!(!model.lan_sharing_modified());
        assert!(
            model.lan_sharing_intent().is_none(),
            "an unchanged draft expresses no intent"
        );
        model.edit_lan_sharing(|draft| draft.enabled = !draft.enabled);
        assert!(model.lan_sharing_modified());
        assert!(model.lan_sharing_intent().is_some());
    }

    #[test]
    fn the_rules_page_model_owns_the_json_drafts() {
        let snapshot = demo_snapshot();
        let mut model = RulesPageModel::default();
        observe_rules_model(&mut model, &snapshot);
        assert!(!model.json_modified(RulesJsonSection::RuleProviders));
        model
            .json_buffer_mut(RulesJsonSection::RuleProviders)
            .edit("{}".to_owned());
        assert!(model.json_modified(RulesJsonSection::RuleProviders));
    }

    #[test]
    fn the_profiles_page_model_owns_the_list_filter() {
        let _snapshot = demo_snapshot();
        let mut model = ProfilesPageModel::default();
        observe_profiles_model(&mut model);
        assert!(model.filter().is_empty());
        assert!(!model.is_busy());
    }

    #[test]
    fn the_doctor_page_model_owns_the_action_phase() {
        let snapshot = demo_snapshot();
        let mut model = DoctorPageModel::default();
        observe_doctor_model(&mut model, &snapshot);
        assert!(!model.is_busy());
        let pending = model
            .begin(DoctorAction::Diagnose)
            .expect("the first diagnostic starts");
        assert!(model.is_busy());
        model.finish(
            pending.token,
            Err(Failure::new(
                ErrorCode::NotReady,
                "the diagnostic failed",
                true,
            )),
        );
        assert!(!model.is_busy());
        assert_eq!(
            model.failure().map(|failure| failure.message.as_str()),
            Some("the diagnostic failed")
        );
    }

    #[test]
    fn the_dns_page_model_owns_the_modes() {
        let snapshot = demo_snapshot();
        let mut model = DnsPageModel::default();
        observe_dns_model(&mut model, &snapshot);
        let page = snapshot.pages.dns.data.as_ref().expect("the demo DNS page");
        assert_eq!(model.enhanced_mode(), page.enhanced_mode);
        assert_eq!(model.filter_mode(), page.filter_mode);
    }

    #[test]
    fn the_app_routing_page_model_reports_no_mode_when_unobserved() {
        let mut model = AppRoutingPageModel::default();
        assert!(
            model.mode().is_none(),
            "an unobserved app-routing page reports no mode"
        );
        let snapshot = demo_snapshot();
        observe_app_routing_model(&mut model, &snapshot);
        assert!(model.mode().is_some());
    }

    #[test]
    fn the_sync_page_model_owns_the_observation_and_the_phases() {
        let snapshot = demo_snapshot();
        let mut model = SyncPageModel::default();
        let page = snapshot
            .pages
            .sync
            .data
            .as_ref()
            .expect("the demo snapshot publishes the sync page");
        model.observe_page(page.clone());
        assert_eq!(model.status(), SyncStatus::Connected);
        assert!(!model.is_syncing());
        model.sync_now_phase_mut().begin();
        assert!(model.sync_now_phase().is_running());
    }
}
