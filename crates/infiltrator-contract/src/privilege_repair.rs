//! Shared contract for system proxy tamper watchdog and TUN privilege elevation / conflict repair (FLPARITY-034).

use serde::{Deserialize, Serialize};

/// High-level classification of network environment / privilege conflicts.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivilegeRepairKind {
    /// Missing TUN capabilities (e.g. CAP_NET_ADMIN on Linux or admin rights on Windows).
    MissingTunPrivilege,
    /// Essential listen port (e.g. 7890 mixed, 9090 controller) is occupied by another process.
    PortConflict,
    /// System proxy setting has been overwritten or tampered by an external app or crash.
    SystemProxyTampered,
    /// Leftover ghost TUN adapter interface from a previously crashed process.
    GhostTunAdapter,
}

impl PrivilegeRepairKind {
    pub fn title_zh(&self) -> &'static str {
        match self {
            Self::MissingTunPrivilege => "TUN 虚拟网卡提权要求",
            Self::PortConflict => "代理监听端口冲突",
            Self::SystemProxyTampered => "系统代理配置异常被篡改",
            Self::GhostTunAdapter => "残留虚拟网卡接口冲突",
        }
    }
}

/// A concrete action that can be executed automatically or manually to repair the issue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum PrivilegeElevationAction {
    /// Grant Linux capabilities via pkexec setcap.
    GrantLinuxCapabilities { binary_path: String },
    /// Elevate via Windows UAC prompt.
    ElevateWindowsUac,
    /// Switch conflicted port to a recommended free port.
    RebindPort { from_port: u16, to_port: u16 },
    /// Terminate the conflicting external process by PID.
    TerminateConflictingProcess { pid: u32, process_name: String },
    /// Re-apply expected system proxy settings.
    RestoreSystemProxy { expected_endpoint: String },
    /// Remove stale ghost TUN adapter.
    CleanupGhostAdapter { adapter_name: String },
}

/// An individual privilege or conflict issue detected on the host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivilegeRepairIssue {
    pub id: String,
    pub kind: PrivilegeRepairKind,
    pub title: String,
    pub description: String,
    pub recommended_action: PrivilegeElevationAction,
    pub manual_shell_command: Option<String>,
    pub can_auto_resolve: bool,
}

/// Status of the background system proxy watchdog.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemProxyWatchdogStatus {
    /// Watchdog is idle (system proxy is disabled by user).
    #[default]
    Idle,
    /// Watchdog is actively monitoring and system proxy is in expected state.
    ActiveHealthy,
    /// System proxy was modified by an external process or reset.
    Tampered {
        expected_endpoint: String,
        actual_endpoint: Option<String>,
        tampered_timestamp_secs: u64,
    },
}

impl SystemProxyWatchdogStatus {
    pub fn is_tampered(&self) -> bool {
        matches!(self, Self::Tampered { .. })
    }

    /// Pure function to detect tampering between desired state and actual observation.
    pub fn detect(
        desired_enabled: bool,
        desired_endpoint: Option<&str>,
        observed_enabled: bool,
        observed_endpoint: Option<&str>,
        now_secs: u64,
    ) -> Self {
        if !desired_enabled {
            return Self::Idle;
        }
        let expected = desired_endpoint.unwrap_or("127.0.0.1:7890");
        if !observed_enabled || observed_endpoint != Some(expected) {
            Self::Tampered {
                expected_endpoint: expected.to_owned(),
                actual_endpoint: observed_endpoint.map(str::to_owned),
                tampered_timestamp_secs: now_secs,
            }
        } else {
            Self::ActiveHealthy
        }
    }
}

/// Comprehensive snapshot of host privilege and port health.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivilegeRepairSnapshot {
    pub issues: Vec<PrivilegeRepairIssue>,
    pub system_proxy_watchdog: SystemProxyWatchdogStatus,
    pub revision: u64,
}

impl PrivilegeRepairSnapshot {
    pub fn is_healthy(&self) -> bool {
        self.issues.is_empty() && !self.system_proxy_watchdog.is_tampered()
    }

    pub fn generate_linux_setcap_command(binary_path: &str) -> String {
        format!("sudo setcap cap_net_admin,cap_net_bind_service=+ep \"{binary_path}\"")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_proxy_watchdog_detects_external_tampering() {
        let status = SystemProxyWatchdogStatus::detect(
            true,
            Some("127.0.0.1:7890"),
            true,
            Some("127.0.0.1:10808"), // Tampered by another app
            1730000000,
        );
        assert!(status.is_tampered());
        if let SystemProxyWatchdogStatus::Tampered {
            expected_endpoint,
            actual_endpoint,
            ..
        } = status
        {
            assert_eq!(expected_endpoint, "127.0.0.1:7890");
            assert_eq!(actual_endpoint.as_deref(), Some("127.0.0.1:10808"));
        } else {
            panic!("expected tampered status");
        }
    }

    #[test]
    fn system_proxy_watchdog_detects_unexpected_disable() {
        let status = SystemProxyWatchdogStatus::detect(
            true,
            Some("127.0.0.1:7890"),
            false, // Disabled externally
            None,
            1730000000,
        );
        assert!(status.is_tampered());
    }

    #[test]
    fn linux_setcap_command_formats_safely() {
        let cmd = PrivilegeRepairSnapshot::generate_linux_setcap_command("/opt/musicfrog/core");
        assert_eq!(
            cmd,
            "sudo setcap cap_net_admin,cap_net_bind_service=+ep \"/opt/musicfrog/core\""
        );
    }
}
