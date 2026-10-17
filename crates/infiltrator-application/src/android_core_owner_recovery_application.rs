//! Ordered restart recovery for the Android Core/VPN owner.

use crate::vpn_application::VpnServiceApplication;
use infiltrator_contract::android_control::{
    AndroidCoreProcessObservation, AndroidCoreProcessRecoveryReceipt,
};
use infiltrator_ports::active_profile_reference_store::{
    ActiveProfileOwnerRecoveryEvidence, ActiveProfileOwnerRecoveryOutcome,
    ActiveProfileReferenceStore,
};
use infiltrator_ports::android_core_process_recovery::AndroidCoreProcessRecoveryPort;
use lurker_core_contract::active_profile_reference::ActiveProfileReferenceState;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreIdentity;
use lurker_core_contract::lifecycle::{CoreLifecycle, CoreLifecycleSnapshot};
use std::sync::Arc;

#[cfg(test)]
#[path = "android_core_owner_recovery_application_tests.rs"]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AndroidCoreOwnerRecoveryOutcome {
    NoPreviousOwner,
    Recovered(ActiveProfileReferenceState),
}

enum RecoveryCandidate {
    NeverStarted {
        terminal: CoreLifecycleSnapshot,
    },
    ManagedChildExited {
        context: CoreProcessStartContext,
        terminal: CoreLifecycleSnapshot,
    },
}

#[derive(Clone)]
pub struct AndroidCoreOwnerRecoveryApplication {
    process: Arc<dyn AndroidCoreProcessRecoveryPort>,
    owner_store: Arc<dyn ActiveProfileReferenceStore>,
    vpn: VpnServiceApplication,
    identity: CoreIdentity,
}

impl AndroidCoreOwnerRecoveryApplication {
    pub fn new(
        process: Arc<dyn AndroidCoreProcessRecoveryPort>,
        owner_store: Arc<dyn ActiveProfileReferenceStore>,
        vpn: VpnServiceApplication,
        identity: CoreIdentity,
    ) -> Self {
        Self {
            process,
            owner_store,
            vpn,
            identity,
        }
    }

    /// Keep provider binding closed until the process record, persisted owner
    /// proof and VPN terminal state agree on one recovery transaction.
    pub async fn recover(&self) -> Result<AndroidCoreOwnerRecoveryOutcome, Failure> {
        let observation = self.process.observe().await.map_err(Failure::from)?;
        let candidate = match observation {
            AndroidCoreProcessObservation::NeverStarted => RecoveryCandidate::NeverStarted {
                terminal: CoreLifecycleSnapshot {
                    identity: Some(self.identity.clone()),
                    lifecycle: CoreLifecycle::Stopped,
                    generation: 0,
                    session_token: None,
                    revision: 0,
                    failure: None,
                },
            },
            AndroidCoreProcessObservation::Exited { owned, .. } => {
                let terminal = stopped_snapshot_for_context(&owned.context, &self.identity)?;
                RecoveryCandidate::ManagedChildExited {
                    context: owned.context,
                    terminal,
                }
            }
            AndroidCoreProcessObservation::Running { .. } => {
                return Err(Failure::new(
                    ErrorCode::NotReady,
                    "Android Core child is still running; owner recovery will not release its proof",
                    true,
                ));
            }
            AndroidCoreProcessObservation::Unavailable { reason } => {
                return Err(Failure::new(
                    ErrorCode::NotReady,
                    format!("Android Core child identity is unavailable: {reason:?}"),
                    true,
                ));
            }
        };

        self.vpn.stop().await?;
        let evidence = match &candidate {
            RecoveryCandidate::NeverStarted { terminal } => {
                ActiveProfileOwnerRecoveryEvidence::NeverStarted {
                    terminal: terminal.clone(),
                }
            }
            RecoveryCandidate::ManagedChildExited { context, terminal } => {
                ActiveProfileOwnerRecoveryEvidence::ManagedChildExited {
                    context: context.clone(),
                    terminal: terminal.clone(),
                }
            }
        };
        let owner_outcome = self
            .owner_store
            .recover_bound_owner_after_terminal(evidence)
            .await
            .map_err(Failure::from)?;

        match (candidate, owner_outcome) {
            (
                RecoveryCandidate::NeverStarted { .. },
                ActiveProfileOwnerRecoveryOutcome::NoRecoveryRequired,
            ) => Ok(AndroidCoreOwnerRecoveryOutcome::NoPreviousOwner),
            (
                RecoveryCandidate::NeverStarted { .. },
                ActiveProfileOwnerRecoveryOutcome::Recovered(selection),
            ) => Ok(AndroidCoreOwnerRecoveryOutcome::Recovered(selection)),
            (
                RecoveryCandidate::ManagedChildExited { context, .. },
                ActiveProfileOwnerRecoveryOutcome::Recovered(selection),
            ) => match self.process.consume_exited(&context).await {
                Ok(AndroidCoreProcessRecoveryReceipt::Consumed)
                | Ok(AndroidCoreProcessRecoveryReceipt::AlreadyAbsent) => {
                    Ok(AndroidCoreOwnerRecoveryOutcome::Recovered(selection))
                }
                Ok(receipt) => Err(Failure::new(
                    ErrorCode::NotReady,
                    format!("Android Core child exit record was not consumed: {receipt:?}"),
                    true,
                )),
                Err(error) => Err(Failure::from(error)),
            },
            (
                RecoveryCandidate::ManagedChildExited { .. },
                ActiveProfileOwnerRecoveryOutcome::NoRecoveryRequired,
            ) => Err(Failure::new(
                ErrorCode::NotReady,
                "Android exited Core child has no matching persisted owner proof",
                true,
            )),
        }
    }
}

fn stopped_snapshot_for_context(
    context: &CoreProcessStartContext,
    expected_identity: &CoreIdentity,
) -> Result<CoreLifecycleSnapshot, Failure> {
    let (identity, generation, session_token) = match context {
        CoreProcessStartContext::Managed {
            identity,
            generation,
            session_token,
        } => (identity, *generation, *session_token),
        CoreProcessStartContext::Standalone { .. } => {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "Android exited child cannot carry a standalone Core context",
                true,
            ));
        }
    };
    if !identity.same_instance(expected_identity)
        || generation.value() == 0
        || !session_token.is_valid()
    {
        return Err(Failure::new(
            ErrorCode::NotReady,
            "Android exited child context does not match the configured Core owner",
            true,
        ));
    }
    Ok(CoreLifecycleSnapshot {
        identity: Some(identity.clone()),
        lifecycle: CoreLifecycle::Stopped,
        generation: generation.value(),
        session_token: None,
        revision: 0,
        failure: None,
    })
}
