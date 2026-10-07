//! Composed doctor actions use the same Core command service and shared observations as Bevy.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::doctor::DoctorAction;
use infiltrator_contract::surface_snapshot::PageStatus;

impl AppState {
    pub(crate) fn shared_doctor_command(&mut self, action: DoctorAction) -> Task<Message> {
        if matches!(action, DoctorAction::RepairAll | DoctorAction::RepairOne(_)) {
            let available = self.surface.latest().is_some_and(|snapshot| {
                let page = &snapshot.pages.doctor;
                matches!(page.status, PageStatus::Ready)
                    && page.data.as_ref().is_some_and(|data| {
                        data.checks.iter().any(|check| {
                            check.fix_available
                                && match &action {
                                    DoctorAction::RepairOne(id) => check.id == *id,
                                    _ => true,
                                }
                        })
                    })
            });
            if !available {
                return Task::none();
            }
        }
        let Some(application) = self.commands.clone() else {
            return Task::none();
        };
        let pending = match self.diag.doctor.action.begin(action) {
            Ok(pending) => pending,
            Err(_) => return Task::none(),
        };
        let token = pending.token;
        self.diag.doctor.error = None;
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
}
