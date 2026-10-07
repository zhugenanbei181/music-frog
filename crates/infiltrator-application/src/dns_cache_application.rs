//! DNS cache maintenance use-case.
//!
//! One flush request covers both targets:
//! * the running core's Fake-IP mapping table through the controller gateway;
//! * the operating system resolver cache through the host
//!   [`SystemDnsCachePort`](infiltrator_ports::system_dns_cache::SystemDnsCachePort).
//!
//! The report keeps per-target outcomes, so a host without a drivable OS
//! cache flush reports a typed unsupported instead of a fabricated success.
//! The command handler and the surface reader share one instance, which is how
//! the honest last-flush report reaches both surfaces.

use crate::dns_cache_actions::allocate_operation;
use futures_util::lock;
use infiltrator_contract::dns_cache::{
    DnsCacheFlushReport, DnsCacheOperation, DnsCacheOperationId, DnsCacheSnapshot, DnsFlushOutcome,
};
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::error::Failure;
use infiltrator_ports::error::PortError;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use infiltrator_ports::system_dns_cache::SystemDnsCachePort;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct DnsCacheApplication {
    runtime: Option<Arc<dyn RuntimeGateway>>,
    system_cache: Option<Arc<dyn SystemDnsCachePort>>,
    last: Arc<Mutex<DnsCacheSnapshot>>,
    serial: Arc<lock::Mutex<()>>,
}

impl DnsCacheApplication {
    pub fn new(
        runtime: Option<Arc<dyn RuntimeGateway>>,
        system_cache: Option<Arc<dyn SystemDnsCachePort>>,
    ) -> Self {
        Self {
            runtime,
            system_cache,
            last: Arc::new(Mutex::new(DnsCacheSnapshot::default())),
            serial: Arc::new(lock::Mutex::new(())),
        }
    }

    /// A host that can drive neither target still keeps an honest report.
    pub fn unconfigured() -> Self {
        Self::new(None, None)
    }

    /// The honest report of the last flush (or `NotRequested`).
    pub fn last_report(&self) -> DnsCacheFlushReport {
        self.snapshot().report
    }

    pub fn snapshot(&self) -> DnsCacheSnapshot {
        self.last
            .lock()
            .map(|last| last.clone())
            .unwrap_or_else(|_| DnsCacheSnapshot {
                operation: DnsCacheOperation::Failed,
                failure: Some(Failure::new(
                    ErrorCode::InvalidState,
                    "DNS cache state lock is poisoned",
                    false,
                )),
                ..DnsCacheSnapshot::default()
            })
    }

    /// Flush the Fake-IP table and, when the host provides one, the OS cache.
    ///
    /// The returned report is also stored as the last report. A partially
    /// unsupported host is not an error: the report says which target ran.
    pub async fn flush_all(&self) -> Result<DnsCacheFlushReport, Failure> {
        self.flush_with_id(allocate_operation()?).await
    }
    pub async fn flush_with_id(
        &self,
        operation: DnsCacheOperationId,
    ) -> Result<DnsCacheFlushReport, Failure> {
        let _serial = self.serial.try_lock().ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "DNS cache flush is already running",
                true,
            )
        })?;
        {
            let mut state = self.last.lock().map_err(|_| {
                Failure::new(
                    ErrorCode::InvalidState,
                    "DNS cache state lock is poisoned",
                    false,
                )
            })?;
            if operation.0 == 0
                || state
                    .operation_id
                    .is_some_and(|previous| previous.0 >= operation.0)
            {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "DNS cache operation identity is stale",
                    false,
                ));
            }
            state.revision.checked_add(2).ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidState,
                    "DNS cache revision exhausted",
                    false,
                )
            })?;
            state.revision = state.revision.checked_add(1).ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidState,
                    "DNS cache revision exhausted",
                    false,
                )
            })?;
            state.operation = DnsCacheOperation::Running;
            state.operation_id = Some(operation);
            state.failure = None;
        }
        let mut run = FlushRun {
            state: self.last.clone(),
            operation,
            committed: false,
        };
        let fake_ip = match self.runtime.as_ref() {
            Some(runtime) => outcome_from_result(runtime.flush_fakeip_cache().await),
            None => DnsFlushOutcome::Unsupported {
                reason: "no running core controller is attached".to_owned(),
            },
        };
        let os_cache = match self.system_cache.as_ref() {
            Some(port) => match port.flush_system_cache().await {
                Ok(true) => DnsFlushOutcome::Flushed,
                Ok(false) => DnsFlushOutcome::Unsupported {
                    reason: "host reported no drivable OS DNS cache flush".to_owned(),
                },
                Err(error) => outcome_from_result::<()>(Err(error)),
            },
            None => DnsFlushOutcome::Unsupported {
                reason: "host did not provide a system DNS cache adapter".to_owned(),
            },
        };
        let report = DnsCacheFlushReport { fake_ip, os_cache };
        let failure = [&report.fake_ip, &report.os_cache]
            .into_iter()
            .find_map(|outcome| match outcome {
                DnsFlushOutcome::Failed { failure } => Some(failure.clone()),
                _ => None,
            });
        let mut state = self.last.lock().map_err(|_| {
            Failure::new(
                ErrorCode::InvalidState,
                "DNS cache state lock is poisoned",
                false,
            )
        })?;
        state.revision = state.revision.checked_add(1).ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidState,
                "DNS cache revision exhausted",
                false,
            )
        })?;
        state.operation = report.operation();
        state.report = report.clone();
        state.report_id = Some(operation);
        state.failure = failure.clone();
        run.committed = true;
        if let Some(failure) = failure {
            Err(failure)
        } else {
            Ok(report)
        }
    }
}

struct FlushRun {
    state: Arc<Mutex<DnsCacheSnapshot>>,
    operation: DnsCacheOperationId,
    committed: bool,
}
impl Drop for FlushRun {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        if let Ok(mut state) = self.state.lock()
            && state.operation_id == Some(self.operation)
            && state.operation == DnsCacheOperation::Running
        {
            state.operation = DnsCacheOperation::Failed;
            state.failure = Some(Failure::new(
                ErrorCode::Canceled,
                "DNS cache operation was canceled before its final report",
                true,
            ));
            if let Some(revision) = state.revision.checked_add(1) {
                state.revision = revision;
            }
        }
    }
}

fn outcome_from_result<T>(result: Result<T, PortError>) -> DnsFlushOutcome {
    match result {
        Ok(_) => DnsFlushOutcome::Flushed,
        Err(PortError::Unsupported { reason, .. }) => DnsFlushOutcome::Unsupported { reason },
        Err(error) => DnsFlushOutcome::Failed {
            failure: Failure::from(error),
        },
    }
}

#[cfg(test)]
#[path = "dns_cache_application_test.rs"]
mod tests;

#[cfg(test)]
#[path = "dns_cache_behavior_test.rs"]
mod behavior_tests;
