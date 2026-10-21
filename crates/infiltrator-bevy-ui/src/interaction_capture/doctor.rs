//! Diagnostic captures activate the native button and retain the real failed command observation.
use super::capability::capture_command_application;
use super::geometry::CaptureGeometry;
use super::host::{AnswerDelivery, CaptureCapability, CaptureCommandSink};
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::command::CommandSinkHandle;
use crate::pages::doctor::spinner::DoctorSpinner;
use crate::pages::doctor::{
    DoctorPageRoot, DoctorProjectionUpdated, RepairDoctorRowButton, RunDoctorDiagnosticsButton,
};
use crate::pages::doctor_actions::{DoctorFeedbackText, RetryDoctorButton};
use crate::pages::page_models::DoctorPageModelState;
use crate::route::{ActiveRoute, Route};
use crate::surface::{LatestSurfaceSnapshot, doctor_projection};
use bevy::a11y::AccessibilityNode;
use bevy::app::App;
use bevy::app::Update;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ecs::{entity::Entity, resource::Resource};
use bevy::ui::InteractionDisabled;
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::parity::FeatureId;
use infiltrator_presentation::doctor_actions::DoctorActionPhase;
use infiltrator_presentation::doctor_projection::project_observation;
use infiltrator_test_support::doctor_capture_fixtures::{CHECK_ID, DoctorCapturePort};
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::page::PageStatus;
use std::sync::Arc;

/// The repair-progress business identity.
///
/// The parity registry owns the feature's identity; the capture names it by
/// the registry's stable id, the same identity the capture rig passes as the
/// scenario name. When the registry row lands, this constant becomes the
/// typed `FeatureId::DoctorRepairProgress` variant.
pub const REPAIR_PROGRESS_FEATURE_ID: &str = "doctor-repair-progress";

#[derive(Resource)]
struct CaptureDoctor {
    application: DoctorApplication,
    port: Arc<DoctorCapturePort>,
    started: bool,
    initialized: bool,
}
pub fn install(app: &mut App) {
    install_with(app, AnswerDelivery::Deliver);
    app.add_systems(Update, initialize.before(activate));
}

