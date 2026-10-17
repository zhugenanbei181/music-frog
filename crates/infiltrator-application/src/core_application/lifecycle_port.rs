impl CoreApplication {
    pub(super) async fn start_locked(&self) -> Result<(), Failure> {
        if !matches!(
            self.current_state(),
            CoreState::Idle { .. } | CoreState::Failed { .. }
        ) {
            return Err(runtime_projection::invalid_state_failure("start"));
        }

        self.clear_active_core_version();

        // A manual start begins a fresh watchdog window. The recovery path
        // marks itself `Restarting` before calling this method, so its retry
        // counters remain intact until the new session is proven ready.
        self.reset_watchdog_for_start();

        let session_token = self.allocate_session_token();
        self.apply_domain_event(core_state::CoreEvent::StartRequested { session_token });
        let Some(identity) = self.inner.process.core_identity() else {
            let failure = Failure::new(
                ErrorCode::InvalidState,
                "Core process launch requires a stable Core identity",
                false,
            );
            self.apply_domain_event(core_state::CoreEvent::StartFailed {
                session_token,
                error: failure.message.clone(),
            });
            return Err(failure);
        };
        let start_context = CoreProcessStartContext::Managed {
            identity,
            generation: CoreGeneration::new(self.generation()),
            session_token,
        };
        let admission = match self.inner.process.prepare_start(&start_context).await {
            Ok(admission) => admission,
            Err(error) => {
                let failure = Failure::from(error);
                self.apply_domain_event(core_state::CoreEvent::StartFailed {
                    session_token,
                    error: failure.message.clone(),
                });
                return Err(failure);
            }
        };
        let owner_binding = {
            let state = self
                .inner
                .active_profile_owner_binding
                .lock()
                .expect("active profile owner binding lock");
            match &*state {
                ActiveProfileOwnerBindingState::Bound(binding) => Some(Arc::clone(binding)),
                ActiveProfileOwnerBindingState::Unbound
                | ActiveProfileOwnerBindingState::Releasing
                | ActiveProfileOwnerBindingState::Released => None,
            }
        };
        if let Some(binding) = owner_binding {
            if let Err(error) = binding.record_start_context(&start_context).await {
                let failure = Failure::from(error);
                self.apply_domain_event(core_state::CoreEvent::StartFailed {
                    session_token,
                    error: failure.message.clone(),
                });
                return Err(failure);
            }
        }
        if admission == CoreProcessStartAdmission::LaunchNewProcess {
            if let Err(error) = self.inner.process.start(start_context).await {
                let message = error.to_string();
                self.apply_domain_event(core_state::CoreEvent::StartFailed {
                    session_token,
                    error: message,
                });
                return Err(Failure::from(error));
            }
        }
        match self
            .wait_for_readiness(session_token, self.inner.readiness_policy.timeout)
            .await
        {
            Ok(endpoint) => {
                self.refresh_active_core_version().await;
                self.apply_domain_event(core_state::CoreEvent::ReadinessSuccess {
                    session_token,
                    endpoint,
                });
                Ok(())
            }
            Err(error) => {
                let failure = self.stop_after_readiness_failure(error).await;
                self.apply_domain_event(core_state::CoreEvent::StartFailed {
                    session_token,
                    error: failure.message.clone(),
                });
                Err(failure)
            }
        }
    }
    pub(super) async fn stop_locked(&self) -> Result<(), Failure> {
        if matches!(self.current_state(), CoreState::Idle { .. }) {
            return self.inner.process.stop().await.map_err(Failure::from);
        }

        let Some(session_token) = self.session_token() else {
            return Err(runtime_projection::invalid_state_failure("stop"));
        };
        self.apply_domain_event(core_state::CoreEvent::StopRequested { session_token });
        if let Err(error) = self.inner.process.stop().await {
            let message = error.to_string();
            self.apply_domain_event(core_state::CoreEvent::StopFailed {
                session_token,
                error: message,
            });
            return Err(Failure::from(error));
        }

        self.apply_domain_event(core_state::CoreEvent::StopCompleted { session_token });
        self.reset_watchdog_after_stop();
        Ok(())
    }
    pub(super) async fn restart_locked(&self) -> Result<(), Failure> {
        self.stop_locked().await?;
        self.start_locked().await
    }

    pub(super) async fn stop_after_readiness_failure(&self, error: PortError) -> Failure {
        let mut failure = Failure::from(error);
        if let Err(cleanup_error) = self.inner.process.stop().await {
            failure.message.push_str(&format!(
                "; stopping the core after readiness failure also failed: {cleanup_error}"
            ));
        }
        failure
    }
    pub(super) async fn wait_for_readiness(
        &self,
        session_token: SessionToken,
        timeout: time::Duration,
    ) -> Result<String, PortError> {
        let deadline = time::Instant::now() + timeout;
        loop {
            if self.session_token() != Some(session_token) {
                return Err(PortError::Failed("stale core session token".to_string()));
            }
            match self.inner.process.status().await {
                Ok(CoreLifecycle::Starting)
                | Ok(CoreLifecycle::Ready)
                | Ok(CoreLifecycle::Running) => {}
                Ok(_) => {
                    return Err(PortError::Failed(
                        "core process exited before readiness".to_string(),
                    ));
                }
                Err(error) => return Err(error),
            }

            let probe_error = match self.inner.readiness.probe().await {
                Ok(endpoint) => return Ok(endpoint),
                Err(error) => error,
            };

            if time::Instant::now() >= deadline {
                return Err(probe_error);
            }
            self.inner
                .runtime
                .sleep(self.inner.readiness_policy.poll_interval)
                .await;
        }
    }
}

