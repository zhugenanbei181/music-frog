//! Presentation read-model projection for privilege repair and conflict resolution wizard (FLPARITY-034).

use infiltrator_contract::privilege_repair::{
    PrivilegeElevationAction, PrivilegeRepairIssue, PrivilegeRepairSnapshot,
    SystemProxyWatchdogStatus,
};

/// A projected item for each detected issue in the repair wizard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivilegeRepairIssueProjection {
    pub id: String,
    pub kind_title: &'static str,
    pub title: String,
    pub description: String,
    pub action_button_label: &'static str,
    pub manual_command: Option<String>,
    pub can_auto_resolve: bool,
}

impl From<&PrivilegeRepairIssue> for PrivilegeRepairIssueProjection {
    fn from(issue: &PrivilegeRepairIssue) -> Self {
        let action_button_label = match &issue.recommended_action {
            PrivilegeElevationAction::GrantLinuxCapabilities { .. } => "Polkit 一键授权",
            PrivilegeElevationAction::ElevateWindowsUac => "UAC 管理员授权",
            PrivilegeElevationAction::RebindPort { .. } => "避让可用端口",
            PrivilegeElevationAction::TerminateConflictingProcess { .. } => "结束占用进程",
            PrivilegeElevationAction::RestoreSystemProxy { .. } => "自愈恢复系统代理",
            PrivilegeElevationAction::CleanupGhostAdapter { .. } => "清理残留网卡",
        };

        Self {
            id: issue.id.clone(),
            kind_title: issue.kind.title_zh(),
            title: issue.title.clone(),
            description: issue.description.clone(),
            action_button_label,
            manual_command: issue.manual_shell_command.clone(),
            can_auto_resolve: issue.can_auto_resolve,
        }
    }
}

/// Presentation model for the privilege repair wizard modal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivilegeRepairModalProjection {
    pub is_needed: bool,
    pub headline: String,
    pub subhead: String,
    pub badge_label: &'static str,
    pub issues: Vec<PrivilegeRepairIssueProjection>,
    pub has_auto_fix: bool,
    pub primary_action_label: Option<&'static str>,
    pub manual_command: Option<String>,
}

impl PrivilegeRepairModalProjection {
    pub fn of(snapshot: &PrivilegeRepairSnapshot) -> Self {
        let mut issues: Vec<PrivilegeRepairIssueProjection> =
            snapshot.issues.iter().map(Into::into).collect();

        // If system proxy watchdog detected external tampering, project it as a repair issue
        if let SystemProxyWatchdogStatus::Tampered {
            expected_endpoint,
            actual_endpoint,
            ..
        } = &snapshot.system_proxy_watchdog
        {
            let actual_str = actual_endpoint.as_deref().unwrap_or("已关闭 / 未配置");
            issues.push(PrivilegeRepairIssueProjection {
                id: "system_proxy_tampered_watchdog".to_owned(),
                kind_title: "系统代理配置异常",
                title: "系统代理被外部程序修改".to_owned(),
                description: format!(
                    "预期指向 {expected_endpoint}，但实测被更改为 {actual_str}。建议立即恢复。"
                ),
                action_button_label: "一键恢复系统代理",
                manual_command: None,
                can_auto_resolve: true,
            });
        }

        let is_needed = !issues.is_empty();
        let has_auto_fix = issues.iter().any(|i| i.can_auto_resolve);

        let (headline, subhead, badge_label) = if !is_needed {
            (
                "系统网络与权限状态健康".to_owned(),
                "无端口冲突与权限限制，代理与 TUN 模式可正常就绪。".to_owned(),
                "状态正常",
            )
        } else if issues.iter().any(|i| i.kind_title.contains("端口冲突")) {
            (
                "检测到代理端口冲突".to_owned(),
                "核心服务端口已被其他进程占用，请选择自动避让或关闭冲突程序。".to_owned(),
                "端口冲突",
            )
        } else if issues.iter().any(|i| i.kind_title.contains("提权")) {
            (
                "需要管理员 / 网络管理权限".to_owned(),
                "TUN 虚拟网卡模式需要系统网络设备配置权限，请完成授权。".to_owned(),
                "需要提权",
            )
        } else {
            (
                "网络环境需要修复".to_owned(),
                "检测到异常状态，建议运行一键自愈修复。".to_owned(),
                "待修复",
            )
        };

        let primary_action_label = if issues.is_empty() {
            None
        } else if has_auto_fix {
            Some("一键全部修复")
        } else {
            Some("查看手动修复命令")
        };

        let manual_command = issues.iter().find_map(|i| i.manual_command.clone());

        Self {
            is_needed,
            headline,
            subhead,
            badge_label,
            issues,
            has_auto_fix,
            primary_action_label,
            manual_command,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::privilege_repair::PrivilegeRepairKind;

    #[test]
    fn healthy_snapshot_projects_clean_status() {
        let snapshot = PrivilegeRepairSnapshot::default();
        let proj = PrivilegeRepairModalProjection::of(&snapshot);
        assert!(!proj.is_needed);
        assert_eq!(proj.badge_label, "状态正常");
        assert_eq!(proj.primary_action_label, None);
    }

    #[test]
    fn port_conflict_and_tampered_proxy_projects_issues() {
        let issue = PrivilegeRepairIssue {
            id: "port_7890".to_owned(),
            kind: PrivilegeRepairKind::PortConflict,
            title: "端口 7890 被占用".to_owned(),
            description: "占用进程: clash-verge (PID 12345)".to_owned(),
            recommended_action: PrivilegeElevationAction::RebindPort {
                from_port: 7890,
                to_port: 7891,
            },
            manual_shell_command: Some("kill -9 12345".to_owned()),
            can_auto_resolve: true,
        };

        let snapshot = PrivilegeRepairSnapshot {
            issues: vec![issue],
            system_proxy_watchdog: SystemProxyWatchdogStatus::Tampered {
                expected_endpoint: "127.0.0.1:7890".to_owned(),
                actual_endpoint: None,
                tampered_timestamp_secs: 1730000000,
            },
            revision: 1,
        };

        let proj = PrivilegeRepairModalProjection::of(&snapshot);
        assert!(proj.is_needed);
        assert_eq!(proj.badge_label, "端口冲突");
        assert_eq!(proj.issues.len(), 2);
        assert_eq!(proj.issues[0].action_button_label, "避让可用端口");
        assert_eq!(proj.issues[1].action_button_label, "一键恢复系统代理");
        assert_eq!(proj.primary_action_label, Some("一键全部修复"));
        assert_eq!(proj.manual_command.as_deref(), Some("kill -9 12345"));
    }
}
