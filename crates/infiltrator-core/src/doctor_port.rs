//! Adapter from the concrete doctor implementation to the application port.

use crate::bootstrap::ensure_bootstrap_at;
use crate::doctor::{DoctorEnv, explain_check, fix_with, list_checks, run_with};
use crate::{bootstrap, doctor};
use infiltrator_contract::doctor::{
    BootstrapReport, BootstrapStep, DoctorCheckMeta, DoctorCheckResult, DoctorFixAction,
    DoctorFixReport, DoctorReport, DoctorStatus,
};
use infiltrator_ports::doctor::DoctorPort;
use infiltrator_ports::error::PortError;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;

pub struct MihomoDoctor {
    environment: DoctorEnv,
}

impl MihomoDoctor {
    pub fn detect() -> anyhow::Result<Self> {
        Ok(Self {
            environment: DoctorEnv::detect()?,
        })
    }

    pub fn with_home(home: PathBuf) -> Self {
        Self {
            environment: DoctorEnv::with_home(home),
        }
    }
}

#[async_trait::async_trait]
impl DoctorPort for MihomoDoctor {
    async fn run(&self, filter: Option<String>) -> Result<DoctorReport, PortError> {
        Ok(convert_report(
            run_with(&self.environment, filter.as_deref()).await,
        ))
    }

    async fn fix(&self, filter: Option<String>) -> Result<DoctorFixReport, PortError> {
        fix_with(&self.environment, filter.as_deref())
            .await
            .map(convert_fix_report)
            .map_err(adapter_error)
    }

    fn list_checks(&self) -> Vec<DoctorCheckMeta> {
        list_checks()
            .iter()
            .map(|meta| DoctorCheckMeta {
                id: meta.id.to_owned(),
                category: meta.category.to_owned(),
                summary: meta.summary.to_owned(),
                why: meta.why.to_owned(),
                fail_means: meta.fail_means.to_owned(),
                hint: meta.hint.to_owned(),
                fixable: meta.fixable,
                default_enabled: meta.default_enabled,
            })
            .collect()
    }

    fn explain(&self, check_id: &str) -> Result<DoctorCheckMeta, PortError> {
        let meta = explain_check(check_id).map_err(adapter_error)?;
        Ok(DoctorCheckMeta {
            id: meta.id.to_owned(),
            category: meta.category.to_owned(),
            summary: meta.summary.to_owned(),
            why: meta.why.to_owned(),
            fail_means: meta.fail_means.to_owned(),
            hint: meta.hint.to_owned(),
            fixable: meta.fixable,
            default_enabled: meta.default_enabled,
        })
    }

    async fn bootstrap(&self) -> Result<BootstrapReport, PortError> {
        ensure_bootstrap_at(self.environment.home())
            .await
            .map(convert_bootstrap_report)
            .map_err(adapter_error)
    }
}

fn adapter_error(error: anyhow::Error) -> PortError {
    if let Some(failure) = error.downcast_ref::<PortError>() {
        return failure.clone();
    }
    let message = format!("{error:#}");
    match error
        .chain()
        .find_map(|cause| cause.downcast_ref::<Error>())
        .map(Error::kind)
    {
        Some(ErrorKind::PermissionDenied) => PortError::PermissionDenied(message),
        Some(ErrorKind::NotFound) => PortError::NotFound(message),
        Some(_) => PortError::Io(message),
        None => PortError::Failed(message),
    }
}

fn convert_report(report: doctor::DoctorReport) -> DoctorReport {
    DoctorReport {
        started_at: report.started_at,
        finished_at: report.finished_at,
        checks: report
            .checks
            .into_iter()
            .map(|check| DoctorCheckResult {
                id: check.id,
                category: check.category,
                status: match check.status {
                    doctor::DoctorStatus::Pass => DoctorStatus::Pass,
                    doctor::DoctorStatus::Warn => DoctorStatus::Warn,
                    doctor::DoctorStatus::Fail => DoctorStatus::Fail,
                    doctor::DoctorStatus::Skip => DoctorStatus::Skip,
                },
                summary: check.summary,
                detail: check.detail,
                hint: check.hint,
            })
            .collect(),
    }
}

fn convert_fix_report(report: doctor::DoctorFixReport) -> DoctorFixReport {
    DoctorFixReport {
        actions: report
            .actions
            .into_iter()
            .map(|action| DoctorFixAction {
                id: action.id,
                summary: action.summary,
            })
            .collect(),
    }
}

fn convert_bootstrap_report(report: bootstrap::BootstrapReport) -> BootstrapReport {
    BootstrapReport {
        steps: report
            .steps
            .into_iter()
            .map(|step| BootstrapStep {
                id: step.id.to_owned(),
                executed: step.executed,
                detail: step.detail,
            })
            .collect(),
    }
}

#[cfg(test)]
#[path = "doctor_port_test.rs"]
mod tests;
