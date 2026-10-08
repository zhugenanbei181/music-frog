//! Tests for two-phase hot reload preflight transactions (CORE-040-01).

use super::*;
use async_trait::async_trait;
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::error::PortError;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

struct MockLifecycle {
    generation: AtomicU64,
    reload_requested: AtomicBool,
    reload_completed: AtomicBool,
    reload_failed: AtomicBool,
}

impl MockLifecycle {
    fn new(generation: u64) -> Self {
        Self {
            generation: AtomicU64::new(generation),
            reload_requested: AtomicBool::new(false),
            reload_completed: AtomicBool::new(false),
            reload_failed: AtomicBool::new(false),
        }
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
        self.reload_requested.store(true, Ordering::SeqCst);
        Ok(SessionToken::new(
            self.generation.load(Ordering::SeqCst) as u128
        ))
    }

    fn complete_reload(&self, _session_token: SessionToken) -> Result<(), PortError> {
        self.reload_completed.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn fail_reload(&self, _session_token: SessionToken, _error: String) -> Result<(), PortError> {
        self.reload_failed.store(true, Ordering::SeqCst);
        Ok(())
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

struct TwoPhaseReloader {
    preflight_should_fail: bool,
    reload_should_fail: bool,
    preflight_calls: AtomicU64,
    reload_calls: AtomicU64,
}

impl TwoPhaseReloader {
    fn passing() -> Self {
        Self {
            preflight_should_fail: false,
            reload_should_fail: false,
            preflight_calls: AtomicU64::new(0),
            reload_calls: AtomicU64::new(0),
        }
    }

    fn failing_preflight() -> Self {
        Self {
            preflight_should_fail: true,
            reload_should_fail: false,
            preflight_calls: AtomicU64::new(0),
            reload_calls: AtomicU64::new(0),
        }
    }

    fn failing_reload() -> Self {
        Self {
            preflight_should_fail: false,
            reload_should_fail: true,
            preflight_calls: AtomicU64::new(0),
            reload_calls: AtomicU64::new(0),
        }
    }
}

#[async_trait]
impl ConfigReloader for TwoPhaseReloader {
    async fn preflight_check(&self, _path: &Path) -> Result<(), String> {
        self.preflight_calls.fetch_add(1, Ordering::SeqCst);
        if self.preflight_should_fail {
            Err("syntax error in candidate profile".to_string())
        } else {
            Ok(())
        }
    }

    async fn reload(&self, _path: &Path) -> Result<(), String> {
        self.reload_calls.fetch_add(1, Ordering::SeqCst);
        if self.reload_should_fail {
            Err("kernel rejected live switch".to_string())
        } else {
            Ok(())
        }
    }
}

#[tokio::test]
async fn test_preflight_success_proceeds_to_live_switch() {
    let lifecycle = MockLifecycle::new(1);
    let reloader = TwoPhaseReloader::passing();
    let path = PathBuf::from("/tmp/valid_config.yaml");
    let params = ApplyParams::default();

    let outcome = reload_and_check(&lifecycle, &reloader, &path, &params)
        .await
        .expect("successful two-phase reload");

    assert_eq!(outcome.method, ApplyMethod::HotReload);
    assert_eq!(reloader.preflight_calls.load(Ordering::SeqCst), 1);
    assert_eq!(reloader.reload_calls.load(Ordering::SeqCst), 1);
    assert!(lifecycle.reload_requested.load(Ordering::SeqCst));
    assert!(lifecycle.reload_completed.load(Ordering::SeqCst));
    assert!(!lifecycle.reload_failed.load(Ordering::SeqCst));
}

#[tokio::test]
async fn test_preflight_failure_aborts_without_touching_live_session() {
    let lifecycle = MockLifecycle::new(1);
    let reloader = TwoPhaseReloader::failing_preflight();
    let path = PathBuf::from("/tmp/invalid_config.yaml");
    let params = ApplyParams::default();

    let err = reload_and_check(&lifecycle, &reloader, &path, &params)
        .await
        .expect_err("preflight must abort transaction");

    assert!(err.contains("config preflight check failed"));
    assert_eq!(reloader.preflight_calls.load(Ordering::SeqCst), 1);
    assert_eq!(reloader.reload_calls.load(Ordering::SeqCst), 0);
    assert!(!lifecycle.reload_requested.load(Ordering::SeqCst));
    assert!(!lifecycle.reload_completed.load(Ordering::SeqCst));
    assert!(!lifecycle.reload_failed.load(Ordering::SeqCst));
}

#[tokio::test]
async fn test_phase_two_failure_marks_reload_failed_for_rollback() {
    let lifecycle = MockLifecycle::new(1);
    let reloader = TwoPhaseReloader::failing_reload();
    let path = PathBuf::from("/tmp/config.yaml");
    let params = ApplyParams::default();

    let err = reload_and_check(&lifecycle, &reloader, &path, &params)
        .await
        .expect_err("reload must fail");

    assert!(err.contains("kernel rejected live switch"));
    assert_eq!(reloader.preflight_calls.load(Ordering::SeqCst), 1);
    assert_eq!(reloader.reload_calls.load(Ordering::SeqCst), 1);
    assert!(lifecycle.reload_requested.load(Ordering::SeqCst));
    assert!(lifecycle.reload_failed.load(Ordering::SeqCst));
    assert!(!lifecycle.reload_completed.load(Ordering::SeqCst));
}
