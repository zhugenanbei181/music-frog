//! Neutral Core process supervision and readiness ports.
//!
//! The ports never expose a process handle, an HTTP client or a timer to the
//! application boundary: they answer in terms of the neutral lifecycle
//! vocabulary and the typed [`PortError`] failure.

use crate::error::PortError;
use async_trait::async_trait;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::identity::{CoreIdentity, CoreVersion};
use lurker_core_contract::lifecycle::CoreLifecycle;

/// Admission observed before a managed launch is persisted and dispatched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreProcessStartAdmission {
    /// No owned process exists; the host may start a new one.
    LaunchNewProcess,
    /// The exact owned process is still running and may be reused.
    ReuseExactOwnedProcess,
}

/// Controls an engine execution host without exposing process handles.
#[async_trait]
pub trait CoreProcess: Send + Sync {
    /// Stable engine/configured-instance identity. Missing identity disables
    /// scoped core commands instead of guessing which backend owns the process.
    fn core_identity(&self) -> Option<CoreIdentity> {
        None
    }

    /// Observe the host process slot before the application persists its
    /// managed session proof. Android overrides this to refuse stale or
    /// ambiguous children before a new session can replace their proof.
    async fn prepare_start(
        &self,
        _context: &CoreProcessStartContext,
    ) -> Result<CoreProcessStartAdmission, PortError> {
        Ok(CoreProcessStartAdmission::LaunchNewProcess)
    }

    /// Starts the owned core process for the supplied ownership context.
    ///
    /// The host must honour the context: a `Managed` context binds the new
    /// process to the given identity, generation and session token.
    async fn start(&self, context: CoreProcessStartContext) -> Result<(), PortError>;
    /// Stop the exact configured process instance and confirm its exit before
    /// returning success. A command acknowledgement or disappearance of a
    /// tracked PID record is not terminal evidence; identity mismatches and
    /// stop deadlines fail. With no owned or recorded instance, stop is an
    /// idempotent no-op.
    async fn stop(&self) -> Result<(), PortError>;
    /// Observes the lifecycle state of the owned process.
    async fn status(&self) -> Result<CoreLifecycle, PortError>;
    /// Returns the controller endpoint of the owned process, when it has one.
    fn controller_endpoint(&self) -> Option<String>;

    /// Returns the process id of the owned process, when the host can read it.
    async fn pid(&self) -> Option<u32> {
        None
    }
}

/// Proves that the controller is ready without exposing an HTTP client or
/// polling/timer implementation to the application boundary.
#[async_trait]
pub trait CoreReadiness: Send + Sync {
    /// Probes the controller and returns the readiness evidence it observed.
    ///
    /// A successful probe proves the controller answers; it does not prove
    /// that the engine applied a specific configuration.
    async fn probe(&self) -> Result<String, PortError>;

    /// Authoritative version reported by the ready running engine, when the
    /// host can read it. Absence leaves version-scoped operations unavailable.
    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use lurker_core_contract::core_process::CoreProcessStartContext;
    use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, CoreInstanceId, EngineId};
    use lurker_core_contract::session::SessionToken;

    #[test]
    fn managed_start_context_uses_the_android_wire_tag_and_exact_fences() {
        let context = CoreProcessStartContext::managed(
            CoreIdentity::new(
                EngineId::new("mihomo").expect("valid engine"),
                CoreInstanceId::new("android-active").expect("valid instance"),
                None,
            ),
            CoreGeneration::new(7),
            SessionToken::new(42),
        );

        let value = serde_json::to_value(context).expect("serialize managed start context");
        assert_eq!(
            value,
            serde_json::json!({
                "Managed": {
                    "identity": {
                        "engine": "mihomo",
                        "instance": "android-active",
                        "version": null
                    },
                    "generation": 7,
                    "session_token": 42
                }
            }),
        );
    }

    #[test]
    fn standalone_start_context_remains_an_explicit_separate_variant() {
        let context = CoreProcessStartContext::standalone(CoreIdentity::new(
            EngineId::new("mihomo").expect("valid engine"),
            CoreInstanceId::new("desktop-active").expect("valid instance"),
            None,
        ));

        let value = serde_json::to_value(context).expect("serialize standalone start context");
        assert_eq!(
            value,
            serde_json::json!({
                "Standalone": {
                    "identity": {
                        "engine": "mihomo",
                        "instance": "desktop-active",
                        "version": null
                    }
                }
            }),
        );
    }
}
