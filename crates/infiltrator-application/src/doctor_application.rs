//! Doctor and bootstrap use-cases over a host-provided port.

use futures_util::lock;
use infiltrator_contract::doctor::{
    BootstrapReport, DoctorCheckMeta, DoctorFixReport, DoctorObservation, DoctorReport,
};
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::error::Failure;
use infiltrator_ports::doctor::DoctorPort;
use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct DoctorApplication {
    port: Arc<dyn DoctorPort>,
    observation: Arc<Mutex<DoctorObservation>>,
    operation: Arc<lock::Mutex<()>>,
}

impl fmt::Debug for DoctorApplication {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DoctorApplication")
            .finish_non_exhaustive()
    }
}

struct PendingObservation(Arc<Mutex<DoctorObservation>>);
impl Drop for PendingObservation {
    fn drop(&mut self) {
        match self.0.lock() {
            Ok(mut state) => match state.clone().cancel() {
                Ok(cancelled) => *state = cancelled,
                Err(_) => {}
            },
            Err(_) => {}
        }
    }
}

impl DoctorApplication {
    pub fn new(port: Arc<dyn DoctorPort>) -> Self {
        Self {
            port,
            observation: Arc::new(Mutex::new(DoctorObservation::Unobserved)),
            operation: Arc::new(lock::Mutex::new(())),
        }
    }

    pub async fn run(&self, filter: Option<String>) -> Result<DoctorReport, Failure> {
        let _operation = self.operation.lock().await;
        let _pending = self.begin()?;
        let result = self
            .port
            .run(filter)
            .await
            .map_err(Failure::from)
            .and_then(Self::validate_report);
        self.finish(&result)?;
        result
    }

    pub async fn fix(&self, filter: Option<String>) -> Result<DoctorFixReport, Failure> {
        let _operation = self.operation.lock().await;
        let _pending = self.begin()?;
        if let Some(id) = filter.as_deref() {
            let meta = match self.port.explain(id).map_err(Failure::from) {
                Ok(meta) => meta,
                Err(failure) => {
                    self.finish(&Err(failure.clone()))?;
                    return Err(failure);
                }
            };
            if meta.id != id || !meta.fixable {
                let failure = Failure::new(
                    ErrorCode::InvalidInput,
                    "diagnostic check is not repairable",
                    false,
                );
                self.finish(&Err(failure.clone()))?;
                return Err(failure);
            }
        }
        match self.port.fix(filter).await.map_err(Failure::from) {
            Ok(fixed) => {
                let result = self
                    .port
                    .run(None)
                    .await
                    .map_err(Failure::from)
                    .and_then(Self::validate_report);
                self.finish(&result)?;
                result.map(|_| fixed)
            }
            Err(failure) => {
                self.finish(&Err(failure.clone()))?;
                Err(failure)
            }
        }
    }

    pub fn list_checks(&self) -> Vec<DoctorCheckMeta> {
        self.port.list_checks()
    }

    pub fn explain(&self, check_id: &str) -> Result<DoctorCheckMeta, Failure> {
        self.port.explain(check_id).map_err(Failure::from)
    }

    pub async fn bootstrap(&self) -> Result<BootstrapReport, Failure> {
        let _operation = self.operation.lock().await;
        let _pending = self.begin()?;
        match self.port.bootstrap().await.map_err(Failure::from) {
            Ok(bootstrap) => {
                let result = self
                    .port
                    .run(None)
                    .await
                    .map_err(Failure::from)
                    .and_then(Self::validate_report);
                self.finish(&result)?;
                result.map(|_| bootstrap)
            }
            Err(failure) => {
                self.finish(&Err(failure.clone()))?;
                Err(failure)
            }
        }
    }

    pub fn observation(&self) -> DoctorObservation {
        match self.observation.lock() {
            Ok(observation) => observation.clone(),
            Err(_) => DoctorObservation::Failed(Self::poisoned()),
        }
    }
    fn begin(&self) -> Result<PendingObservation, Failure> {
        let mut state = self.observation.lock().map_err(|_| Self::poisoned())?;
        let next = state.begin()?;
        *state = next;
        Ok(PendingObservation(self.observation.clone()))
    }
    fn finish(&self, result: &Result<DoctorReport, Failure>) -> Result<(), Failure> {
        let mut state = self.observation.lock().map_err(|_| Self::poisoned())?;
        let next = state.clone().finish(result.clone())?;
        *state = next;
        Ok(())
    }
    fn validate_report(report: DoctorReport) -> Result<DoctorReport, Failure> {
        let mut ids = HashSet::new();
        if report.finished_at < report.started_at
            || report
                .checks
                .iter()
                .any(|check| check.id.trim().is_empty() || !ids.insert(check.id.as_str()))
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "diagnostic report has invalid timestamps or check identities",
                false,
            ));
        }
        Ok(report)
    }
    fn poisoned() -> Failure {
        Failure::new(
            ErrorCode::InvalidState,
            "diagnostic observation lock is poisoned",
            false,
        )
    }
}

#[cfg(test)]
#[path = "doctor_application_test.rs"]
mod tests;
