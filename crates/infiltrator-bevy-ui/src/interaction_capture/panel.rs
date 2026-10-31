//! Open the business panel the business is about and observe its real card.
//!
//! The profiles workspace carries two independent business panels (the
//! aggregation wizard and the snapshot diff viewer). Each panel is a modal
//! dialog on the official Bevy behaviour layer: the capture activates the
//! panel's own launcher control, so the panel really opens on screen.
use super::geometry::CaptureGeometry;
use super::scroll::request_scroll;
use super::{CaptureObservationSet, InteractionCapture, ObservedInteraction, selected};
use crate::pages::business_panel::{
    BusinessPanelCard, BusinessPanelRoot, BusinessPanelScrollArea, BusinessPanelState,
    OpenBusinessPanel, PanelKind,
};
use crate::pages::profiles_aggregator::{AggregatorStatusText, PreviewAggregationButton};
use crate::route::{ActiveRoute, Route};
use bevy::app::{App, PostUpdate, Update};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_contract::parity::FeatureId;

/// Track preview trigger in aggregator panel capture.
#[derive(Resource, Default)]
pub struct PanelCaptureState {
    pub preview_triggered: bool,
}

/// The panel a business is about.
fn panel_of(feature: FeatureId) -> Option<PanelKind> {
    match feature {
        FeatureId::ProfilesAggregatorWizard | FeatureId::ProfilesAggregatorPreview => {
            Some(PanelKind::Aggregator)
        }
        FeatureId::ProfilesDiffViewer => Some(PanelKind::SnapshotDiff),
        _ => None,
    }
}

pub fn install(app: &mut App) {
    app.init_resource::<PanelCaptureState>();
    app.add_systems(Update, activate);
    app.add_systems(PostUpdate, observe.in_set(CaptureObservationSet));
}

#[derive(SystemParam)]
pub struct PanelControls<'w, 's> {
    route: Res<'w, ActiveRoute>,
    panel: Res<'w, BusinessPanelState>,
    launchers: Query<'w, 's, (Entity, &'static OpenBusinessPanel)>,
    cards: Query<'w, 's, (Entity, &'static BusinessPanelCard)>,
    roots: Query<'w, 's, (Entity, &'static BusinessPanelRoot)>,
    preview_buttons: Query<'w, 's, Entity, With<PreviewAggregationButton>>,
    status_lines:
        Query<'w, 's, (Entity, &'static Text, &'static LocalizedText), With<AggregatorStatusText>>,
    panel_scrolls: Query<'w, 's, (Entity, &'static BusinessPanelScrollArea)>,
    state: ResMut<'w, PanelCaptureState>,
}

/// Open the panel: the capture activates the panel's own launcher control, so
/// the modal really opens through the surface's own observer.
pub fn activate(
    mut controls: PanelControls,
    feature: Res<'_, InteractionCapture>,
    mut commands: Commands,
) {
    let Some(kind) = panel_of(feature.feature) else {
        return;
    };
    if !selected(&feature, &controls.route, feature.feature, Route::Profiles) {
        return;
    }
    if controls.panel.0 != Some(kind) {
        if let Some((entity, _)) = controls.launchers.iter().find(|(_, open)| open.0 == kind) {
            commands.trigger(Activate { entity });
        }
        return;
    }
    if feature.feature == FeatureId::ProfilesAggregatorPreview
        && !controls.state.preview_triggered
        && let Some(entity) = controls.preview_buttons.iter().next()
    {
        controls.state.preview_triggered = true;
        commands.trigger(Activate { entity });
    }
}

/// Observe the opened panel's own card. The bounds are the card the business
/// really shows, so the pixel judge compares the panel's own pixels.
pub fn observe(
    controls: PanelControls,
    mut commands: Commands,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
) {
    let Some(kind) = panel_of(feature.feature) else {
        return;
    };
    let bounds = (|| {
        if controls.panel.0 != Some(kind) {
            return None;
        }
        let (root, _) = controls.roots.iter().find(|(_, marker)| marker.0 == kind)?;
        if !geometry.visible(root) {
            return None;
        }
        if feature.feature == FeatureId::ProfilesAggregatorPreview {
            if !controls.state.preview_triggered {
                return None;
            }
            // The terminal readback is the surface's own submitted-status
            // line; the receipt shows it scrolled into the panel's own
            // viewport, not the launcher button that opened the panel.
            let Some((status, _, _copy)) = controls
                .status_lines
                .iter()
                .find(|(_, _, copy)| copy.key == "aggregation_preview_submitted")
            else {
                return None;
            };
            let Some((scroll, _scroll_marker)) = controls
                .panel_scrolls
                .iter()
                .find(|(_, marker)| marker.0 == kind)
            else {
                return None;
            };
            if !geometry.unclipped(status) {
                if let (Some(bounds), Some(viewport)) =
                    (geometry.rect(status), geometry.rect(scroll))
                {
                    request_scroll(&mut commands, scroll, bounds, viewport);
                }
                return None;
            }
            return geometry.bounds(status, "aggregator preview submitted status");
        }
        let (card, _) = controls.cards.iter().find(|(_, marker)| marker.0 == kind)?;
        geometry.bounds(card, "business panel card")
    })();
    observation.publish(&mut feature, bounds);
}
