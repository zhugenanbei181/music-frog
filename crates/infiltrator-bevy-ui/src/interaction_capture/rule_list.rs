//! Scoped activation of the actual staged-list UI and permission failure panel.
use super::capability::{capture_command_application, capture_core_capability_application};
use super::geometry::CaptureGeometry;
use super::host::{CaptureCapability, CaptureCommandSink};
use super::scroll::{request_scroll, union};
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::command::CommandSinkHandle;
use crate::pages::rules::RulesPageRoot;
use crate::pages::rules_draft::{
    RuleListControl, RuleListEditorCard, RuleListPreview, RuleListStatus, RulesDraftState,
};
use crate::pages::rules_edit::RuleToggleButton;
use crate::pages::rules_outcome::{RuleProviderFailureText, RuleProviderRefreshButton};
use crate::pages::rules_projection::{RuleIndexText, RulePayloadText, RuleProxyText, RuleTypeText};
use crate::pages::rules_view::RuleRow;
use crate::route::surface_projection::replay_surface_snapshot;
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Or, QueryData, QueryFilter, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::rule_list_application::RuleListApplication;
use infiltrator_application::rule_provider_runtime_application::RuleProviderRuntimeApplication;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_composition::mihomo_configuration::for_mihomo_store;
use infiltrator_composition::mihomo_configuration::mihomo_rule_trace_fixture;
use infiltrator_composition::mihomo_configuration::profiles_for_mihomo;
use infiltrator_composition::surface_reader::ProductSurfaceReader;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::surface::SurfaceReader;
use infiltrator_test_support::core_process_fixtures::CoreCaptureProcess;
use infiltrator_test_support::rule_provider_fixtures::RuleProviderCapturePort;
use infiltrator_test_support::rule_trace_fixtures::{RuleTraceStore, TRACE_DOCUMENT};
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::page::SurfaceOrigin;
use std::sync::{Arc, Mutex, atomic::Ordering};

