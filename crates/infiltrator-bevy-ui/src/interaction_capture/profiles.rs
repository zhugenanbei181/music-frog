//! Drive the profile editor's own controls so the business really renders.
//!
//! The editor card owns the large-document window: the control that opens it
//! is the card's own, and the window it opens is the business the surface
//! shows. The capture activates the card's own control, so the card really
//! renders the window.
use super::geometry::CaptureGeometry;
use super::scroll::{outermost_scroll_area, request_scroll};
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::pages::profiles::ProfilesPageRoot;
use crate::pages::profiles_editor::{
    ProfileEditorDiagnosticPill, ProfileEditorDiagnosticText, ProfileEditorSnippetButton,
};
use crate::pages::profiles_editor_large::{
    ProfileEditorLargeWindowButton, ProfileEditorLargeWindowCloseButton,
    ProfileEditorLargeWindowPanel, ProfileEditorLargeWindowStatusText,
};
use crate::route::surface_projection::replay_surface_snapshot;
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, ScrollArea};
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::profile_document::{DocumentInspection, ProfileDocumentSnapshot};
use infiltrator_contract::profile_editor_read::{ProfileEditorReadSnapshot, ProfileReadStatus};
use infiltrator_test_support::profile_store_fixtures::large_profile_document;
use infiltrator_test_support::subscription_filter_fixture;
use lurker_core_contract::page::SurfaceOrigin;
use lurker_core_contract::profile_document::{DocumentDiagnosticSnapshot, DocumentValidation};
use lurker_core_contract::profile_protection::ProfileWriteProtection;

/// The capture's own progress through the editor stages.
#[derive(Resource, Default)]
pub struct ProfilesCapture {
    stage: u8,
}

pub fn install(app: &mut App) {
    app.init_resource::<ProfilesCapture>();
}

