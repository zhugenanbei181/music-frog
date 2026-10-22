//! Drive the Overview page's own controls so the business really renders.
//!
//! The Overview page owns businesses that only exist once its own control was
//! activated: the topology drilldown panel is the page's own answer to a
//! stage activation, the reconnect mask is the watchdog's own degradation
//! state, the speedtest history is the Core's own run history and the visible
//! focus is the shell's own focus ring. The capture activates the page's own
//! control, so the page really renders the business.
use super::capability::capture_command_application;
use super::geometry::CaptureGeometry;
use super::host::{CaptureCapability, CaptureCommandSink};
use super::scroll::request_scroll;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::app::shell_focus::ShellFocusOrder;
use crate::command::CommandSinkHandle;
use crate::pages::overview::{
    OverviewCardMoveUpButton, OverviewCardSlot, OverviewCardSlots, OverviewPageRoot,
    OverviewReloadMask,
};
use crate::pages::overview_speedtest::{
    OverviewSpeedtestButton, OverviewSpeedtestHistoryText, SpeedtestButtonPhase,
};
use crate::pages::overview_topology::{TopologyDrilldownFilter, TopologyStageButton};
use crate::pages::overview_topology_drilldown::{
    TopologyDrilldownCloseButton, TopologyDrilldownNavigateButton, TopologyDrilldownPanel,
    TopologyDrilldownText,
};
use crate::route::surface_projection::replay_surface_snapshot;
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_application::overview_layout_application::OverviewLayoutApplication;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::overview_layout::OverviewCardKind;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::reconnect_mask::ReconnectMaskSnapshot;
use infiltrator_presentation::speedtest_summary_projection::history_caption;
use infiltrator_shared::locales::Lang;
use infiltrator_test_support::speedtest_history_fixtures::{
    capture_speedtest_history, running_speedtest_snapshot,
};
use std::sync::Arc;

/// The capture's own progress through one Overview gesture.
///
/// The gesture is a typed sequence: the owner's facts reach the surface, the
/// page's own control answers them, and the owner's answer comes back.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
enum OverviewStage {
    /// The owner's facts are not on the surface yet.
    #[default]
    Unpublished,
    /// The owner's facts are on the surface; the page's own control is ready.
    Published,
    /// The page's own control was activated; the owner's answer is awaited.
    Activated,
}

/// The capture's own progress.
#[derive(Resource, Default)]
pub struct OverviewCapture {
    drilldown_activated: bool,
    stage: OverviewStage,
    focus_moved: bool,
}

/// Install the Overview capture for one business.
///
/// The card order needs a real owner: the page publishes the shared reorder
/// intent and the owner answers with the next order, so the capture composes
/// the owner into the capture's own command bus.
pub fn install(app: &mut App, feature: FeatureId) {
    app.init_resource::<OverviewCapture>();
    if feature != FeatureId::OverviewCardReorder {
        return;
    }
    let owner = Arc::new(OverviewLayoutApplication::new());
    app.insert_resource(CommandSinkHandle(Arc::new(CaptureCommandSink::new(
        capture_command_application().with_overview_layout(owner.as_ref().clone()),
        CaptureCapability::Layout,
    ))));
    app.insert_resource(OverviewLayoutOwner(owner));
}

/// The capture's own owner of the Overview card order.
#[derive(Resource)]
struct OverviewLayoutOwner(Arc<OverviewLayoutApplication>);

