//! Diagnostic captures activate the native button and retain the real failed command observation.
use super::geometry::CaptureGeometry;
use super::host::{CaptureCapability, CaptureCommandSink};
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::command::CommandSinkHandle;
use crate::pages::doctor::{
    DoctorPageRoot, DoctorProjectionUpdated, RepairDoctorRowButton, RunDoctorDiagnosticsButton,
};
use crate::pages::doctor_actions::{DoctorActions, DoctorFeedbackText, RetryDoctorButton};
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
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_application::doctor_capture_fixtures::{CHECK_ID, DoctorCapturePort};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface_snapshot::PageStatus;
use std::sync::Arc;
#[derive(Resource)]
struct CaptureDoctor {
    application: DoctorApplication,
    port: Arc<DoctorCapturePort>,
    started: bool,
    initialized: bool,
}
pub fn install(app: &mut App) {
    let port = Arc::new(DoctorCapturePort::default());
    let doctor = DoctorApplication::new(port.clone());
    app.insert_resource(CommandSinkHandle(Arc::new(CaptureCommandSink::new(
        CommandApplication::new().with_doctor(doctor.clone()),
        CaptureCapability::Doctor,
    ))));
    app.insert_resource(CaptureDoctor {
        application: doctor,
        port,
        started: false,
        initialized: false,
    });
    app.add_systems(Update, initialize.before(activate));
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
    latest.0.pages.doctor = capture.application.page();
    commands.trigger(DoctorProjectionUpdated(doctor_projection(&latest.0)));
    capture.initialized = true;
}
#[derive(SystemParam)]
pub struct DiagnosticActivation<'w, 's> {
    feature: Res<'w, InteractionCapture>,
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, CaptureDoctor>,
    actions: Res<'w, DoctorActions>,
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
    let page = diagnosis.capture.application.page();
    if diagnosis.latest.0.pages.doctor != page {
        diagnosis.latest.0.pages.doctor = page;
        commands.trigger(DoctorProjectionUpdated(doctor_projection(
            &diagnosis.latest.0,
        )));
    }
    if diagnosis.capture.started || diagnosis.actions.state.pending.is_some() {
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
    actions: Res<'w, DoctorActions>,
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
        let actions = &self.actions.state;
        if !self.capture.started
            || !actions.can_retry()
            || actions.failure.as_ref()?.code != ErrorCode::Permission
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
        let (finding, _, disabled, sdk_disabled, accessibility) = self
            .repairs
            .iter()
            .find(|(_, row, _, _, _)| row.check_id == CHECK_ID)?;
        if !disabled.0 || sdk_disabled.is_none() || !accessibility.is_disabled() {
            return None;
        }
        geometry.bounds(finding, "invalid diagnostic repair")?;
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