#[derive(Resource)]
struct CaptureList {
    store: Arc<RuleTraceStore>,
    reader: Arc<ProductSurfaceReader>,
    stage: u8,
}
pub fn install(app: &mut App) {
    let store = mihomo_rule_trace_fixture();
    store.deny_write.store(true, Ordering::SeqCst);
    let port = Arc::new(RuleProviderCapturePort::default());
    port.fail_refresh("the rule provider host refused");
    let process = Arc::new(CoreCaptureProcess::running("mihomo", "capture-core"));
    let core = Arc::new(CoreApplication::new(
        process.clone(),
        process,
        tokio_application_runtime().expect("isolated rule-list capture"),
    ));
    let capabilities = capture_core_capability_application();
    let rule_provider_runtime = RuleProviderRuntimeApplication::new(port, capabilities);
    let reader = Arc::new(
        ProductSurfaceReader::new(core.clone(), SurfaceKind::BevyDesktop, HostKind::Desktop)
            .with_profiles(profiles_for_mihomo(store.clone()))
            .with_configuration(for_mihomo_store(store.clone()))
            .with_rule_provider_runtime(rule_provider_runtime.clone()),
    );
    // The rule-list commit is a Core backend command: the shared bus admits
    // it only with a validated Core scope, so the capture drives the Core
    // owner's scoped route instead of the unscoped one.
    app.insert_resource(CommandSinkHandle(Arc::new(CaptureCommandSink::scoped(
        core,
        capture_command_application()
            .with_rule_list(RuleListApplication::new(store.clone()))
            .with_rule_provider_runtime(rule_provider_runtime),
        CaptureCapability::RuleList,
    ))));
    app.insert_resource(CaptureList {
        store,
        reader,
        stage: 0,
    });
}
#[derive(SystemParam)]
pub struct ListActivation<'w, 's> {
    feature: Res<'w, InteractionCapture>,
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, CaptureList>,
    latest: ResMut<'w, LatestSurfaceSnapshot>,
    editor: Res<'w, RulesDraftState>,
    toggles: Query<'w, 's, (Entity, &'static RuleToggleButton, &'static ButtonDisabled)>,
    controls: Query<'w, 's, (Entity, &'static RuleListControl, &'static ButtonDisabled)>,
    provider_buttons: Query<
        'w,
        's,
        (
            Entity,
            &'static RuleProviderRefreshButton,
            &'static ButtonDisabled,
        ),
    >,
    pages: Query<'w, 's, Entity, With<RulesPageRoot>>,
    cards: Query<'w, 's, Entity, With<RuleListEditorCard>>,
    rows: Query<'w, 's, (Entity, &'static RuleRow)>,
}
pub fn activate(
    mut surface: ListActivation,
    mut geometry: CaptureGeometry,
    mut commands: Commands,
) {
    if surface.feature.feature == FeatureId::RulesProviderRefreshError {
        if !selected(
            &surface.feature,
            &surface.route,
            FeatureId::RulesProviderRefreshError,
            Route::Rules,
        ) {
            return;
        }
        if surface.capture.stage == 0 {
            let reader = surface.capture.reader.clone();
            let observed = Arc::new(Mutex::new(None));
            let result = observed.clone();
            tokio_application_runtime()
                .expect("isolated capture runtime")
                .block_on(Box::pin(async move {
                    *result.lock().expect("capture read") = Some(reader.read().await);
                }));
            let mut snapshot = observed
                .lock()
                .expect("capture read")
                .take()
                .expect("read complete")
                .expect("actual full page");
            snapshot.origin = SurfaceOrigin::Demo;
            snapshot.revision = surface.latest.0.revision + 1;
            surface.latest.0 = snapshot;
            replay_surface_snapshot(surface.latest.0.clone(), &mut commands);
            surface.capture.stage = 1;
            return;
        }
        if surface.capture.stage == 1 {
            let target = surface
                .provider_buttons
                .iter()
                .find(|(_, button, disabled)| button.0 == 0 && !disabled.0)
                .map(|(entity, _, _)| entity);
            if let Some(entity) = target {
                if let Ok(page) = surface.pages.single()
                    && let (Some(bounds), Some(viewport)) =
                        (geometry.rect(entity), geometry.rect(page))
                    && !request_scroll(&mut commands, page, bounds, viewport)
                {
                    return;
                }
                commands.trigger(Activate { entity });
                surface.capture.stage = 2;
            }
        }
        return;
    }
    if !selected(
        &surface.feature,
        &surface.route,
        FeatureId::RulesListEditor,
        Route::Rules,
    ) {
        return;
    }
    if surface.capture.stage == 0 {
        let reader = surface.capture.reader.clone();
        let observed = Arc::new(Mutex::new(None));
        let result = observed.clone();
        tokio_application_runtime()
            .expect("isolated capture runtime")
            .block_on(Box::pin(async move {
                *result.lock().expect("capture read") = Some(reader.read().await);
            }));
        let mut snapshot = observed
            .lock()
            .expect("capture read")
            .take()
            .expect("read complete")
            .expect("actual full page");
        snapshot.origin = SurfaceOrigin::Demo;
        snapshot.revision = surface.latest.0.revision + 1;
        surface.latest.0 = snapshot;
        replay_surface_snapshot(surface.latest.0.clone(), &mut commands);
        surface.capture.stage = 1;
        return;
    }
    let target = match surface.capture.stage {
        1 if surface.editor.model.editable() => surface
            .toggles
            .iter()
            .find(|(_, row, disabled)| {
                row.0 == surface.editor.model.row_id(0) && row.0.is_some() && !disabled.0
            })
            .map(|(entity, _, _)| entity),
        2 if surface.editor.model.can_save() => surface
            .controls
            .iter()
            .find(|(_, control, disabled)| **control == RuleListControl::Save && !disabled.0)
            .map(|(entity, _, _)| entity),
        _ => None,
    };
    if let Some(entity) = target {
        if let Ok(page) = surface.pages.single()
            && let (Some(bounds), Some(viewport)) = (geometry.rect(entity), geometry.rect(page))
            && !request_scroll(&mut commands, page, bounds, viewport)
        {
            return;
        }
        if geometry
            .bounds(entity, "rule-list-native-control")
            .is_some()
        {
            commands.trigger(Activate { entity });
            surface.capture.stage += 1;
        }
    }
    if surface.capture.stage == 3 && reveal(&surface, &mut geometry, &mut commands) {
        // The receipt needs the editor card and the changed rule row on screen
        // at the same time: the page scrolls its own viewport until both are
        // really rendered, and the capture measures them on the next frame.
        return;
    }
}

/// Scroll the rules page until the editor card and the changed rule row are
/// really rendered together.
///
/// Returns `true` when a scroll was requested, so the caller waits for the
/// next frame before it measures the controls.
fn reveal(
    surface: &ListActivation<'_, '_>,
    geometry: &mut CaptureGeometry<'_, '_>,
    commands: &mut Commands,
) -> bool {
    let Ok(page) = surface.pages.single() else {
        return false;
    };
    let Some(viewport) = geometry.rect(page) else {
        return false;
    };
    let Ok(card) = surface.cards.single() else {
        return false;
    };
    let Some((row, _)) = surface.rows.iter().find(|(_, marker)| marker.0 == 0) else {
        return false;
    };
    let (Some(card_box), Some(row_box)) = (geometry.rect(card), geometry.rect(row)) else {
        return false;
    };
    let Some(subject) = union(&[card_box, row_box]) else {
        return false;
    };
    !request_scroll(commands, page, subject, viewport)
}
#[derive(SystemParam)]
pub struct ListObservation<'w, 's> {
    capture: Res<'w, CaptureList>,
    editor: Res<'w, RulesDraftState>,
    cards: Query<'w, 's, Entity, With<RuleListEditorCard>>,
    rows: Query<'w, 's, (Entity, &'static RuleRow)>,
    row_copy: Query<'w, 's, RuleCopyGeometry, RuleCopyFilter>,
    status: Query<'w, 's, (Entity, &'static Text), With<RuleListStatus>>,
    preview: Query<'w, 's, (Entity, &'static Text), With<RuleListPreview>>,
    controls: Query<'w, 's, (Entity, &'static RuleListControl, &'static ButtonDisabled)>,
    provider_failures: Query<'w, 's, (Entity, &'static RuleProviderFailureText, &'static Text)>,
    pages: Query<'w, 's, Entity, With<RulesPageRoot>>,
}

#[derive(QueryData)]
pub struct RuleCopyGeometry {
    entity: Entity,
    text: &'static Text,
    index: Option<&'static RuleIndexText>,
    kind: Option<&'static RuleTypeText>,
    payload: Option<&'static RulePayloadText>,
    proxy: Option<&'static RuleProxyText>,
}
#[derive(QueryFilter)]
pub struct RuleIdentityFilter {
    identity: Or<(With<RuleIndexText>, With<RuleTypeText>)>,
}
#[derive(QueryFilter)]
pub struct RuleContentFilter {
    content: Or<(With<RulePayloadText>, With<RuleProxyText>)>,
}
#[derive(QueryFilter)]
pub struct RuleCopyFilter {
    copy: Or<(RuleIdentityFilter, RuleContentFilter)>,
}

fn first_row_copy_visible(
    probe: &ListObservation<'_, '_>,
    geometry: &mut CaptureGeometry<'_, '_>,
) -> bool {
    let mut bounds = [None; 4];
    for copy in &probe.row_copy {
        let slot = if copy.index.is_some_and(|marker| marker.0 == 0) {
            Some(0)
        } else if copy.kind.is_some_and(|marker| marker.0 == 0) {
            Some(1)
        } else if copy.payload.is_some_and(|marker| marker.0 == 0) {
            Some(2)
        } else if copy.proxy.is_some_and(|marker| marker.0 == 0) {
            Some(3)
        } else {
            None
        };
        if let Some(slot) = slot {
            if copy.text.0.is_empty() {
                return false;
            }
            bounds[slot] = geometry.bounds(copy.entity, "rule-list-first-row-copy");
        }
    }
    let [Some(index), Some(kind), Some(payload), Some(proxy)] = bounds else {
        return false;
    };
    let areas = [index, kind, payload, proxy];
    for (position, a) in areas.iter().enumerate() {
        for b in &areas[position + 1..] {
            if a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
            {
                return false;
            }
        }
    }
    true
}
pub fn observe(
    probe: ListObservation,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
    mut commands: Commands,
) {
    if feature.feature == FeatureId::RulesProviderRefreshError {
        if probe.capture.stage < 2 {
            return;
        }
        let Some((entity, _, text)) = probe
            .provider_failures
            .iter()
            .find(|(_, marker, _)| marker.0 == 0)
        else {
            return;
        };
        if text.0.is_empty() || !text.0.contains("rule provider host refused") {
            return;
        }
        if let Ok(page) = probe.pages.single()
            && let (Some(bounds), Some(viewport)) = (geometry.rect(entity), geometry.rect(page))
            && !geometry.unclipped(entity)
        {
            request_scroll(&mut commands, page, bounds, viewport);
            return;
        }
        let Some(bounds) = geometry.bounds(entity, "rules-provider-refresh-error") else {
            return;
        };
        feature.activated = true;
        observation.0 = Some(bounds);
        return;
    }
    if feature.feature != FeatureId::RulesListEditor
        || probe.capture.stage != 3
        || probe.editor.model.is_pending()
        || !probe.editor.model.dirty()
        || !probe.editor.model.can_save()
        || !probe
            .editor
            .model
            .failure()
            .as_ref()
            .is_some_and(|failure| failure.code == ErrorCode::Permission)
        || probe.capture.store.writes.load(Ordering::SeqCst) != 0
        || probe.capture.store.content() != TRACE_DOCUMENT
    {
        return;
    }
    let (Ok(card), Ok((status, status_text)), Ok((preview, preview_text))) = (
        probe.cards.single(),
        probe.status.single(),
        probe.preview.single(),
    ) else {
        return;
    };
    if status_text.0.is_empty() || !preview_text.0.contains("SRC-IP-CIDR,10.0.0.0/8,DIRECT") {
        return;
    }
    let Some((row, _)) = probe.rows.iter().find(|(_, marker)| marker.0 == 0) else {
        return;
    };
    // The receipt needs the editor card and the rule row it really changed on
    // screen at the same time: the page scrolls its own viewport until both
    // are rendered, and the capture measures them on the next frame.
    if !geometry.unclipped(card) || !geometry.unclipped(row) {
        reveal_observation(&probe, &mut geometry, &mut commands);
        return;
    }
    let Some(bounds) = geometry.bounds(card, "rule-list-editor") else {
        return;
    };
    if geometry.bounds(row, "rule-list-actual-first-row").is_none() {
        return;
    }
    if !first_row_copy_visible(&probe, &mut geometry) {
        return;
    }
    for entity in [status, preview] {
        if geometry.bounds(entity, "rule-list-state-copy").is_none() {
            return;
        }
    }
    for wanted in [
        RuleListControl::Save,
        RuleListControl::Discard,
        RuleListControl::Settings,
    ] {
        let Some((entity, _, _)) = probe
            .controls
            .iter()
            .find(|(_, control, disabled)| **control == wanted && !disabled.0)
        else {
            return;
        };
        if geometry.bounds(entity, "rule-list-action").is_none() {
            return;
        }
    }
    feature.activated = true;
    observation.0 = Some(bounds);
}

/// Scroll the rules page until the editor card and the changed rule row are
/// really rendered together.
fn reveal_observation(
    probe: &ListObservation<'_, '_>,
    geometry: &mut CaptureGeometry<'_, '_>,
    commands: &mut Commands,
) {
    let Ok(page) = probe.pages.single() else {
        return;
    };
    let Some(viewport) = geometry.rect(page) else {
        return;
    };
    let Ok(card) = probe.cards.single() else {
        return;
    };
    let Some((row, _)) = probe.rows.iter().find(|(_, marker)| marker.0 == 0) else {
        return;
    };
    let (Some(card_box), Some(row_box)) = (geometry.rect(card), geometry.rect(row)) else {
        return;
    };
    let Some(subject) = union(&[card_box, row_box]) else {
        return;
    };
    request_scroll(commands, page, subject, viewport);
}
