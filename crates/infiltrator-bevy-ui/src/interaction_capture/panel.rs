//! Open the business panel the business is about and observe its real card.
//!
//! The profiles workspace carries two independent business panels (the
//! aggregation wizard and the snapshot diff viewer). Each panel is a modal
//! dialog on the official Bevy behaviour layer: the capture activates the
//! panel's own launcher control, so the panel really opens on screen.
use super::geometry::CaptureGeometry;
use super::{CaptureObservationSet, InteractionCapture, ObservedInteraction, selected};
use crate::pages::business_panel::{
    BusinessPanelCard, BusinessPanelRoot, BusinessPanelState, OpenBusinessPanel, PanelKind,
};
use crate::pages::profiles_aggregator::PreviewAggregationButton;
use crate::route::{ActiveRoute, Route};
use bevy::app::{App, PostUpdate, Update};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ui_widgets::Activate;
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
            let entity = controls.preview_buttons.iter().next()?;
            return geometry.bounds(entity, "aggregator preview button");
        }
        let (card, _) = controls.cards.iter().find(|(_, marker)| marker.0 == kind)?;
        geometry.bounds(card, "business panel card")
    })();
    observation.publish(&mut feature, bounds);
}
