//! The Profiles page performs its own deletion and refresh gestures.
//!
//! Every gesture goes through the page's own control and the shared owner
//! that holds the fact. The capture publishes the owner's real readback, the
//! page's own control asks the owner, and the receipt measures the answer the
//! page really renders.
use super::capability::capture_command_application;
use super::geometry::CaptureGeometry;
use super::host::{CaptureCapability, CaptureCommandSink};
use super::scroll::{outermost_scroll_area, request_scroll};
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::command::CommandSinkHandle;
use crate::pages::profiles::{
    DeleteProfileButton, LastProfilesProjection, ProfilesPageRoot, UpdateProfileButton,
};
use crate::pages::profiles_outcome::{ProfileDeleteOutcomeText, ProfileRefreshText};
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
use infiltrator_application::core_application::CoreApplication;
use infiltrator_composition::mihomo_configuration::profiles_for_mihomo;
use infiltrator_composition::surface_reader::ProductSurfaceReader;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::surface::SurfaceReader;
use infiltrator_test_support::core_process_fixtures::CoreCaptureProcess;
use infiltrator_test_support::profile_store_fixtures::ProfileStoreCapture;
use infiltrator_test_support::subscription_source_fixtures::SubscriptionSourceCapture;
use std::sync::{Arc, Mutex};

/// The engine the capture's own profile store owns.
const CAPTURE_ENGINE: &str = "mihomo";

/// The capture's own progress through one profiles gesture.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ProfilesStage {
    /// The owner's readback is not on the surface yet.
    #[default]
    Unpublished,
    /// The owner's readback is on the surface; the page's own control is ready.
    Published,
    /// The page's own control was activated; the owner's answer is awaited.
    Activated,
}

/// The isolated owners the profiles capture really drives.
#[derive(Resource)]
struct ProfilesCapture {
    stage: ProfilesStage,
    core: Arc<CoreApplication>,
    reader: Arc<ProductSurfaceReader>,
}

pub fn install(app: &mut App) {
    let runtime = tokio_application_runtime().expect("isolated profiles capture runtime");
    let core = Arc::new(CoreApplication::new(
        Arc::new(CoreCaptureProcess::new(CAPTURE_ENGINE, "profiles-capture")),
        Arc::new(CoreCaptureProcess::new(CAPTURE_ENGINE, "profiles-capture")),
        runtime,
    ));
    let store = Arc::new(ProfileStoreCapture::new());
    let profiles = profiles_for_mihomo(store);
    let reader = Arc::new(
        ProductSurfaceReader::new(core.clone(), SurfaceKind::BevyDesktop, HostKind::Desktop)
            .with_profiles(profiles.clone()),
    );
    // The profile commands are Core backend commands: the shared bus admits
    // them only with a validated Core scope, so the capture drives the Core
    // owner's scoped route instead of the unscoped one.
    app.insert_resource(CommandSinkHandle(Arc::new(CaptureCommandSink::scoped(
        core.clone(),
        capture_command_application()
            .with_profile(profiles)
            .with_subscription_source(Arc::new(SubscriptionSourceCapture::refusing(
                "the capture's subscription source refuses the refresh",
            ))),
        CaptureCapability::Profiles,
    ))));
    app.insert_resource(ProfilesCapture {
        stage: ProfilesStage::Unpublished,
        core,
        reader,
    });
}

