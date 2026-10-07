//! Doctor and bootstrap use-cases over a host-provided port.

use crate::doctor_projection::project_report;
use futures_util::lock;
use infiltrator_contract::doctor::{
    BootstrapReport, DoctorCheckMeta, DoctorFixReport, DoctorReport,
};
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::error::Failure;
use infiltrator_contract::surface_snapshot::{DoctorPageSnapshot, PageData, PageStatus};
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

#[derive(Default)]
struct DoctorObservation {
    report: Option<DoctorReport>,
    failure: Option<Failure>,
    pending: bool,
}

struct PendingObservation(Arc<Mutex<DoctorObservation>>);
impl Drop for PendingObservation {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.lock()
            && state.pending
        {
            state.pending = false;
            state.failure = Some(Failure::new(
                ErrorCode::Canceled,
                "diagnostic operation was canceled",
                true,
            ));
        }
    }
}

impl DoctorApplication {
    pub fn new(port: Arc<dyn DoctorPort>) -> Self {
        Self {
            port,
            observation: Arc::new(Mutex::new(DoctorObservation::default())),
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

    pub fn page(&self) -> PageData<DoctorPageSnapshot> {
        let metadata = self.port.list_checks();
        let observation = match self.observation.lock() {
            Ok(observation) => observation,
            Err(_) => return PageData::failed(Self::poisoned()),
        };
        let data = project_report(observation.report.as_ref(), &metadata);
        let status = if observation.pending {
            PageStatus::Loading
        } else if let Some(failure) = &observation.failure {
            PageStatus::Failed {
                failure: failure.clone(),
            }
        } else if observation.report.is_some() {
            PageStatus::Ready
        } else {
            PageStatus::Empty
        };
        PageData {
            status,
            data: Some(data),
        }
    }
    fn begin(&self) -> Result<PendingObservation, Failure> {
        let mut state = self.observation.lock().map_err(|_| Self::poisoned())?;
        state.pending = true;
        state.failure = None;
        Ok(PendingObservation(self.observation.clone()))
    }
    fn finish(&self, result: &Result<DoctorReport, Failure>) -> Result<(), Failure> {
        let mut state = self.observation.lock().map_err(|_| Self::poisoned())?;
        state.pending = false;
        match result {
            Ok(report) => {
                state.report = Some(report.clone());
                state.failure = None;
            }
            Err(failure) => state.failure = Some(failure.clone()),
        }
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
