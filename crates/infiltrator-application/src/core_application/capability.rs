//! Engine-scoped lifecycle capability admission and active identity refresh.

use super::CoreApplication;
use super::execution::closed_failure;
use crate::command_capability::{CommandCapabilityRoute, classify_command};
use crate::core_capability_application::{
    CoreCapabilityApplication, CoreCapabilityGateFailure, CoreCapabilityRequirement,
};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::core_identity::CoreIdentity;
use infiltrator_contract::error::{ErrorCode, Failure};
use std::sync::atomic::Ordering;

impl CoreApplication {
    /// Clone the owner-installed operation capability resolver for read-only
    /// projections that must use the same gate as command execution.
    pub fn core_capability_application(&self) -> CoreCapabilityApplication {
        self.inner
            .core_capabilities
            .read()
            .expect("core capability resolver lock")
            .clone()
    }

    /// Install the same engine-scoped capability resolver used by the product
    /// command facade. A missing resolver denies lifecycle commands.
    pub fn install_core_capability_application(
        &self,
        application: CoreCapabilityApplication,
    ) -> Result<(), Failure> {
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(closed_failure());
        }
        if self.inner.active_command.load(Ordering::Acquire) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "core capability resolver cannot change during a command",
                false,
            ));
        }
        *self
            .inner
            .core_capabilities
            .write()
            .expect("core capability resolver lock") = application;
        Ok(())
    }

    pub(super) fn authorize_lifecycle_command(
        &self,
        intent: &CommandIntent,
    ) -> Result<(), Failure> {
        let requirement = match classify_command(intent) {
            CommandCapabilityRoute::LifecycleOwned(requirement) => requirement,
            _ => {
                return Err(Failure::unsupported(
                    "non-lifecycle command cannot use the lifecycle capability gate",
                ));
            }
        };
        self.authorize_lifecycle_requirement(requirement)
    }

    pub(super) fn authorize_lifecycle_admission(
        &self,
        intent: &CommandIntent,
    ) -> Result<(), Failure> {
        let requirement = match classify_command(intent) {
            CommandCapabilityRoute::LifecycleOwned(requirement) => requirement,
            CommandCapabilityRoute::LifecycleChild
                if matches!(intent, CommandIntent::StopProxy) =>
            {
                return Ok(());
            }
            _ => {
                return Err(Failure::unsupported(
                    "non-lifecycle command cannot use the lifecycle admission gate",
                ));
            }
        };
        self.authorize_lifecycle_requirement(requirement)
    }

    pub(super) fn authorize_lifecycle_requirement(
        &self,
        requirement: CoreCapabilityRequirement,
    ) -> Result<(), Failure> {
        let scope = self.snapshot().scope();
        self.inner
            .core_capabilities
            .read()
            .expect("core capability resolver lock")
            .authorize(scope.as_ref(), requirement)
            .map(|_| ())
            .map_err(CoreCapabilityGateFailure::into_failure)
    }

    pub(super) fn current_core_identity(&self) -> Option<CoreIdentity> {
        self.inner
            .core_identity
            .read()
            .expect("core identity lock")
            .clone()
    }

    pub(super) fn clear_active_core_version(&self) {
        if let Some(identity) = self
            .inner
            .core_identity
            .write()
            .expect("core identity lock")
            .as_mut()
        {
            identity.version = None;
        }
    }

    pub(super) async fn refresh_active_core_version(&self) {
        match self.inner.readiness.active_core_version().await {
            Ok(Some(version)) => {
                if let Some(identity) = self
                    .inner
                    .core_identity
                    .write()
                    .expect("core identity lock")
                    .as_mut()
                {
                    identity.version = Some(version);
                }
            }
            Ok(None) => {}
            Err(error) => log::warn!(
                target: "infiltrator-application",
                "ready core version is unavailable: {error}"
            ),
        }
    }
}
