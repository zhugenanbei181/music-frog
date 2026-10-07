//! Doctor observers correlate real results and retain errors on the diagnostic surface.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::pages::doctor::{
    RepairAllDoctorButton, RepairDoctorRowButton, RunDoctorDiagnosticsButton,
};
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::component::Component;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::doctor_actions::DoctorActionState;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_contract::command::RequestId;
use infiltrator_contract::doctor::DoctorAction;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface_snapshot::PageStatus;

#[derive(Resource, Default)]
pub struct DoctorActions {
    pub state: DoctorActionState,
    request: Option<RequestId>,
}
#[derive(Component, Clone, Default)]
#[require(Button, ButtonDisabled)]
pub struct BootstrapDoctorButton;
#[derive(Component, Clone, Default)]
#[require(Button, ButtonDisabled)]
pub struct RetryDoctorButton;
#[derive(Component, Clone, Default)]
pub struct DoctorFeedbackText;

pub fn on_action(
    event: On<Activate>,
    buttons: Query<DoctorButtons>,
    latest: Res<LatestSurfaceSnapshot>,
    sink: Option<Res<CommandSinkHandle>>,
    mut actions: ResMut<DoctorActions>,
) {
    let Ok(button) = buttons.get(event.entity) else {
        return;
    };
    let action = if button.run.is_some() {
        Some(DoctorAction::Diagnose)
    } else if button.repair_all.is_some() {
        Some(DoctorAction::RepairAll)
    } else if let Some(row) = button.repair_one {
        Some(DoctorAction::RepairOne(row.check_id.clone()))
    } else if button.bootstrap.is_some() {
        Some(DoctorAction::Bootstrap)
    } else if button.retry.is_some() {
        if !actions.state.can_retry() {
            return;
        }
        actions.state.retry_action.clone()
    } else {
        None
    };
    let Some(action) = action else { return };
    if matches!(action, DoctorAction::RepairAll | DoctorAction::RepairOne(_)) {
        let page = &latest.0.pages.doctor;
        let available = matches!(page.status, PageStatus::Ready)
            && page.data.as_ref().is_some_and(|data| {
                data.checks.iter().any(|check| {
                    check.fix_available
                        && match &action {
                            DoctorAction::RepairOne(id) => check.id == *id,
                            _ => true,
                        }
                })
            });
        if !available {
            return;
        }
    }
    let pending = match actions.state.begin(action.clone()) {
        Ok(pending) => pending,
        Err(_) => return,
    };
    let command = match action {
        DoctorAction::Diagnose => UiCommand::RunDoctorDiagnostics,
        DoctorAction::RepairAll => UiCommand::RepairAllDoctorIssues,
        DoctorAction::RepairOne(check_id) => UiCommand::RepairDoctorIssue { check_id },
        DoctorAction::Bootstrap => UiCommand::BootstrapDoctor,
    };
    if let Some(request) = sink.and_then(|sink| sink.submit_tracked(command)) {
        actions.request = Some(request);
    } else {
        actions.state.finish(
            pending.token,
            Err(Failure::new(
                ErrorCode::NotReady,
                "diagnostic command service is unavailable",
                true,
            )),
        );
    }
}
pub fn on_result(event: On<CommandExecutedEvent>, mut actions: ResMut<DoctorActions>) {
    let Some(pending) = actions.state.pending.as_ref() else {
        return;
    };
    if actions.request != Some(event.request_id)
        || event.command.to_intent().as_ref() != Some(&pending.action.intent())
    {
        return;
    }
    let token = pending.token;
    if actions.state.finish(token, event.unit_result()) {
        actions.request = None
    }
}
#[derive(QueryData)]
pub struct DoctorButtons {
    run: Option<&'static RunDoctorDiagnosticsButton>,
    repair_all: Option<&'static RepairAllDoctorButton>,
    repair_one: Option<&'static RepairDoctorRowButton>,
    bootstrap: Option<&'static BootstrapDoctorButton>,
    retry: Option<&'static RetryDoctorButton>,
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct DoctorControls {
    disabled: &'static mut ButtonDisabled,
    run: Option<&'static RunDoctorDiagnosticsButton>,
    repair_all: Option<&'static RepairAllDoctorButton>,
    repair_one: Option<&'static RepairDoctorRowButton>,
    bootstrap: Option<&'static BootstrapDoctorButton>,
    retry: Option<&'static RetryDoctorButton>,
}
pub fn sync_controls(
    actions: Res<DoctorActions>,
    latest: Res<LatestSurfaceSnapshot>,
    locale: Res<UiLocale>,
    mut controls: Query<DoctorControls>,
    mut feedback: Query<&mut Text, With<DoctorFeedbackText>>,
) {
    let busy = actions.state.pending.is_some();
    let page = &latest.0.pages.doctor;
    for mut control in &mut controls {
        if control.run.is_none()
            && control.repair_all.is_none()
            && control.repair_one.is_none()
            && control.bootstrap.is_none()
            && control.retry.is_none()
        {
            continue;
        }
        let disabled = busy
            || if control.retry.is_some() {
                !actions.state.can_retry()
            } else if control.repair_all.is_some() || control.repair_one.is_some() {
                !matches!(page.status, PageStatus::Ready)
                    || page.data.as_ref().is_none_or(|data| {
                        !data.checks.iter().any(|check| {
                            check.fix_available
                                && control
                                    .repair_one
                                    .is_none_or(|row| row.check_id == check.id)
                        })
                    })
            } else {
                false
            };
        if control.disabled.0 != disabled {
            control.disabled.0 = disabled
        }
    }
    let value = actions
        .state
        .failure
        .as_ref()
        .map(|failure| failure.message.clone())
        .unwrap_or_else(|| {
            if busy {
                LocalizedText::plain("doctor_operation_pending").render(&locale)
            } else {
                String::new()
            }
        });
    for mut text in &mut feedback {
        if text.0 != value {
            text.0 = value.clone()
        }
    }
}
