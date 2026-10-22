//! Drive the profile editor's own controls so the business really renders.
//!
//! The editor card owns the large-document window: the control that opens it
//! is the card's own, and the window it opens is the business the surface
//! shows. The capture activates the card's own control, so the card really
//! renders the window.
use super::geometry::CaptureGeometry;
use super::scroll::{outermost_scroll_area, request_scroll};
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::pages::profiles_editor_large::{
    ProfileEditorLargeWindowButton, ProfileEditorLargeWindowCloseButton,
    ProfileEditorLargeWindowPanel, ProfileEditorLargeWindowStatusText,
};
use crate::route::{ActiveRoute, Route};
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, ScrollArea};
use infiltrator_contract::parity::FeatureId;

/// The capture's own progress: the window is opened exactly once.
#[derive(Resource, Default)]
pub struct ProfilesCapture {
    opened: bool,
}

pub fn install(app: &mut App) {
    app.init_resource::<ProfilesCapture>();
}

#[derive(SystemParam)]
pub struct ProfilesSurface<'w, 's> {
    feature: Res<'w, InteractionCapture>,
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, ProfilesCapture>,
    buttons: Query<'w, 's, Entity, With<ProfileEditorLargeWindowButton>>,
}

/// Activate the editor card's own large-document control.
pub fn activate(mut surface: ProfilesSurface, mut commands: Commands) {
    if !selected(
        &surface.feature,
        &surface.route,
        FeatureId::EditorLargeDocumentWindow,
        Route::Profiles,
    ) || surface.capture.opened
    {
        return;
    }
    let Some(entity) = surface.buttons.iter().next() else {
        return;
    };
    surface.capture.opened = true;
    commands.trigger(Activate { entity });
}

#[derive(SystemParam)]
pub struct ProfilesObservation<'w, 's> {
    panels: Query<'w, 's, Entity, With<ProfileEditorLargeWindowPanel>>,
    statuses: Query<'w, 's, (Entity, &'static Text), With<ProfileEditorLargeWindowStatusText>>,
    closes: Query<'w, 's, Entity, With<ProfileEditorLargeWindowCloseButton>>,
    parents: Query<'w, 's, &'static ChildOf>,
    areas: Query<'w, 's, Entity, With<ScrollArea>>,
}

/// Observe the large-document window the editor card really rendered.
pub fn observe(
    surface: ProfilesObservation,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
    mut commands: Commands,
) {
    let target = feature.feature;
    let bounds = (|| {
        if target != FeatureId::EditorLargeDocumentWindow {
            return None;
        }
        let panel = surface.panels.single().ok()?;
        if !geometry.visible(panel) {
            return None;
        }
        let (status, text) = surface.statuses.single().ok()?;
        if text.0.is_empty() {
            return None;
        }
        // The editor card sits deep in the profiles page, so the capture
        // scrolls the page's own viewport until the window is really
        // rendered, and measures it on the next frame.
        if !geometry.unclipped(panel) {
            reveal(&surface, &mut geometry, &mut commands, panel);
            return None;
        }
        geometry.bounds(panel, "profile editor large document window")?;
        geometry.bounds(status, "profile editor large document status")?;
        geometry.bounds(
            surface.closes.single().ok()?,
            "profile editor large document close",
        )?;
        geometry.bounds(panel, "profile editor large document window")
    })();
    observation.publish(&mut feature, bounds);
}

/// Scroll the profiles page until the editor window is really rendered.
fn reveal(
    surface: &ProfilesObservation<'_, '_>,
    geometry: &mut CaptureGeometry<'_, '_>,
    commands: &mut Commands,
    target: Entity,
) {
    let Some(page) = outermost_scroll_area(target, &surface.parents, &surface.areas) else {
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
