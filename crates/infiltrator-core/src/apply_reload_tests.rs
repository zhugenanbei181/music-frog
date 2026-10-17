//! Tests for the typed lifecycle around native live reload.

use super::*;
use async_trait::async_trait;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::session::SessionToken;
use lurker_core_ports::error::PortError;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReloadObservation {
    Idle,
    Requested,
    Completed,
    Failed,
}

struct MockLifecycle {
    generation: AtomicU64,
    reload_observation: Mutex<ReloadObservation>,
}

impl MockLifecycle {
    fn new(generation: u64) -> Self {
        Self {
            generation: AtomicU64::new(generation),
            reload_observation: Mutex::new(ReloadObservation::Idle),
        }
    }

    fn transition_reload(&self, next: ReloadObservation) -> Result<(), PortError> {
        let mut observed = self
            .reload_observation
            .lock()
            .expect("reload observation lock");
        match (*observed, next) {
            (ReloadObservation::Idle, ReloadObservation::Requested)
            | (ReloadObservation::Requested, ReloadObservation::Completed)
            | (ReloadObservation::Requested, ReloadObservation::Failed) => {
                *observed = next;
                Ok(())
            }
            (current, requested) => Err(PortError::Failed(format!(
                "illegal reload transition: {current:?} -> {requested:?}"
            ))),
        }
    }

    fn reload_observation(&self) -> ReloadObservation {
        *self
            .reload_observation
            .lock()
            .expect("reload observation lock")
    }
}

#[async_trait]
impl CoreLifecyclePort for MockLifecycle {
    async fn start(&self) -> Result<u64, PortError> {
        Ok(self.generation.load(Ordering::SeqCst))
    }

    async fn stop(&self) -> Result<(), PortError> {
        Ok(())
    }

    async fn restart(&self) -> Result<u64, PortError> {
        Ok(self.generation.fetch_add(1, Ordering::SeqCst) + 1)
    }

    fn lifecycle(&self) -> CoreLifecycle {
        CoreLifecycle::Running
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    fn session_token(&self) -> Option<SessionToken> {
        Some(SessionToken::new(
            self.generation.load(Ordering::SeqCst) as u128
        ))
    }

    fn begin_reload(&self) -> Result<SessionToken, PortError> {
        self.transition_reload(ReloadObservation::Requested)?;
        Ok(SessionToken::new(
            self.generation.load(Ordering::SeqCst) as u128
        ))
    }

    fn complete_reload(&self, _session_token: SessionToken) -> Result<(), PortError> {
        self.transition_reload(ReloadObservation::Completed)
    }

    fn fail_reload(&self, _session_token: SessionToken, _error: String) -> Result<(), PortError> {
        self.transition_reload(ReloadObservation::Failed)
    }

    async fn wait_for_ready(&self, _generation: u64, _timeout: Duration) -> Result<(), PortError> {
        Ok(())
    }

    async fn wait_for_ready_session(
        &self,
        _generation: u64,
        _session_token: SessionToken,
        _timeout: Duration,
    ) -> Result<(), PortError> {
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum LiveReloadResult {
    Accepted,
    Rejected,
}

struct ReloadProbe {
    result: LiveReloadResult,
    calls: AtomicU64,
}

impl ReloadProbe {
    fn new(result: LiveReloadResult) -> Self {
        Self {
            result,
            calls: AtomicU64::new(0),
        }
    }
}

#[async_trait]
impl PreparedConfigReload for ReloadProbe {
    async fn reload(&self, _path: &Path) -> Result<(), String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.result {
            LiveReloadResult::Accepted => Ok(()),
            LiveReloadResult::Rejected => Err("kernel rejected live switch".to_owned()),
        }
    }
}

#[async_trait]
impl ConfigReloader for ReloadProbe {
    async fn prepare(&self) -> Result<Box<dyn PreparedConfigReload + '_>, String> {
        Ok(Box::new(self))
    }
}

#[tokio::test]
async fn live_reload_success_completes_the_typed_session_transition() {
    let lifecycle = MockLifecycle::new(1);
    let reloader = ReloadProbe::new(LiveReloadResult::Accepted);
    let path = PathBuf::from("/tmp/valid_config.yaml");
    let params = ApplyParams::default();

    let outcome = reload_and_check(&lifecycle, &reloader, &path, &params)
        .await
        .expect("successful native reload");

    assert_eq!(outcome.method, ApplyMethod::HotReload);
    assert_eq!(reloader.calls.load(Ordering::SeqCst), 1);
    assert_eq!(lifecycle.reload_observation(), ReloadObservation::Completed);
}

#[tokio::test]
async fn native_reload_failure_marks_the_typed_session_for_recovery() {
    let lifecycle = MockLifecycle::new(1);
    let reloader = ReloadProbe::new(LiveReloadResult::Rejected);
    let path = PathBuf::from("/tmp/invalid_config.yaml");
    let params = ApplyParams::default();

    let error = reload_and_check(&lifecycle, &reloader, &path, &params)
        .await
        .expect_err("native reload rejection must be returned to the transaction owner");

    assert!(error.contains("kernel rejected live switch"));
    assert_eq!(reloader.calls.load(Ordering::SeqCst), 1);
    assert_eq!(lifecycle.reload_observation(), ReloadObservation::Failed);
}
