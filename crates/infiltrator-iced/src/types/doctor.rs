//! Doctor 体检面板：admin API 的响应 DTO（只读反序列化）与面板 UI 状态。
//!
//! 约束：DTO 字段与 `/admin/api/doctor*`、`/admin/api/bootstrap` 的 JSON
//! 一一对应（核心侧只 Serialize，这里在 UI 层镜像只读视图，不反向依赖）。

use serde::Deserialize;

/// 单项检查结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorStatus {
    Pass,
    Warn,
    Fail,
    Skip,
}

/// 一条已执行的检查结果。
#[derive(Debug, Clone, Deserialize)]
pub struct DoctorCheckResult {
    pub id: String,
    pub category: String,
    pub status: DoctorStatus,
    pub summary: String,
    pub detail: Option<String>,
    pub hint: Option<String>,
}

/// 一次体检的完整报告（时间戳为 unix 秒）。
#[derive(Debug, Clone, Deserialize)]
pub struct DoctorReport {
    pub started_at: u64,
    pub finished_at: u64,
    pub checks: Vec<DoctorCheckResult>,
}

impl DoctorReport {
    pub fn count_by_status(&self, status: DoctorStatus) -> usize {
        self.checks
            .iter()
            .filter(|check| check.status == status)
            .count()
    }
}

/// 一次修复实际执行的动作。
#[derive(Debug, Clone, Deserialize)]
pub struct DoctorFixAction {
    pub id: String,
    pub summary: String,
}

/// 修复端点响应；`actions` 为空表示没有需要修复的东西。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DoctorFixReport {
    #[serde(default)]
    pub actions: Vec<DoctorFixAction>,
}

/// 引导端点响应里的单步。
#[derive(Debug, Clone, Deserialize)]
pub struct BootstrapStep {
    pub id: String,
    pub executed: bool,
    pub detail: String,
}

/// 引导端点响应。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct BootstrapReport {
    #[serde(default)]
    pub steps: Vec<BootstrapStep>,
}

/// 体检面板的 UI 状态（诊断域子状态）。
#[derive(Debug, Clone, Default)]
pub struct DoctorPanelState {
    pub report: Option<DoctorReport>,
    pub is_running: bool,
    pub is_fixing: bool,
    pub is_bootstrapping: bool,
    pub error: Option<String>,
}