/// Install the doctor's capture with the capture's own answer delivery.
fn install_with(app: &mut App, delivery: AnswerDelivery) {
    let port = Arc::new(DoctorCapturePort::default());
    let doctor = DoctorApplication::new(port.clone());
    app.insert_resource(CommandSinkHandle(Arc::new(
        CaptureCommandSink::new(
            capture_command_application().with_doctor(doctor.clone()),
            CaptureCapability::Doctor,
        )
        .with_delivery(delivery),
    )));
    app.insert_resource(CaptureDoctor {
        application: doctor,
        port,
        started: false,
        initialized: false,
    });
}
fn initialize(
    mut capture: ResMut<CaptureDoctor>,
    mut latest: ResMut<LatestSurfaceSnapshot>,
    mut commands: Commands,
) {
    if capture.initialized {
        return;
    }
    let initial = capture.application.clone();
    tokio_application_runtime()
        .expect("isolated runtime")
        .block_on(Box::pin(async move {
            initial.run(None).await.expect("initial diagnosis");
        }));
    latest.0.pages.doctor = project_observation(
        &capture.application.observation(),
        &capture.application.list_checks(),
    );
    commands.trigger(DoctorProjectionUpdated(doctor_projection(&latest.0)));
    capture.initialized = true;
}
#[derive(SystemParam)]
pub struct DiagnosticActivation<'w, 's> {
    feature: Res<'w, InteractionCapture>,
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, CaptureDoctor>,
    actions: Res<'w, DoctorPageModelState>,
    latest: ResMut<'w, LatestSurfaceSnapshot>,
    run: Query<'w, 's, Entity, With<RunDoctorDiagnosticsButton>>,
}
pub fn activate(mut diagnosis: DiagnosticActivation, mut commands: Commands) {
    if !selected(
        &diagnosis.feature,
        &diagnosis.route,
        FeatureId::DoctorFailureRecovery,
        Route::Doctor,
    ) {
        return;
    }
    let page = project_observation(
        &diagnosis.capture.application.observation(),
        &diagnosis.capture.application.list_checks(),
    );
    if diagnosis.latest.0.pages.doctor != page {
        diagnosis.latest.0.pages.doctor = page;
        commands.trigger(DoctorProjectionUpdated(doctor_projection(
            &diagnosis.latest.0,
        )));
    }
    if diagnosis.capture.started
        || matches!(
            &diagnosis.actions.0.action().phase,
            DoctorActionPhase::Pending(_)
        )
    {
        return;
    }
    let Some(entity) = diagnosis.run.iter().next() else {
        return;
    };
    diagnosis.capture.started = true;
    commands.trigger(Activate { entity });
}
#[derive(SystemParam)]
pub struct DiagnosticObservation<'w, 's> {
    capture: Res<'w, CaptureDoctor>,
    actions: Res<'w, DoctorPageModelState>,
    latest: Res<'w, LatestSurfaceSnapshot>,
    retry: Query<'w, 's, (Entity, &'static ButtonDisabled), With<RetryDoctorButton>>,
    repairs: Query<
        'w,
        's,
        (
            Entity,
            &'static RepairDoctorRowButton,
            &'static ButtonDisabled,
            Option<&'static InteractionDisabled>,
            &'static AccessibilityNode,
        ),
    >,
    feedback: Query<'w, 's, (Entity, &'static Text), With<DoctorFeedbackText>>,
    pages: Query<'w, 's, Entity, With<DoctorPageRoot>>,
}
impl DiagnosticObservation<'_, '_> {
    fn bounds(&self, geometry: &mut CaptureGeometry) -> Option<[f32; 4]> {
        let actions = &self.actions.0.action();
        if !self.capture.started
            || !matches!(
                &actions.phase,
                DoctorActionPhase::Failed { failure, .. }
                    if failure.code == ErrorCode::Permission
            )
            || self.capture.port.run_count() != 2
        {
            return None;
        }
        let page = &self.latest.0.pages.doctor;
        if !matches!(page.status, PageStatus::Failed { .. })
            || page.data.as_ref()?.checks.first()?.id != CHECK_ID
        {
            return None;
        }
        let (retry, disabled) = self.retry.iter().next()?;
        if disabled.0 {
            return None;
        }
        let retry = geometry.bounds(retry, "diagnostic retry")?;
        // The failed check's repair control is the page's own typed fact: it
        // is disabled three ways (the button, the interaction and the
        // accessibility node), so the capture proves the control cannot be
        // activated. Its own box is not measured: the compact viewport
        // cannot show the header's recovery controls and the check row at
        // the same time, so the capture measures the recovery control the
        // surface offers — the same region the Iced peer measures — and the
        // failed check stays a fact of the page's projection.
        let (_, _, disabled, sdk_disabled, accessibility) = self
            .repairs
            .iter()
            .find(|(_, row, _, _, _)| row.check_id == CHECK_ID)?;
        if !disabled.0 || sdk_disabled.is_none() || !accessibility.is_disabled() {
            return None;
        }
        let (feedback, copy) = self.feedback.iter().next()?;
        if !copy.0.contains("Allow diagnostic access") {
            return None;
        }
        geometry.bounds(feedback, "diagnostic error guidance")?;
        let page = geometry.bounds(self.pages.iter().next()?, "diagnostic panel")?;
        if retry[0] < page[0]
            || retry[1] < page[1]
            || retry[0] + retry[2] > page[0] + page[2]
            || retry[1] + retry[3] > page[1] + page[3]
        {
            return None;
        }
        Some(page)
    }
}
pub fn observe(
    diagnosis: DiagnosticObservation,
    mut geometry: CaptureGeometry,
    mut state: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
) {
    observation.publish(&mut state, diagnosis.bounds(&mut geometry));
}

// ---- The repair-progress capture: the page's own progress indicator ----
//
// The doctor's rotating indicator is a restatement of the shared phase: it
// turns exactly while the owner owes an answer and disappears with the
// phase. A capture that wants a pixel receipt of that indicator therefore
// needs the phase to hold at `Pending`, so the capture keeps the owner's
// terminal answer: the surface keeps the transient phase, and the ring
// keeps turning on screen.

/// The capture's own progress: the diagnosis is submitted exactly once.
#[derive(Resource, Default)]
struct RepairProgressCapture {
    started: bool,
}

/// Install the repair-progress capture: the page's own progress indicator.
pub fn install_repair_progress(app: &mut App) {
    install_with(app, AnswerDelivery::Hold);
    app.init_resource::<RepairProgressCapture>();
    // The initial diagnosis answers before the capture submits its own, so
    // the owner's own answer is the first one the page ever shows.
    app.add_systems(Update, initialize.before(activate_repair_progress));
}

#[derive(SystemParam)]
pub struct RepairProgressActivation<'w, 's> {
    feature: Res<'w, InteractionCapture>,
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, RepairProgressCapture>,
    run: Query<'w, 's, Entity, With<RunDoctorDiagnosticsButton>>,
}

/// Submit the page's own diagnostic control once.
pub fn activate_repair_progress(mut activation: RepairProgressActivation, mut commands: Commands) {
    if activation.feature.feature.spec().id != REPAIR_PROGRESS_FEATURE_ID
        || activation.route.0 != Some(Route::Doctor)
        || activation.capture.started
    {
        return;
    }
    let Some(entity) = activation.run.iter().next() else {
        return;
    };
    activation.capture.started = true;
    commands.trigger(Activate { entity });
}

#[derive(SystemParam)]
pub struct RepairProgressObservation<'w, 's> {
    actions: Res<'w, DoctorPageModelState>,
    spinners: Query<'w, 's, Entity, With<DoctorSpinner>>,
    pages: Query<'w, 's, Entity, With<DoctorPageRoot>>,
}

/// Observe the indicator while the shared phase is pending.
pub fn observe_repair_progress(
    observation: RepairProgressObservation,
    mut geometry: CaptureGeometry,
    mut state: ResMut<InteractionCapture>,
    mut published: ResMut<ObservedInteraction>,
) {
    let bounds = (|| {
        if state.feature.spec().id != REPAIR_PROGRESS_FEATURE_ID {
            return None;
        }
        // The ring turns exactly while the owner owes an answer: the phase
        // and the indicator are the same fact.
        if !matches!(
            &observation.actions.0.action().phase,
            DoctorActionPhase::Pending(_)
        ) {
            return None;
        }
        let ring = geometry.bounds(
            observation.spinners.iter().next()?,
            "diagnostic progress ring",
        )?;
        let page = geometry.bounds(observation.pages.iter().next()?, "diagnostic panel")?;
        if ring[0] < page[0]
            || ring[1] < page[1]
            || ring[0] + ring[2] > page[0] + page[2]
            || ring[1] + ring[3] > page[1] + page[3]
        {
            return None;
        }
        Some(page)
    })();
    published.publish(&mut state, bounds);
}
