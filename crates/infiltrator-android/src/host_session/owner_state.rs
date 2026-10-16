use super::*;

pub(crate) enum SharedCoreOwnerState {
    Uninitialized,
    Recovering,
    Ready,
    Active(Arc<SharedCore>),
    Switching(Arc<SharedCore>),
    RecoveryRequired,
    Closing(Arc<SharedCore>),
    ClosingWithoutCore,
    Closed,
}

pub(crate) enum SharedCoreOwnerAdmission<'a> {
    Existing(&'a Arc<SharedCore>),
    Build,
    RecoveryRequired,
    Switching,
    Closing,
}

pub(crate) enum SharedCoreOwnerSwitchCompletion {
    Installed(Arc<SharedCore>),
    RestorePrevious,
    RecoveryRequired,
}

pub(crate) enum SharedCoreOwnerSwitchFinish {
    Installed,
    Restored,
    RecoveryRequired,
    OwnerStateChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SharedCoreOwnerSessionCompletion {
    Ready,
    AlreadyReady,
    RecoveryNotStarted,
    RecoveryRequired,
    Closing,
}

pub(super) fn begin_owner_session_transition(state: &mut SharedCoreOwnerState) -> SharedCoreOwnerSessionBegin {
    match state {
        SharedCoreOwnerState::Uninitialized
        | SharedCoreOwnerState::RecoveryRequired
        | SharedCoreOwnerState::Closed => {
            *state = SharedCoreOwnerState::Recovering;
            SharedCoreOwnerSessionBegin::NewSession
        }
        SharedCoreOwnerState::Recovering => SharedCoreOwnerSessionBegin::RecoveryInProgress,
        SharedCoreOwnerState::Ready | SharedCoreOwnerState::Active(_) => {
            SharedCoreOwnerSessionBegin::AlreadyReady
        }
        SharedCoreOwnerState::Switching(_) => SharedCoreOwnerSessionBegin::RecoveryInProgress,
        SharedCoreOwnerState::Closing(_) | SharedCoreOwnerState::ClosingWithoutCore => {
            SharedCoreOwnerSessionBegin::RetryClose
        }
    }
}

pub(super) fn complete_owner_session_transition(
    state: &mut SharedCoreOwnerState,
) -> SharedCoreOwnerSessionCompletion {
    match state {
        SharedCoreOwnerState::Recovering => {
            *state = SharedCoreOwnerState::Ready;
            SharedCoreOwnerSessionCompletion::Ready
        }
        SharedCoreOwnerState::Ready | SharedCoreOwnerState::Active(_) => {
            SharedCoreOwnerSessionCompletion::AlreadyReady
        }
        SharedCoreOwnerState::Uninitialized => SharedCoreOwnerSessionCompletion::RecoveryNotStarted,
        SharedCoreOwnerState::Switching(_) => SharedCoreOwnerSessionCompletion::Closing,
        SharedCoreOwnerState::RecoveryRequired => {
            SharedCoreOwnerSessionCompletion::RecoveryRequired
        }
        SharedCoreOwnerState::Closing(_)
        | SharedCoreOwnerState::ClosingWithoutCore
        | SharedCoreOwnerState::Closed => SharedCoreOwnerSessionCompletion::Closing,
    }
}

pub(super) fn shared_core_owner_admission(state: &SharedCoreOwnerState) -> SharedCoreOwnerAdmission<'_> {
    match state {
        SharedCoreOwnerState::Active(core) => SharedCoreOwnerAdmission::Existing(core),
        SharedCoreOwnerState::Ready => SharedCoreOwnerAdmission::Build,
        SharedCoreOwnerState::Uninitialized
        | SharedCoreOwnerState::Recovering
        | SharedCoreOwnerState::RecoveryRequired => SharedCoreOwnerAdmission::RecoveryRequired,
        SharedCoreOwnerState::Switching(_) => SharedCoreOwnerAdmission::Switching,
        SharedCoreOwnerState::Closing(_)
        | SharedCoreOwnerState::ClosingWithoutCore
        | SharedCoreOwnerState::Closed => SharedCoreOwnerAdmission::Closing,
    }
}

