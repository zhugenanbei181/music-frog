//! Recover the core lifecycle through the doctor's own bootstrap control.
//!
//! The doctor page carries the lifecycle recovery panel: the bootstrap
//! control asks the shared owner to initialise the engine again. The capture
//! activates that very control, so the surface really submits the shared
//! `BootstrapDoctor` command, and the panel really reports the owner's
//! terminal answer.
use super::capability::capture_command_application;
use super::geometry::CaptureGeometry;
use super::host::{CaptureCapability, CaptureCommandSink};
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::command::CommandSinkHandle;
use crate::pages::doctor::{DoctorPageRoot, DoctorProjectionUpdated};
use crate::pages::doctor_actions::{BootstrapDoctorButton, DoctorFeedbackText};
use crate::pages::page_models::DoctorPageModelState;
use crate::route::{ActiveRoute, Route};
use crate::surface::{LatestSurfaceSnapshot, doctor_projection};
use bevy::app::App;
use bevy::app::Update;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::doctor::DoctorAction;
use infiltrator_contract::parity::FeatureId;
use infiltrator_presentation::doctor_actions::DoctorActionPhase;
use infiltrator_presentation::doctor_projection::project_observation;
use infiltrator_test_support::doctor_capture_fixtures::DoctorCapturePort;
use std::sync::Arc;

/// The capture's own diagnostic owner.
#[derive(Resource)]
struct RecoveryCapture {
    application: DoctorApplication,
    initialized: bool,
    submitted: bool,
    published: bool,
}

pub fn install(app: &mut App) {
    let port = Arc::new(DoctorCapturePort::default());
    let application = DoctorApplication::new(port.clone());
    app.insert_resource(CommandSinkHandle(Arc::new(CaptureCommandSink::new(
        capture_command_application().with_doctor(application.clone()),
        CaptureCapability::DoctorBootstrap,
    ))));
    app.insert_resource(RecoveryCapture {
        application,
        initialized: false,
        submitted: false,
        published: false,
    });
    app.add_systems(Update, initialize.before(activate));
}

/// Publish the owner's diagnostic page so the recovery panel renders the real
/// check list before the capture touches the bootstrap control.
fn initialize(
    mut capture: ResMut<RecoveryCapture>,
    mut latest: ResMut<LatestSurfaceSnapshot>,
    mut commands: Commands,
) {
    if capture.initialized {
        return;
    }
    let application = capture.application.clone();
    tokio_application_runtime()
        .expect("isolated runtime")
        .block_on(Box::pin(async move {
            application.run(None).await.expect("initial diagnosis");
        }));
    latest.0.pages.doctor = project_observation(
        &capture.application.observation(),
        &capture.application.list_checks(),
    );
    commands.trigger(DoctorProjectionUpdated(doctor_projection(&latest.0)));
    capture.initialized = true;
}

/// The recovery panel's own controls and its shared action state.
#[derive(SystemParam)]
pub struct RecoverySurface<'w, 's> {
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, RecoveryCapture>,
    model: Res<'w, DoctorPageModelState>,
    latest: ResMut<'w, LatestSurfaceSnapshot>,
    buttons: Query<'w, 's, Entity, With<BootstrapDoctorButton>>,
    feedback: Query<'w, 's, (Entity, &'static Text), With<DoctorFeedbackText>>,
    pages: Query<'w, 's, Entity, With<DoctorPageRoot>>,
}

/// Activate the recovery panel's own bootstrap control.
pub fn activate(
    mut surface: RecoverySurface,
    feature: Res<InteractionCapture>,
    mut commands: Commands,
) {
    if !selected(
        &feature,
        &surface.route,
        FeatureId::LifecycleRecoveryPanel,
        Route::Doctor,
    ) {
        return;
    }
    if !surface.capture.initialized {
        return;
    }
    if !surface.capture.submitted {
        if surface.model.0.is_busy() {
            return;
        }
        let Some(entity) = surface.buttons.iter().next() else {
            return;
        };
        surface.capture.submitted = true;
        commands.trigger(Activate { entity });
        return;
    }
    if surface.capture.published {
        return;
    }
    let failed = matches!(
        &surface.model.0.action().phase,
        DoctorActionPhase::Failed { .. }
    );
    if !failed {
        return;
    }
    surface.capture.published = true;
    surface.latest.0.pages.doctor = project_observation(
        &surface.capture.application.observation(),
        &surface.capture.application.list_checks(),
    );
    let projection = doctor_projection(&surface.latest.0);
    commands.trigger(DoctorProjectionUpdated(projection));
}

/// Observe the recovery panel's own failure guidance.
pub fn observe(
    surface: RecoverySurface,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
) {
    let bounds = (|| {
        if !surface.capture.published {
            return None;
        }
        let failure = match &surface.model.0.action().phase {
            DoctorActionPhase::Failed {
                failure,
                retry_action,
            } => {
                if *retry_action != DoctorAction::Bootstrap {
                    return None;
                }
                failure
            }
            DoctorActionPhase::Idle | DoctorActionPhase::Pending(_) => return None,
        };
        let (entity, text) = surface.feedback.iter().next()?;
        if text.0 != failure.message {
            return None;
        }
        let feedback = geometry.bounds(entity, "lifecycle recovery guidance")?;
        let page = geometry.bounds(surface.pages.iter().next()?, "recovery panel")?;
        if feedback[0] < page[0]
            || feedback[1] < page[1]
            || feedback[0] + feedback[2] > page[0] + page[2]
            || feedback[1] + feedback[3] > page[1] + page[3]
        {
            return None;
        }
        Some(page)
    })();
    observation.publish(&mut feature, bounds);
}
