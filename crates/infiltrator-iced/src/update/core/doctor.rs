//! Doctor 体检面板 handlers：经内嵌 admin server 的 loopback HTTP 调用
//! `/admin/api/doctor*` 与 `/admin/api/bootstrap`，报告写回 `diag.doctor`。
//!
//! 约束：demo 模式在 dispatcher 层拦截（update.rs），单测只驱动 `Result`
//! 消息、从不发起真实请求；请求失败只落在面板错误位，不弹全局错误条。

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::doctor::DoctorAction;
use infiltrator_contract::doctor::{BootstrapReport, DoctorFixReport, DoctorReport};
use infiltrator_contract::error::InfiltratorError;
use infiltrator_desktop::admin_client::AdminApiClient;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

impl AppState {
    /// 内嵌 admin server 的 API 基址：实际绑定地址优先（端口被占时会向上
    /// 漂移），服务未起时退回配置端口，让调用以连接失败的形式暴露。
    fn admin_api_base(&self) -> String {
        match self.shell.admin_server.url() {
            Some(url) => url.trim_end_matches('/').to_string(),
            None => format!("http://127.0.0.1:{}/admin", self.shell.admin_port),
        }
    }

    /// Polling 链上的体检域。Unmatched messages fall through to the next
    /// domain in the `update_core` chain.
    pub(super) fn update_core_doctor(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::RunDoctor => {
                if self.commands.is_some() {
                    return self.shared_doctor_command(DoctorAction::Diagnose);
                }
                if self.diag.doctor.is_running {
                    return Task::none();
                }
                self.diag.doctor.is_running = true;
                self.diag.doctor.error = None;
                let base = self.admin_api_base();
                Task::perform(
                    async move {
                        AdminApiClient::new(base)
                            .map_err(|e| InfiltratorError::Internal(e.to_string()))?
                            .get::<DoctorReport>("/api/doctor")
                            .await
                            .map_err(|e| InfiltratorError::Internal(e.to_string()))
                    },
                    Message::DoctorReportReady,
                )
            }
            Message::DoctorCommandFinished { token, result } => {
                let failure = result.as_ref().err().cloned();
                if self.diag.doctor.action.finish(token, result) {
                    self.diag.doctor.error = failure.map(|failure| failure.message);
                    if self.shell.demo
                        && let Some(doctor) = &self.diag.doctor.capture_observation
                    {
                        let mut snapshot =
                            self.surface.latest().expect("capture observation").clone();
                        snapshot.revision += 1;
                        snapshot.pages.doctor = doctor.page();
                        self.apply_shared_surface_snapshot(snapshot);
                    }
                }
                Task::none()
            }
            Message::RetryDoctorCommand if !self.diag.doctor.action.can_retry() => Task::none(),
            Message::RetryDoctorCommand => match self.diag.doctor.action.retry_action.clone() {
                Some(action) => self.shared_doctor_command(action),
                None => Task::none(),
            },
            Message::DoctorReportReady(result) => {
                self.diag.doctor.is_running = false;
                match result {
                    Ok(report) => {
                        self.diag.doctor.report = Some(report);
                        self.diag.doctor.error = None;
                    }
                    Err(error) => self.diag.doctor.error = Some(error.to_string()),
                }
                Task::none()
            }
            Message::RepairDoctorIssue(id) => {
                self.shared_doctor_command(DoctorAction::RepairOne(id))
            }
            Message::RunDoctorFix => {
                if self.commands.is_some() {
                    return self.shared_doctor_command(DoctorAction::RepairAll);
                }
                if self.diag.doctor.is_fixing {
                    return Task::none();
                }
                self.diag.doctor.is_fixing = true;
                let base = self.admin_api_base();
                Task::perform(
                    async move {
                        AdminApiClient::new(base)
                            .map_err(|e| InfiltratorError::Internal(e.to_string()))?
                            .post::<DoctorFixReport, _>("/api/doctor/fix", &serde_json::json!({}))
                            .await
                            .map_err(|e| InfiltratorError::Internal(e.to_string()))
                    },
                    Message::DoctorFixApplied,
                )
            }
            Message::DoctorFixApplied(result) => {
                self.diag.doctor.is_fixing = false;
                match result {
                    Ok(report) => {
                        // 修复会改动文件系统状态，随后刷新报告。
                        Task::batch(vec![
                            Task::done(Message::RunDoctor),
                            Task::done(Message::ShowToast(
                                doctor_fix_toast(&report, &self.shell.lang),
                                ToastStatus::Success,
                            )),
                        ])
                    }
                    Err(error) => {
                        self.diag.doctor.error = Some(error.to_string());
                        Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::RunBootstrap => {
                if self.commands.is_some() {
                    return self.shared_doctor_command(DoctorAction::Bootstrap);
                }
                if self.diag.doctor.is_bootstrapping {
                    return Task::none();
                }
                self.diag.doctor.is_bootstrapping = true;
                let base = self.admin_api_base();
                Task::perform(
                    async move {
                        AdminApiClient::new(base)
                            .map_err(|e| InfiltratorError::Internal(e.to_string()))?
                            .post::<BootstrapReport, _>("/api/bootstrap", &serde_json::json!({}))
                            .await
                            .map_err(|e| InfiltratorError::Internal(e.to_string()))
                    },
                    Message::BootstrapFinished,
                )
            }
            Message::BootstrapFinished(result) => {
                self.diag.doctor.is_bootstrapping = false;
                match result {
                    Ok(report) => {
                        // 引导同样改动文件系统状态，随后刷新报告。
                        Task::batch(vec![
                            Task::done(Message::RunDoctor),
                            Task::done(Message::ShowToast(
                                bootstrap_toast(&report, &self.shell.lang),
                                ToastStatus::Success,
                            )),
                        ])
                    }
                    Err(error) => {
                        self.diag.doctor.error = Some(error.to_string());
                        Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                    }
                }
            }
            other => self.update_core_proxies(other),
        }
    }
}

fn doctor_fix_toast(report: &DoctorFixReport, code: &str) -> String {
    if report.actions.is_empty() {
        Lang(code).tr("doctor_no_repair_needed").into_owned()
    } else {
        localize(
            code,
            "doctor_repair_result",
            &[
                ("count", report.actions.len().to_string()),
                (
                    "ids",
                    report
                        .actions
                        .iter()
                        .map(|action| action.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            ],
        )
    }
}
fn bootstrap_toast(report: &BootstrapReport, code: &str) -> String {
    let executed = report.steps.iter().filter(|step| step.executed).count();
    localize(
        code,
        "doctor_bootstrap_result",
        &[
            ("executed", executed.to_string()),
            ("skipped", (report.steps.len() - executed).to_string()),
        ],
    )
}