#[derive(SystemParam)]
pub struct ProfilesSurface<'w, 's> {
    feature: Res<'w, InteractionCapture>,
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, ProfilesCapture>,
    latest: ResMut<'w, LatestSurfaceSnapshot>,
    large_buttons: Query<'w, 's, Entity, With<ProfileEditorLargeWindowButton>>,
    snippet_buttons: Query<'w, 's, (Entity, &'static ProfileEditorSnippetButton)>,
    diagnostic_pills: Query<'w, 's, Entity, With<ProfileEditorDiagnosticPill>>,
    parents: Query<'w, 's, &'static ChildOf>,
    areas: Query<'w, 's, Entity, With<ScrollArea>>,
}

fn scroll_into_view(
    entity: Entity,
    parents: &Query<&'static ChildOf>,
    areas: &Query<Entity, With<ScrollArea>>,
    geometry: &mut CaptureGeometry,
    commands: &mut Commands,
) -> bool {
    let Some(page) = outermost_scroll_area(entity, parents, areas) else {
        return true;
    };
    let Some(viewport) = geometry.rect(page) else {
        return true;
    };
    let Some(bounds) = geometry.rect(entity) else {
        return true;
    };
    request_scroll(commands, page, bounds, viewport)
}

/// Activate the editor card's own controls.
pub fn activate(
    mut surface: ProfilesSurface,
    mut geometry: CaptureGeometry,
    mut commands: Commands,
) {
    let fid = surface.feature.feature;
    if !matches!(
        fid,
        FeatureId::EditorLargeDocumentWindow
            | FeatureId::EditorSnippetInsertion
            | FeatureId::EditorYamlErrorNavigation
    ) {
        return;
    }
    if !selected(&surface.feature, &surface.route, fid, Route::Profiles) {
        return;
    }
    if surface.capture.stage == 0 {
        let content = match fid {
            FeatureId::EditorLargeDocumentWindow => large_profile_document(120),
            FeatureId::EditorYamlErrorNavigation => {
                "mixed-port: 7890\nproxies:\n  - [unterminated\n".to_string()
            }
            _ => "mixed-port: 7890\nmode: rule\nrules:\n  - MATCH,DIRECT\n".to_string(),
        };
        let inspection = match fid {
            FeatureId::EditorYamlErrorNavigation => {
                DocumentInspection::Inspected(DocumentValidation::Invalid {
                    diagnostics: vec![DocumentDiagnosticSnapshot {
                        line: Some(3),
                        column: Some(5),
                        message: "syntax error: unexpected end of stream".to_string(),
                    }],
                })
            }
            _ => DocumentInspection::Inspected(DocumentValidation::Valid),
        };
        let workspace_identity =
            subscription_filter_fixture::observation("main", &content, Default::default())
                .expect("valid workspace identity")
                .workspace_identity;
        let document = ProfileDocumentSnapshot::new(
            workspace_identity.clone(),
            content,
            ProfileWriteProtection::Editable,
        )
        .with_inspection(inspection);

        let mut proj = surface.latest.0.clone();
        proj.origin = SurfaceOrigin::Demo;
        proj.revision += 1;
        proj.profile_editor.document = Some(document);
        proj.profile_editor.read = ProfileEditorReadSnapshot {
            profile: Some("main".into()),
            observed_workspace_identity: Some(workspace_identity.clone()),
            verified_workspace_identity: Some(workspace_identity),
            document: ProfileReadStatus::Ready,
            ..Default::default()
        };
        surface.latest.0 = proj;
        replay_surface_snapshot(surface.latest.0.clone(), &mut commands);
        surface.capture.stage = 1;
        return;
    }
    if surface.capture.stage == 1 {
        match fid {
            FeatureId::EditorLargeDocumentWindow => {
                if let Some(entity) = surface.large_buttons.iter().next() {
                    if !scroll_into_view(
                        entity,
                        &surface.parents,
                        &surface.areas,
                        &mut geometry,
                        &mut commands,
                    ) {
                        return;
                    }
                    commands.trigger(Activate { entity });
                    surface.capture.stage = 2;
                }
            }
            FeatureId::EditorSnippetInsertion => {
                if let Some((entity, _)) = surface
                    .snippet_buttons
                    .iter()
                    .find(|(_, button)| button.index == 0)
                {
                    if !scroll_into_view(
                        entity,
                        &surface.parents,
                        &surface.areas,
                        &mut geometry,
                        &mut commands,
                    ) {
                        return;
                    }
                    commands.trigger(Activate { entity });
                    surface.capture.stage = 2;
                }
            }
            FeatureId::EditorYamlErrorNavigation => {
                if let Some(entity) = surface.diagnostic_pills.iter().next() {
                    if !scroll_into_view(
                        entity,
                        &surface.parents,
                        &surface.areas,
                        &mut geometry,
                        &mut commands,
                    ) {
                        return;
                    }
                    surface.capture.stage = 2;
                }
            }
            _ => {}
        }
    }
}

#[derive(SystemParam)]
pub struct ProfilesObservation<'w, 's> {
    capture: Res<'w, ProfilesCapture>,
    panels: Query<'w, 's, Entity, With<ProfileEditorLargeWindowPanel>>,
    statuses: Query<'w, 's, (Entity, &'static Text), With<ProfileEditorLargeWindowStatusText>>,
    closes: Query<'w, 's, Entity, With<ProfileEditorLargeWindowCloseButton>>,
    snippet_buttons: Query<'w, 's, (Entity, &'static ProfileEditorSnippetButton)>,
    diagnostic_pills: Query<'w, 's, Entity, With<ProfileEditorDiagnosticPill>>,
    diagnostic_texts: Query<'w, 's, (Entity, &'static Text), With<ProfileEditorDiagnosticText>>,
    pages: Query<'w, 's, Entity, With<ProfilesPageRoot>>,
    parents: Query<'w, 's, &'static ChildOf>,
    areas: Query<'w, 's, Entity, With<ScrollArea>>,
}

/// Observe the editor card elements the capture really rendered.
pub fn observe(
    surface: ProfilesObservation,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
    mut commands: Commands,
) {
    let fid = feature.feature;
    if surface.capture.stage < 2 {
        return;
    }
    let bounds = (|| match fid {
        FeatureId::EditorLargeDocumentWindow => {
            let panel = surface.panels.single().ok()?;
            if !geometry.visible(panel) {
                return None;
            }
            let (status, text) = surface.statuses.single().ok()?;
            if text.0.is_empty() {
                return None;
            }
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
        }
        FeatureId::EditorSnippetInsertion => {
            let (button, _) = surface.snippet_buttons.iter().find(|(_, b)| b.index == 0)?;
            if !geometry.unclipped(button) {
                reveal(&surface, &mut geometry, &mut commands, button);
                return None;
            }
            geometry.bounds(button, "profile editor snippet button")
        }
        FeatureId::EditorYamlErrorNavigation => {
            let pill = surface.diagnostic_pills.iter().next()?;
            let (text_entity, text) = surface.diagnostic_texts.iter().next()?;
            if text.0.is_empty() {
                return None;
            }
            if !geometry.unclipped(pill) {
                reveal(&surface, &mut geometry, &mut commands, pill);
                return None;
            }
            geometry.bounds(pill, "profile editor syntax diagnostic pill")?;
            geometry.bounds(text_entity, "profile editor syntax diagnostic text")
        }
        _ => None,
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
    let Some(page) = outermost_scroll_area(target, &surface.parents, &surface.areas)
        .or_else(|| surface.pages.iter().next())
    else {
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
