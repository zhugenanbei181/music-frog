//! Runtime-owned switch host.
//!
//! The coordinator owns the journal order; this host supplies the live facts:
//! the exact previous process, the candidate identity and the freshly started
//! candidate. It never changes the journal itself.

use super::process::{prepare_standalone, start_and_bind_owner, stop_process};
use super::{LurkerRuntimeState, RuntimeInner, lock_active};
use crate::switch::host::EngineSwitchHost;
use async_trait::async_trait;
use lurker_core_contract::engine_switch::{EngineSwitchStage, EngineSwitchTarget};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, EngineId};
use lurker_core_ports::engine_backend::{
    ENGINE_BACKEND_CAPABILITY, EngineBackendPorts, EngineBackendProviderAvailability,
};
use lurker_core_ports::error::PortError;
use std::sync::Arc;

/// One switch attempt, bound to the runtime that owns the live process.
pub(super) struct RuntimeSwitchHost {
    pub(super) inner: Arc<RuntimeInner>,
    pub(super) from: EngineId,
    pub(super) to: EngineId,
}

#[async_trait]
impl EngineSwitchHost<EngineBackendPorts, EngineId> for RuntimeSwitchHost {
    async fn validate_target(
        &self,
        target: &EngineSwitchTarget<EngineId>,
    ) -> Result<(), PortError> {
        let (engine_id, selection) = match target {
            EngineSwitchTarget::Selected {
                engine_id,
                selection,
            } => (engine_id, selection),
            EngineSwitchTarget::Unselected => {
                return Err(not_ready(
                    "an engine switch requires a selected target engine",
                ));
            }
        };
        if selection != engine_id {
            return Err(not_ready(
                "an engine switch selection must equal the target engine",
            ));
        }
        match self.inner.registry.availability(engine_id) {
            EngineBackendProviderAvailability::Registered => {}
            EngineBackendProviderAvailability::Unsupported { reason } => {
                return Err(PortError::unsupported(ENGINE_BACKEND_CAPABILITY, reason));
            }
        }
        match self.inner.state() {
            LurkerRuntimeState::Running { .. } => Ok(()),
            // `switch_engine` publishes the closing stage before the
            // coordinator validates the target; only this operation can own
            // the transient stage, so the target match is the exact fence.
            LurkerRuntimeState::Switching { to, .. } if to == *engine_id => Ok(()),
            _ => Err(not_ready(
                "an engine switch requires the runtime to be running",
            )),
        }
    }

    async fn prepare_candidate(
        &self,
        engine_id: &EngineId,
        selection: &EngineId,
        backend: &EngineBackendPorts,
    ) -> Result<CoreIdentity, PortError> {
        if selection != engine_id || backend.engine_id() != engine_id {
            return Err(not_ready(
                "the engine switch candidate does not match the requested engine",
            ));
        }
        let identity = backend.process().core_identity().ok_or_else(|| {
            PortError::Failed(format!(
                "{} candidate backend has no Core identity",
                engine_id.as_str()
            ))
        })?;
        if identity.engine != *engine_id {
            return Err(PortError::Failed(format!(
                "{} candidate backend reports identity for {}",
                engine_id.as_str(),
                identity.engine.as_str()
            )));
        }
        if let Some(expected_instance) = &self.inner.expected_instance
            && identity.instance != *expected_instance
        {
            return Err(not_ready(
                "the engine switch candidate instance differs from the configured instance",
            ));
        }
        Ok(identity)
    }

    async fn terminate_old_owner(
        &self,
        identity: &CoreIdentity,
        generation: CoreGeneration,
    ) -> Result<CoreGeneration, PortError> {
        let backend = match lock_active(&self.inner.active) {
            Ok(active) => active.backend.clone(),
            Err(cause) => return Err(cause),
        }
        .ok_or_else(|| {
            PortError::Failed("the engine switch has no live old owner backend".to_owned())
        })?;
        let live_identity = backend.process().core_identity().ok_or_else(|| {
            PortError::Failed("the live engine owner has no Core identity".to_owned())
        })?;
        if live_identity.engine != identity.engine || !live_identity.same_instance(identity) {
            return Err(not_ready(
                "the live Core owner differs from the durable switch owner",
            ));
        }
        stop_process(&backend)
            .await
            .map_err(|failure| failure.cause)?;
        if let Ok(mut active) = lock_active(&self.inner.active) {
            active.backend = None;
            active.scope = None;
            active.version = None;
        }
        self.inner.publish(LurkerRuntimeState::Switching {
            from: self.from.clone(),
            to: self.to.clone(),
            stage: EngineSwitchStage::OldStopped {
                previous: self.from.clone(),
                owner_identity: identity.clone(),
                owner_generation: generation,
                terminal_generation: generation,
            },
        });
        Ok(generation)
    }

    async fn apply_candidate(
        &self,
        backend: EngineBackendPorts,
        selection: &EngineId,
    ) -> Result<(CoreIdentity, CoreGeneration), PortError> {
        if backend.engine_id() != selection {
            return Err(PortError::Failed(format!(
                "the applied engine backend does not match {}",
                selection.as_str()
            )));
        }
        {
            let active = lock_active(&self.inner.active)?;
            if active.backend.is_some() {
                return Err(PortError::Failed(
                    "the previous engine owner is still bound while applying a candidate"
                        .to_owned(),
                ));
            }
        }
        let (context, identity) =
            prepare_standalone(&backend, self.inner.expected_instance.as_ref())
                .await
                .map_err(|failure| failure.cause)?;
        let (scope, version) = start_and_bind_owner(
            &backend,
            context,
            identity.clone(),
            self.inner.readiness_timeout,
        )
        .await
        .map_err(|failure| failure.cause)?;
        let mut active = lock_active(&self.inner.active)?;
        active.backend = Some(backend);
        active.scope = Some(scope);
        active.version = version;
        Ok((identity, CoreGeneration::new(1)))
    }

    async fn close_candidate(&self, _backend: EngineBackendPorts) -> Result<(), PortError> {
        Ok(())
    }
}

fn not_ready(message: &'static str) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::NotReady, message, false))
}