pub(crate) fn begin_shared_core_switch(
    state: &mut SharedCoreOwnerState,
) -> Result<Arc<SharedCore>, SharedCoreOwnerAdmission<'static>> {
    match state {
        SharedCoreOwnerState::Active(core) => {
            let previous = Arc::clone(core);
            *state = SharedCoreOwnerState::Switching(Arc::clone(&previous));
            Ok(previous)
        }
        SharedCoreOwnerState::Switching(_) => Err(SharedCoreOwnerAdmission::Switching),
        SharedCoreOwnerState::Uninitialized
        | SharedCoreOwnerState::Recovering
        | SharedCoreOwnerState::RecoveryRequired => Err(SharedCoreOwnerAdmission::RecoveryRequired),
        SharedCoreOwnerState::Ready => Err(SharedCoreOwnerAdmission::Build),
        SharedCoreOwnerState::Closing(_)
        | SharedCoreOwnerState::ClosingWithoutCore
        | SharedCoreOwnerState::Closed => Err(SharedCoreOwnerAdmission::Closing),
    }
}

pub(crate) fn finish_shared_core_switch(
    state: &mut SharedCoreOwnerState,
    previous: &Arc<SharedCore>,
    completion: SharedCoreOwnerSwitchCompletion,
) -> SharedCoreOwnerSwitchFinish {
    match state {
        SharedCoreOwnerState::Switching(current) if Arc::ptr_eq(current, previous) => {
            match completion {
                SharedCoreOwnerSwitchCompletion::Installed(candidate) => {
                    *state = SharedCoreOwnerState::Active(candidate);
                    SharedCoreOwnerSwitchFinish::Installed
                }
                SharedCoreOwnerSwitchCompletion::RestorePrevious => {
                    *state = SharedCoreOwnerState::Active(Arc::clone(previous));
                    SharedCoreOwnerSwitchFinish::Restored
                }
                SharedCoreOwnerSwitchCompletion::RecoveryRequired => {
                    *state = SharedCoreOwnerState::RecoveryRequired;
                    SharedCoreOwnerSwitchFinish::RecoveryRequired
                }
            }
        }
        SharedCoreOwnerState::Switching(_) => SharedCoreOwnerSwitchFinish::OwnerStateChanged,
        _ => SharedCoreOwnerSwitchFinish::OwnerStateChanged,
    }
}


#[cfg(test)]
mod shared_core_owner_session_state_tests {
    use super::{
        SharedCoreOwnerAdmission, SharedCoreOwnerSessionBegin, SharedCoreOwnerSessionCompletion,
        SharedCoreOwnerState, begin_owner_session_transition, complete_owner_session_transition,
        shared_core_owner_admission,
    };

    #[test]
    fn shared_core_is_unavailable_until_startup_recovery_completes() {
        let mut state = SharedCoreOwnerState::Uninitialized;
        assert!(matches!(
            shared_core_owner_admission(&state),
            SharedCoreOwnerAdmission::RecoveryRequired
        ));
        assert_eq!(
            begin_owner_session_transition(&mut state),
            SharedCoreOwnerSessionBegin::NewSession
        );
        assert!(matches!(
            shared_core_owner_admission(&state),
            SharedCoreOwnerAdmission::RecoveryRequired
        ));
        assert_eq!(
            begin_owner_session_transition(&mut state),
            SharedCoreOwnerSessionBegin::RecoveryInProgress
        );
        assert_eq!(
            complete_owner_session_transition(&mut state),
            SharedCoreOwnerSessionCompletion::Ready
        );
        assert!(matches!(
            shared_core_owner_admission(&state),
            SharedCoreOwnerAdmission::Build
        ));
        assert_eq!(
            begin_owner_session_transition(&mut state),
            SharedCoreOwnerSessionBegin::AlreadyReady
        );
    }

    #[test]
    fn closing_or_unstarted_owner_cannot_be_marked_ready() {
        let mut uninitialized = SharedCoreOwnerState::Uninitialized;
        assert_eq!(
            complete_owner_session_transition(&mut uninitialized),
            SharedCoreOwnerSessionCompletion::RecoveryNotStarted
        );

        let mut closing = SharedCoreOwnerState::ClosingWithoutCore;
        assert_eq!(
            begin_owner_session_transition(&mut closing),
            SharedCoreOwnerSessionBegin::RetryClose
        );
        assert_eq!(
            complete_owner_session_transition(&mut closing),
            SharedCoreOwnerSessionCompletion::Closing
        );
        assert!(matches!(
            shared_core_owner_admission(&closing),
            SharedCoreOwnerAdmission::Closing
        ));
    }
}
