//! Composed doctor actions use the same Core command service and shared observations as Bevy.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::doctor::DoctorAction;
use infiltrator_presentation::doctor_actions::DoctorActionPhase;
use lurker_core_contract::page::PageStatus;

#[derive(Clone, Copy)]
enum DoctorActionAvailability {
    Available,
    Unavailable,
}

impl AppState {
    pub(crate) fn shared_doctor_command(&mut self, action: DoctorAction) -> Task<Message> {
        match self.doctor_action_availability(&action) {
            DoctorActionAvailability::Available => {}
            DoctorActionAvailability::Unavailable => return Task::none(),
        }
        let Some(application) = self.command_ingress() else {
            return Task::none();
        };
        let pending = match self.diag.doctor.action.begin(action) {
            Ok(pending) => pending,
            Err(_) => return Task::none(),
        };
        let token = pending.token;
        Task::perform(
            async move {
                match (application.execute(pending.action.intent()).await).into_unit() {
                    Ok(()) => Ok(()),
                    Err(failure) => Err(failure),
                }
            },
            move |result| Message::DoctorCommandFinished { token, result },
        )
    }

    pub(crate) fn shared_retry_doctor_command(&mut self) -> Task<Message> {
        let candidate = match &self.diag.doctor.action.phase {
            DoctorActionPhase::Failed { retry_action, .. } => retry_action.clone(),
            DoctorActionPhase::Idle | DoctorActionPhase::Pending(_) => return Task::none(),
        };
        match self.doctor_action_availability(&candidate) {
            DoctorActionAvailability::Available => {}
            DoctorActionAvailability::Unavailable => return Task::none(),
        }
        let Some(application) = self.command_ingress() else {
            return Task::none();
        };
        let pending = match self.diag.doctor.action.begin_retry() {
            Ok(pending) => pending,
            Err(_) => return Task::none(),
        };
        let token = pending.token;
        Task::perform(
            async move {
                match (application.execute(pending.action.intent()).await).into_unit() {
                    Ok(()) => Ok(()),
                    Err(failure) => Err(failure),
                }
            },
            move |result| Message::DoctorCommandFinished { token, result },
        )
    }

    fn doctor_action_availability(&self, action: &DoctorAction) -> DoctorActionAvailability {
        if !matches!(action, DoctorAction::RepairAll | DoctorAction::RepairOne(_)) {
            return DoctorActionAvailability::Available;
        }
        match self.surface.latest().is_some_and(|snapshot| {
            let page = &snapshot.pages.doctor;
            matches!(page.status, PageStatus::Ready)
                && page.data.as_ref().is_some_and(|data| {
                    data.checks.iter().any(|check| {
                        check.fix_available
                            && match action {
                                DoctorAction::RepairOne(id) => check.id == *id,
                                _ => true,
                            }
                    })
                })
        }) {
            true => DoctorActionAvailability::Available,
            false => DoctorActionAvailability::Unavailable,
        }
    }
}
