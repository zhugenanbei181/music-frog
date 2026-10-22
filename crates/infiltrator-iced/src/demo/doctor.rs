//! Capture executes the actual diagnostic TEA task and observes its failed reader state.
use super::capabilities::DemoHostCapabilityProvider;
use crate::demo::owner::attach_capture_owner_with_snapshot;
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::doctor::{
    BootstrapReport, DoctorCheckMeta, DoctorFixReport, DoctorReport,
};
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_ports::doctor::DoctorPort;
use infiltrator_presentation::doctor_projection::project_observation;
use infiltrator_test_support::doctor_capture_fixtures::DoctorCapturePort;
use infiltrator_test_support::language_choice_fixtures::LanguageCaptureProcess;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_ports::error::PortError;
use std::future::pending;
use std::sync::Arc;

/// The diagnostic host whose repair answer the capture holds.
///
/// The owner publishes its pending phase: the repair command is submitted and
/// the host keeps its answer, so the surface shows the progress the owner
/// really published instead of a locally drawn spinner.
#[derive(Default)]
struct HoldingDoctorPort {
    inner: DoctorCapturePort,
}

#[async_trait::async_trait]
impl DoctorPort for HoldingDoctorPort {
    async fn run(&self, filter: Option<String>) -> Result<DoctorReport, PortError> {
        self.inner.run(filter).await
    }

    async fn fix(&self, _filter: Option<String>) -> Result<DoctorFixReport, PortError> {
        // The host holds its answer: the repair stays pending for the capture.
        pending::<Result<DoctorFixReport, PortError>>().await
    }

    fn list_checks(&self) -> Vec<DoctorCheckMeta> {
        self.inner.list_checks()
    }

    fn explain(&self, check_id: &str) -> Result<DoctorCheckMeta, PortError> {
        self.inner.explain(check_id)
    }

    async fn bootstrap(&self) -> Result<BootstrapReport, PortError> {
        self.inner.bootstrap().await
    }
}

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    match feature {
        FeatureId::DoctorFailureRecovery => activate_failure_recovery(state),
        FeatureId::DoctorRepairProgress => activate_repair_progress(state),
        _ => Task::none(),
    }
}

/// The diagnosis really runs and really fails: the shared application asks the
/// capture's diagnostic host for its report and the host answers with its
/// typed permission failure, which the surface reports.
fn activate_failure_recovery(state: &mut AppState) -> Task<Message> {
    attach_doctor_owner(state, Arc::new(DoctorCapturePort::default()));
    state.update(Message::RunDoctor)
}

/// The repair really starts and the owner really keeps its answer: the shared
/// command submits the repair, the owner publishes its pending phase and the
/// surface renders that phase.
fn activate_repair_progress(state: &mut AppState) -> Task<Message> {
    attach_doctor_owner(state, Arc::new(HoldingDoctorPort::default()));
    state.update(Message::RunDoctorFix)
}

/// Bind the capture's diagnostic owner and publish its page observation.
fn attach_doctor_owner(state: &mut AppState, port: Arc<dyn DoctorPort>) {
    let runtime = tokio_application_runtime().expect("isolated capture runtime");
    let doctor = DoctorApplication::new(port);
    let initial = doctor.clone();
    runtime.block_on(Box::pin(async move {
        initial.run(None).await.expect("observed initial diagnosis");
    }));
    let application = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        runtime,
    );
    application.install_command_handler(Arc::new(
        CommandApplication::new()
            .with_host_capabilities(Arc::new(DemoHostCapabilityProvider::new(
                Capability::CoreLifecycle,
            )))
            .with_doctor(doctor.clone()),
    ));
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "doctor capture is preparing", true),
    );
    snapshot.pages.doctor = project_observation(&doctor.observation(), &doctor.list_checks());
    attach_capture_owner_with_snapshot(state, application, snapshot);
    state.diag.doctor.capture_observation = Some(doctor);
}
