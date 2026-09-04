//! Doctor 体检面板 handlers：经内嵌 admin server 的 loopback HTTP 调用
//! `/admin/api/doctor*` 与 `/admin/api/bootstrap`，报告写回 `diag.doctor`。
//!
//! 约束：demo 模式在 dispatcher 层拦截（update.rs），单测只驱动 `Result`
//! 消息、从不发起真实请求；请求失败只落在面板错误位，不弹全局错误条。

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::doctor::{BootstrapReport, DoctorFixReport, DoctorReport};
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::error::InfiltratorError;

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
                if self.diag.doctor.is_running {
                    return Task::none();
                }
                self.diag.doctor.is_running = true;
                self.diag.doctor.error = None;
                let base = self.admin_api_base();
                Task::perform(
                    async move {
                        infiltrator_desktop::admin_client::AdminApiClient::new(base)
                            .map_err(|e| InfiltratorError::Internal(e.to_string()))?
                            .get::<DoctorReport>("/api/doctor")
                            .await
                            .map_err(|e| InfiltratorError::Internal(e.to_string()))
                    },
                    Message::DoctorReportReady,
                )
            }
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
            Message::RunDoctorFix => {
                if self.diag.doctor.is_fixing {
                    return Task::none();
                }
                self.diag.doctor.is_fixing = true;
                let base = self.admin_api_base();
                Task::perform(
                    async move {
                        infiltrator_desktop::admin_client::AdminApiClient::new(base)
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
                                doctor_fix_toast(&report),
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
                if self.diag.doctor.is_bootstrapping {
                    return Task::none();
                }
                self.diag.doctor.is_bootstrapping = true;
                let base = self.admin_api_base();
                Task::perform(
                    async move {
                        infiltrator_desktop::admin_client::AdminApiClient::new(base)
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
                                bootstrap_toast(&report),
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

fn doctor_fix_toast(report: &DoctorFixReport) -> String {
    if report.actions.is_empty() {
        "体检修复：无需修复".to_string()
    } else {
        format!(
            "体检修复：已执行 {} 项（{}）",
            report.actions.len(),
            report
                .actions
                .iter()
                .map(|action| action.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn bootstrap_toast(report: &BootstrapReport) -> String {
    let executed = report.steps.iter().filter(|step| step.executed).count();
    let skipped = report.steps.len() - executed;
    format!("初始化引导：执行 {executed} 步，跳过 {skipped} 步")
}