use super::*;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceWriteAuthorization;
use infiltrator_contract::snapshot::CoreLifecycleSnapshot;
use infiltrator_ports::core_lifecycle::{CoreLifecyclePort, CoreOwnerTerminationPort};

#[async_trait::async_trait]
impl CoreLifecyclePort for CoreApplication {
    fn core_identity(&self) -> Option<CoreIdentity> {
        self.current_core_identity()
    }

    fn lifecycle(&self) -> CoreLifecycle {
        self.snapshot().lifecycle
    }

    fn generation(&self) -> u64 {
        CoreApplication::generation(self)
    }

    fn session_token(&self) -> Option<SessionToken> {
        CoreApplication::session_token(self)
    }

    fn lifecycle_snapshot(&self) -> CoreLifecycleSnapshot {
        self.snapshot().lifecycle_snapshot()
    }

    async fn start(&self) -> Result<u64, PortError> {
        match self
            .execute_lifecycle_with_id(self.allocate_request_id(), CommandIntent::StartCore)
            .await
        {
            CommandResult::Completed { .. } => Ok(self.generation()),
            CommandResult::Rejected { failure, .. } => Err(PortError::Failed(failure.message)),
            CommandResult::Produced { .. } => Err(PortError::Failed(
                "Unexpected typed result for a lifecycle/control command".into(),
            )),
            CommandResult::Accepted { .. } => Err(PortError::Failed(
                "application start unexpectedly returned Accepted".to_string(),
            )),
        }
    }

    async fn stop(&self) -> Result<(), PortError> {
        match self
            .execute_lifecycle_with_id(self.allocate_request_id(), CommandIntent::StopCore)
            .await
        {
            CommandResult::Completed { .. } => Ok(()),
            CommandResult::Rejected { failure, .. } => Err(PortError::Failed(failure.message)),
            CommandResult::Produced { .. } => Err(PortError::Failed(
                "Unexpected typed result for a lifecycle/control command".into(),
            )),
            CommandResult::Accepted { .. } => Err(PortError::Failed(
                "application stop unexpectedly returned Accepted".to_string(),
            )),
        }
    }

    async fn restart(&self) -> Result<u64, PortError> {
        match self
            .execute_lifecycle_with_id(self.allocate_request_id(), CommandIntent::RestartCore)
            .await
        {
            CommandResult::Completed { .. } => Ok(self.generation()),
            CommandResult::Rejected { failure, .. } => Err(PortError::Failed(failure.message)),
            CommandResult::Produced { .. } => Err(PortError::Failed(
                "Unexpected typed result for a lifecycle/control command".into(),
            )),
            CommandResult::Accepted { .. } => Err(PortError::Failed(
                "application restart unexpectedly returned Accepted".to_string(),
            )),
        }
    }

    fn begin_reload(&self) -> Result<SessionToken, PortError> {
        let CoreState::Running { session_token, .. } = self.current_state() else {
            return Err(PortError::Failed(
                "core is not running for hot reload".to_string(),
            ));
        };
        self.apply_domain_event(core_state::CoreEvent::ReloadRequested { session_token });
        Ok(session_token)
    }

    fn complete_reload(&self, session_token: SessionToken) -> Result<(), PortError> {
        self.check_session(session_token)
            .map_err(|failure| PortError::Failed(failure.message))?;
        self.apply_domain_event(core_state::CoreEvent::ReloadSuccess { session_token });
        Ok(())
    }

    fn fail_reload(&self, session_token: SessionToken, error: String) -> Result<(), PortError> {
        self.check_session(session_token)
            .map_err(|failure| PortError::Failed(failure.message))?;
        self.apply_domain_event(core_state::CoreEvent::ReloadFailed {
            session_token,
            error,
        });
        Ok(())
    }

    async fn wait_for_ready(
        &self,
        generation: u64,
        timeout: time::Duration,
    ) -> Result<(), PortError> {
        if self.generation() != generation {
            return Err(PortError::Failed(
                "stale application generation".to_string(),
            ));
        }
        let Some(session_token) = self.session_token() else {
            return Err(PortError::Failed("core session is not active".to_string()));
        };
        self.wait_for_readiness(session_token, timeout)
            .await
            .map(|_| ())
    }

    async fn wait_for_ready_session(
        &self,
        generation: u64,
        session_token: SessionToken,
        timeout: time::Duration,
    ) -> Result<(), PortError> {
        if self.generation() != generation {
            return Err(PortError::Failed(
                "stale application generation".to_string(),
            ));
        }
        self.check_session(session_token)
            .map_err(|failure| PortError::Failed(failure.message))?;
        self.wait_for_readiness(session_token, timeout)
            .await
            .map(|_| ())
    }
}

#[async_trait::async_trait]
impl CoreOwnerTerminationPort for CoreApplication {
    fn active_profile_reference_write_authorization(
        &self,
    ) -> Result<ActiveProfileReferenceWriteAuthorization, PortError> {
        CoreApplication::active_profile_reference_write_authorization(self)
            .map_err(PortError::Rejected)
    }

    async fn terminate_owner(&self) -> Result<CoreLifecycleSnapshot, PortError> {
        self.close().await.map_err(PortError::Rejected)?;
        Ok(self.snapshot().lifecycle_snapshot())
    }
}