/// The page's own controls the profiles gestures drive.
#[derive(SystemParam)]
pub struct ProfilesControls<'w, 's> {
    feature: Res<'w, InteractionCapture>,
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, ProfilesCapture>,
    latest: ResMut<'w, LatestSurfaceSnapshot>,
    projection: Option<Res<'w, LastProfilesProjection>>,
    deletes: Query<'w, 's, (Entity, &'static DeleteProfileButton)>,
    updates: Query<'w, 's, (Entity, &'static UpdateProfileButton)>,
    pages: Query<'w, 's, Entity, With<ProfilesPageRoot>>,
}

/// Drive the Profiles page's own deletion and refresh gestures.
pub fn activate(mut controls: ProfilesControls, mut commands: Commands) {
    if controls.route.0 != Some(Route::Profiles) {
        return;
    }
    match controls.feature.feature {
        FeatureId::ProfilesDeleteConfirm => {
            if !selected(
                &controls.feature,
                &controls.route,
                FeatureId::ProfilesDeleteConfirm,
                Route::Profiles,
            ) {
                return;
            }
            activate_delete(&mut controls, &mut commands);
        }
        FeatureId::ProfilesSubscriptionError => {
            if !selected(
                &controls.feature,
                &controls.route,
                FeatureId::ProfilesSubscriptionError,
                Route::Profiles,
            ) {
                return;
            }
            activate_refresh(&mut controls, &mut commands);
        }
        _ => {}
    }
}

/// The profile deletion: the owner publishes the profile list, the page's own
/// delete control asks the owner, and the page renders the owner's answer.
fn activate_delete(controls: &mut ProfilesControls<'_, '_>, commands: &mut Commands) {
    match controls.capture.stage {
        ProfilesStage::Unpublished => {
            publish_readback(controls, commands);
            controls.capture.stage = ProfilesStage::Published;
        }
        ProfilesStage::Published => {
            // Only an inactive profile can be deleted: the page never offers
            // the destructive action for the active profile. The page resolves
            // the button's target through the published projection, so the
            // capture asks the very same projection.
            let published = controls
                .projection
                .as_deref()
                .and_then(|projection| projection.0.as_ref())
                .map(|projection| projection.profiles.as_slice())
                .unwrap_or_default();
            let Some((entity, _)) = controls.deletes.iter().find(|(_, button)| {
                published
                    .get(button.profile_idx)
                    .is_some_and(|profile| !profile.is_active)
            }) else {
                return;
            };
            controls.capture.stage = ProfilesStage::Activated;
            commands.trigger(Activate { entity });
        }
        ProfilesStage::Activated => {
            publish_readback(controls, commands);
        }
    }
}

/// The subscription refresh: the owner publishes the profile list, the page's
/// own refresh control asks the owner, and the page renders the owner's typed
/// refusal.
fn activate_refresh(controls: &mut ProfilesControls<'_, '_>, commands: &mut Commands) {
    match controls.capture.stage {
        ProfilesStage::Unpublished => {
            publish_readback(controls, commands);
            controls.capture.stage = ProfilesStage::Published;
        }
        ProfilesStage::Published => {
            let Some((entity, _)) = controls.updates.iter().next() else {
                return;
            };
            controls.capture.stage = ProfilesStage::Activated;
            commands.trigger(Activate { entity });
        }
        ProfilesStage::Activated => {}
    }
}

/// Publish the capture owner's real readback of the profiles page.
///
/// The capture replaces the page's own data with the owner's readback and
/// leaves the rest of the surface alone: the receipt is about the profiles
/// page, not about the other pages the capture never composed.
fn publish_readback(controls: &mut ProfilesControls<'_, '_>, commands: &mut Commands) {
    let reader = controls.capture.reader.clone();
    let slot = Arc::new(Mutex::new(None));
    let output = slot.clone();
    tokio_application_runtime()
        .expect("isolated profiles capture runtime")
        .block_on(Box::pin(async move {
            *output.lock().expect("capture readback slot") = Some(reader.read().await);
        }));
    let readback = slot
        .lock()
        .expect("capture readback slot")
        .take()
        .expect("the capture's own owner answered")
        .expect("the capture's own owner answers");
    let mut snapshot = controls.latest.0.clone();
    snapshot.pages.profiles = readback.pages.profiles;
    snapshot.revision = controls.latest.0.revision + 1;
    controls.latest.0 = snapshot;
    replay_surface_snapshot(controls.latest.0.clone(), commands);
}

/// The Profiles page's own outcome copy.
#[derive(SystemParam)]
pub struct ProfilesObservation<'w, 's> {
    route: Res<'w, ActiveRoute>,
    capture: Res<'w, ProfilesCapture>,
    deletions: Query<'w, 's, (Entity, &'static Text), With<ProfileDeleteOutcomeText>>,
    refreshes: Query<'w, 's, (Entity, &'static Text), With<ProfileRefreshText>>,
    parents: Query<'w, 's, &'static ChildOf>,
    areas: Query<'w, 's, Entity, With<ScrollArea>>,
}

/// Observe the copy the Profiles page really renders for the business.
pub fn observe(
    controls: ProfilesObservation,
    mut geometry: CaptureGeometry,
    mut state: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
    mut commands: Commands,
) {
    if controls.route.0 != Some(Route::Profiles) {
        return;
    }
    let target = state.feature;
    if controls.capture.stage != ProfilesStage::Activated {
        return;
    }
    let subject = (|| match target {
        FeatureId::ProfilesDeleteConfirm => {
            let (entity, text) = controls.deletions.single().ok()?;
            if text.0.is_empty() {
                return None;
            }
            Some((entity, "profile delete outcome"))
        }
        FeatureId::ProfilesSubscriptionError => {
            let (entity, _text) = controls
                .refreshes
                .iter()
                .find(|(_, text)| !text.0.is_empty())?;
            Some((entity, "profile refresh outcome"))
        }
        _ => None,
    })();
    let Some((entity, label)) = subject else {
        return;
    };
    // The outcome line sits below the fold of the Profiles page, so the
    // capture scrolls the page's own viewport until the line is really
    // rendered, and measures it on the next frame.
    if !geometry.unclipped(entity) {
        if let Some(area) = outermost_scroll_area(entity, &controls.parents, &controls.areas)
            && let Some(viewport) = geometry.rect(area)
            && let Some(bounds) = geometry.rect(entity)
        {
            request_scroll(&mut commands, area, bounds, viewport);
        }
        return;
    }
    let bounds = geometry.bounds(entity, label);
    observation.publish(&mut state, bounds);
}
