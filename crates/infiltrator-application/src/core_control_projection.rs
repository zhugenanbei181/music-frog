//! One lifecycle control decision for both peer products, including busy and capability failures.
use infiltrator_contract::capability::Availability;
use infiltrator_contract::core_control::{
    CoreControlAction, CoreControlLabel, CoreControlProjection,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};

pub fn project_core_control(
    core: &CoreLifecycleSnapshot,
    availability: Option<&Availability>,
    request_pending: bool,
    command_failure: Option<&Failure>,
) -> CoreControlProjection {
    let unavailable = match availability {
        Some(Availability::Supported | Availability::Experimental) => None,
        Some(Availability::Unsupported { reason }) => {
            Some((CoreControlLabel::Unsupported, Failure::unsupported(reason)))
        }
        Some(Availability::Unavailable { reason }) => Some((
            CoreControlLabel::Unavailable,
            Failure::new(ErrorCode::NotReady, reason, true),
        )),
        None => Some((
            CoreControlLabel::Unavailable,
            Failure::new(
                ErrorCode::NotReady,
                "core lifecycle capability has not been composed",
                true,
            ),
        )),
    };
    if let Some((label, failure)) = unavailable {
        return CoreControlProjection {
            action: None,
            label,
            issue: Some(failure),
        };
    }
    let (label, action) = if request_pending {
        (CoreControlLabel::Pending, None)
    } else {
        match core.lifecycle {
            CoreLifecycle::Stopped => (CoreControlLabel::Start, Some(CoreControlAction::Start)),
            CoreLifecycle::Running | CoreLifecycle::Ready => {
                (CoreControlLabel::Stop, Some(CoreControlAction::Stop))
            }
            CoreLifecycle::Failed => (CoreControlLabel::Retry, Some(CoreControlAction::Start)),
            CoreLifecycle::Starting => (CoreControlLabel::Starting, None),
            CoreLifecycle::Stopping => (CoreControlLabel::Stopping, None),
        }
    };
    CoreControlProjection {
        action,
        label,
        issue: command_failure.cloned().or_else(|| core.failure.clone()),
    }
}

#[cfg(test)]
#[path = "core_control_projection_tests.rs"]
mod tests;