#[derive(SystemParam)]
pub struct OverviewSurface<'w, 's> {
    feature: Res<'w, InteractionCapture>,
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, OverviewCapture>,
    latest: ResMut<'w, LatestSurfaceSnapshot>,
    stages: Query<'w, 's, (Entity, &'static TopologyStageButton)>,
    speedtests: Query<'w, 's, (Entity, &'static OverviewSpeedtestButton)>,
    moves: Query<
        'w,
        's,
        (
            Entity,
            &'static OverviewCardMoveUpButton,
            &'static ButtonDisabled,
        ),
    >,
    owner: Option<Res<'w, OverviewLayoutOwner>>,
    focus: ResMut<'w, InputFocus>,
    order: Res<'w, ShellFocusOrder>,
}

/// Activate the Overview page's own control for the business.
pub fn activate(mut surface: OverviewSurface, mut commands: Commands) {
    match surface.feature.feature {
        FeatureId::OverviewTopologyDrilldown => activate_drilldown(&mut surface, &mut commands),
        FeatureId::OverviewReconnectMask => activate_reconnect_mask(&mut surface, &mut commands),
        FeatureId::SpeedtestHistoryComparison => {
            activate_speedtest_history(&mut surface, &mut commands);
        }
        FeatureId::SpeedtestProgressCancel => {
            activate_speedtest_cancel(&mut surface, &mut commands)
        }
        FeatureId::ShellKeyboardFocus => activate_keyboard_focus(&mut surface, &mut commands),
        FeatureId::OverviewCardReorder => activate_card_reorder(&mut surface, &mut commands),
        _ => {}
    }
}

/// Activate the Overview page's own topology stage control.
fn activate_drilldown(surface: &mut OverviewSurface<'_, '_>, commands: &mut Commands) {
    if !selected(
        &surface.feature,
        &surface.route,
        FeatureId::OverviewTopologyDrilldown,
        Route::Overview,
    ) || surface.capture.drilldown_activated
    {
        return;
    }
    let Some((entity, _button)) = surface.stages.iter().find(|(_, button)| button.enabled) else {
        return;
    };
    surface.capture.drilldown_activated = true;
    commands.trigger(Activate { entity });
}

/// The crash watchdog's own degradation state: the mask is the owner's
/// answer, so the capture publishes the owner's real state.
fn activate_reconnect_mask(surface: &mut OverviewSurface<'_, '_>, commands: &mut Commands) {
    if !selected(
        &surface.feature,
        &surface.route,
        FeatureId::OverviewReconnectMask,
        Route::Overview,
    ) || surface.capture.stage != OverviewStage::Unpublished
    {
        return;
    }
    let mut snapshot = surface.latest.0.clone();
    snapshot.reconnect_mask =
        ReconnectMaskSnapshot::reconnecting(1, 2_000, "the capture's core is reconnecting");
    snapshot.revision = surface.latest.0.revision + 1;
    surface.latest.0 = snapshot;
    replay_surface_snapshot(surface.latest.0.clone(), commands);
    surface.capture.stage = OverviewStage::Published;
}

/// The Core's own run history: the comparison needs two comparable runs, so
/// the capture publishes the owner's real history.
fn activate_speedtest_history(surface: &mut OverviewSurface<'_, '_>, commands: &mut Commands) {
    if !selected(
        &surface.feature,
        &surface.route,
        FeatureId::SpeedtestHistoryComparison,
        Route::Overview,
    ) || surface.capture.stage != OverviewStage::Unpublished
    {
        return;
    }
    let mut snapshot = surface.latest.0.clone();
    snapshot.speedtest.recent_history = capture_speedtest_history();
    snapshot.revision = surface.latest.0.revision + 1;
    surface.latest.0 = snapshot;
    replay_surface_snapshot(surface.latest.0.clone(), commands);
    surface.capture.stage = OverviewStage::Published;
}

/// The running probe: the owner publishes a run in flight, so the page offers
/// the cancellation control, and the page's own control asks the owner to
/// cancel exactly that run.
fn activate_speedtest_cancel(surface: &mut OverviewSurface<'_, '_>, commands: &mut Commands) {
    if !selected(
        &surface.feature,
        &surface.route,
        FeatureId::SpeedtestProgressCancel,
        Route::Overview,
    ) {
        return;
    }
    match surface.capture.stage {
        OverviewStage::Unpublished => {
            let mut snapshot = surface.latest.0.clone();
            // The owner publishes a run that is really in flight: the page
            // then offers the cancellation control for exactly that run.
            snapshot.speedtest = running_speedtest_snapshot();
            snapshot.revision = surface.latest.0.revision + 1;
            surface.latest.0 = snapshot;
            replay_surface_snapshot(surface.latest.0.clone(), commands);
            surface.capture.stage = OverviewStage::Published;
        }
        OverviewStage::Published => {
            let Some((entity, _button)) = surface
                .speedtests
                .iter()
                .find(|(_, button)| button.phase == SpeedtestButtonPhase::Running)
            else {
                return;
            };
            surface.capture.stage = OverviewStage::Activated;
            commands.trigger(Activate { entity });
        }
        OverviewStage::Activated => {}
    }
}

/// The card order: the owner owns the order, the page renders it.
///
/// The capture gives the page a real owner for the order, activates the
/// page's own move control, and publishes the owner's answer, so the page
/// really re-renders its cards in the owner's order.
fn activate_card_reorder(surface: &mut OverviewSurface<'_, '_>, commands: &mut Commands) {
    if !selected(
        &surface.feature,
        &surface.route,
        FeatureId::OverviewCardReorder,
        Route::Overview,
    ) {
        return;
    }
    let Some(owner) = surface.owner.as_deref().map(|owner| owner.0.clone()) else {
        return;
    };
    match surface.capture.stage {
        OverviewStage::Unpublished => {
            // The owner publishes the order the page renders.
            let mut snapshot = surface.latest.0.clone();
            snapshot.overview_layout = owner.snapshot();
            snapshot.revision = surface.latest.0.revision + 1;
            surface.latest.0 = snapshot;
            replay_surface_snapshot(surface.latest.0.clone(), commands);
            surface.capture.stage = OverviewStage::Published;
        }
        OverviewStage::Published => {
            // Only a card that can move is offered the move: the disabled bit
            // of the control is the shared order's own answer. The capture
            // moves the traffic card, the second card of the shared default
            // order, so the move really changes the order the page renders.
            let Some((entity, _, _)) = surface
                .moves
                .iter()
                .find(|(_, button, disabled)| button.0 == OverviewCardKind::Traffic && !disabled.0)
            else {
                return;
            };
            surface.capture.stage = OverviewStage::Activated;
            commands.trigger(Activate { entity });
        }
        OverviewStage::Activated => {
            let published = owner.snapshot();
            if surface.latest.0.overview_layout.order != published.order {
                let mut snapshot = surface.latest.0.clone();
                snapshot.overview_layout = published;
                snapshot.revision = surface.latest.0.revision + 1;
                surface.latest.0 = snapshot;
                replay_surface_snapshot(surface.latest.0.clone(), commands);
            }
        }
    }
}

/// The shell's own focus: the user moves the focus with the keyboard, so the
/// official focus lands on a control with a user-driven cause and the shell
/// draws its ring.
fn activate_keyboard_focus(surface: &mut OverviewSurface<'_, '_>, _commands: &mut Commands) {
    if !selected(
        &surface.feature,
        &surface.route,
        FeatureId::ShellKeyboardFocus,
        Route::Overview,
    ) || surface.capture.focus_moved
    {
        return;
    }
    // The shell's own landing is an automatic focus and stays invisible: the
    // ring marks a focus the user moved. The capture moves the focus to the
    // next control of the measured focus order, exactly like the Tab key.
    let Some(target) = surface.order.targets.get(1).map(|target| target.entity) else {
        return;
    };
    surface.capture.focus_moved = true;
    surface.focus.set(target, FocusCause::Navigated);
}

#[derive(SystemParam)]
pub struct OverviewObservation<'w, 's> {
    drilldown: Option<Res<'w, TopologyDrilldownFilter>>,
    panels: Query<'w, 's, Entity, With<TopologyDrilldownPanel>>,
    lines: Query<'w, 's, (Entity, &'static Text), With<TopologyDrilldownText>>,
    closes: Query<'w, 's, Entity, With<TopologyDrilldownCloseButton>>,
    navigates: Query<'w, 's, Entity, With<TopologyDrilldownNavigateButton>>,
    masks: Query<'w, 's, Entity, With<OverviewReloadMask>>,
    slots: Query<'w, 's, (Entity, &'static OverviewCardSlot)>,
    containers: Query<'w, 's, (Entity, &'static Children), With<OverviewCardSlots>>,
    histories: Query<'w, 's, (Entity, &'static Text), With<OverviewSpeedtestHistoryText>>,
    speedtests: Query<'w, 's, (Entity, &'static OverviewSpeedtestButton)>,
    focus: Res<'w, InputFocus>,
    order: Res<'w, ShellFocusOrder>,
    locale: Res<'w, UiLocale>,
    latest: Res<'w, LatestSurfaceSnapshot>,
    pages: Query<'w, 's, Entity, With<OverviewPageRoot>>,
}

/// Observe the business the Overview page really rendered.
pub fn observe(
    mut surface: OverviewObservation,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
    mut commands: Commands,
) {
    let target = feature.feature;
    let bounds = (|| {
        match target {
            FeatureId::OverviewTopologyDrilldown => {
                observe_drilldown(&mut surface, &mut geometry, &mut commands)
            }
            FeatureId::OverviewReconnectMask => {
                let mask = surface.masks.single().ok()?;
                if !geometry.visible(mask) {
                    return None;
                }
                geometry.bounds(mask, "overview reconnect mask")
            }
            FeatureId::SpeedtestHistoryComparison => {
                // The page replays the owner's own history, so the capture
                // asks the shared copy for the exact text the page must show
                // instead of pinning one language of its own.
                let language = surface.locale.code().to_string();
                let expected = history_caption(&surface.latest.0.speedtest, &Lang(&language));
                let (entity, text) = surface.histories.single().ok()?;
                if text.0 != expected || expected.is_empty() {
                    return None;
                }
                geometry.bounds(entity, "overview speedtest history")
            }
            FeatureId::SpeedtestProgressCancel => {
                let (entity, button) = surface.speedtests.single().ok()?;
                if button.phase != SpeedtestButtonPhase::Running {
                    return None;
                }
                geometry.bounds(entity, "overview speedtest cancel control")
            }
            FeatureId::OverviewCardReorder => {
                // The owner's order is the page's order: the page renders the
                // slots in the order the owner published.
                let published = surface.latest.0.overview_layout.order.clone();
                if published.first() != Some(&OverviewCardKind::Traffic) {
                    return None;
                }
                // The container's own child order is the page's visual order:
                // a query's iteration order is not a layout order.
                let Ok((_, children)) = surface.containers.single() else {
                    return None;
                };
                let mut visual: Vec<OverviewCardKind> = Vec::with_capacity(children.len());
                for child in children.iter() {
                    let Ok((_, slot)) = surface.slots.get(*child) else {
                        continue;
                    };
                    visual.push(slot.0);
                }
                if visual != published {
                    return None;
                }
                let (entity, _) = surface
                    .slots
                    .iter()
                    .find(|(_, slot)| slot.0 == OverviewCardKind::Traffic)?;
                // The moved card sits deep in the Overview page, so the
                // capture scrolls the page's own viewport until the card is
                // really rendered, and measures it on the next frame.
                if !geometry.unclipped(entity) {
                    reveal_slot(&mut surface, &mut geometry, &mut commands, entity);
                    return None;
                }
                geometry.bounds(entity, "overview reordered card")
            }
            FeatureId::ShellKeyboardFocus => {
                // The ring is the shell's own answer to a user-driven focus.
                let focused = surface.focus.get()?;
                if !surface.order.contains(focused) {
                    return None;
                }
                geometry.bounds(focused, "shell focus ring")
            }
            _ => None,
        }
    })();
    observation.publish(&mut feature, bounds);
}

/// Observe the drilldown panel the page really rendered.
fn observe_drilldown(
    surface: &mut OverviewObservation<'_, '_>,
    geometry: &mut CaptureGeometry<'_, '_>,
    commands: &mut Commands,
) -> Option<[f32; 4]> {
    let drilldown = surface.drilldown.as_ref()?;
    if drilldown.stage.is_none() {
        return None;
    }
    let panel = surface.panels.single().ok()?;
    let (line, text) = surface.lines.single().ok()?;
    if text.0.is_empty() {
        return None;
    }
    // The topology card sits far below the fold of the Overview page, so
    // the capture scrolls the page's own viewport until the panel is
    // really rendered, and measures it on the next frame.
    if !geometry.unclipped(panel) {
        reveal(surface, geometry, commands, panel);
        return None;
    }
    geometry.bounds(panel, "overview topology drilldown panel")?;
    geometry.bounds(line, "overview topology drilldown result")?;
    geometry.bounds(
        surface.closes.single().ok()?,
        "overview topology drilldown close",
    )?;
    geometry.bounds(
        surface.navigates.single().ok()?,
        "overview topology drilldown navigate",
    )?;
    geometry.bounds(panel, "overview topology drilldown panel")
}

/// Scroll the Overview page until the moved card is really rendered.
fn reveal_slot(
    surface: &mut OverviewObservation<'_, '_>,
    geometry: &mut CaptureGeometry<'_, '_>,
    commands: &mut Commands,
    target: Entity,
) {
    let Ok(page) = surface.pages.single() else {
        return;
    };
    let Some(viewport) = geometry.rect(page) else {
        return;
    };
    let Some(bounds) = geometry.rect(target) else {
        return;
    };
    request_scroll(commands, page, bounds, viewport);
}

/// Scroll the Overview page until the drilldown panel is really rendered.
fn reveal(
    surface: &OverviewObservation<'_, '_>,
    geometry: &mut CaptureGeometry<'_, '_>,
    commands: &mut Commands,
    target: Entity,
) {
    let Ok(page) = surface.pages.single() else {
        return;
    };
    let Some(viewport) = geometry.rect(page) else {
        return;
    };
    let Some(bounds) = geometry.rect(target) else {
        return;
    };
    request_scroll(commands, page, bounds, viewport);
}
