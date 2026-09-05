//! Adapter from the concrete doctor implementation to the application port.

use infiltrator_ports::doctor::DoctorPort;
use infiltrator_ports::error::PortError;
use std::path::PathBuf;

pub struct MihomoDoctor {
    environment: crate::doctor::DoctorEnv,
}

impl MihomoDoctor {
    pub fn detect() -> anyhow::Result<Self> {
        Ok(Self {
            environment: crate::doctor::DoctorEnv::detect()?,
        })
    }

    pub fn with_home(home: PathBuf) -> Self {
        Self {
            environment: crate::doctor::DoctorEnv::with_home(home),
        }
    }
}

#[async_trait::async_trait]
impl DoctorPort for MihomoDoctor {
    async fn run(&self, filter: Option<String>) -> Result<infiltrator_contract::doctor::DoctorReport, PortError> {
        Ok(convert_report(
            crate::doctor::run_with(&self.environment, filter.as_deref()).await,
        ))
    }

    async fn fix(&self, filter: Option<String>) -> Result<infiltrator_contract::doctor::DoctorFixReport, PortError> {
        crate::doctor::fix_with(&self.environment, filter.as_deref())
            .await
            .map(convert_fix_report)
            .map_err(adapter_error)
    }

    fn list_checks(&self) -> Vec<infiltrator_contract::doctor::DoctorCheckMeta> {
        crate::doctor::list_checks()
            .iter()
            .map(|meta| infiltrator_contract::doctor::DoctorCheckMeta {
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

    fn explain(&self, check_id: &str) -> Result<infiltrator_contract::doctor::DoctorCheckMeta, PortError> {
        let meta = crate::doctor::explain_check(check_id).map_err(adapter_error)?;
        Ok(infiltrator_contract::doctor::DoctorCheckMeta {
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

    async fn bootstrap(&self) -> Result<infiltrator_contract::doctor::BootstrapReport, PortError> {
        crate::bootstrap::ensure_bootstrap_at(self.environment.home())
            .await
            .map(convert_bootstrap_report)
            .map_err(adapter_error)
    }
}

fn adapter_error(error: impl std::fmt::Display) -> PortError {
    PortError::Failed(error.to_string())
}

fn convert_report(report: crate::doctor::DoctorReport) -> infiltrator_contract::doctor::DoctorReport {
    infiltrator_contract::doctor::DoctorReport {
        started_at: report.started_at,
        finished_at: report.finished_at,
        checks: report
            .checks
            .into_iter()
            .map(|check| infiltrator_contract::doctor::DoctorCheckResult {
                id: check.id,
                category: check.category,
                status: match check.status {
                    crate::doctor::DoctorStatus::Pass => infiltrator_contract::doctor::DoctorStatus::Pass,
                    crate::doctor::DoctorStatus::Warn => infiltrator_contract::doctor::DoctorStatus::Warn,
                    crate::doctor::DoctorStatus::Fail => infiltrator_contract::doctor::DoctorStatus::Fail,
                    crate::doctor::DoctorStatus::Skip => infiltrator_contract::doctor::DoctorStatus::Skip,
                },
                summary: check.summary,
                detail: check.detail,
                hint: check.hint,
            })
            .collect(),
    }
}

fn convert_fix_report(report: crate::doctor::DoctorFixReport) -> infiltrator_contract::doctor::DoctorFixReport {
    infiltrator_contract::doctor::DoctorFixReport {
        actions: report
            .actions
            .into_iter()
            .map(|action| infiltrator_contract::doctor::DoctorFixAction {
                id: action.id,
                summary: action.summary,
            })
            .collect(),
    }
}

fn convert_bootstrap_report(
    report: crate::bootstrap::BootstrapReport,
) -> infiltrator_contract::doctor::BootstrapReport {
    infiltrator_contract::doctor::BootstrapReport {
        steps: report
            .steps
            .into_iter()
            .map(|step| infiltrator_contract::doctor::BootstrapStep {
                id: step.id.to_owned(),
                executed: step.executed,
                detail: step.detail,
            })
            .collect(),
    }
}
