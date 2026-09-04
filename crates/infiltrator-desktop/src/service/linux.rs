//! Linux Polkit policy, setcap capability automation, and permission diagnosis wizard.
//!
//! Provides automated permission checks, capability diagnostics, Polkit policy XML
//! generation, standalone setup scripts, and systemd service unit generation.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::process::Command;

pub const POLKIT_ACTION_ID: &str = "com.musicfrog.infiltrator.setcap";
pub const POLKIT_POLICY_PATH: &str = "/usr/share/polkit-1/actions/com.musicfrog.infiltrator.policy";
pub const POLKIT_RULE_PATH: &str = "/etc/polkit-1/rules.d/50-musicfrog-infiltrator.rules";
pub const DEFAULT_LINUX_CAPABILITIES: &str = "cap_net_admin,cap_net_bind_service+ep";
pub const TUN_DEVICE_PATH: &str = "/dev/net/tun";

/// Result of an individual diagnostic check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticCheck {
    pub name: String,
    pub passed: bool,
    pub details: String,
    pub remediation: Option<String>,
}

/// Comprehensive diagnostic report for Linux permissions and capability requirements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinuxDiagnosticReport {
    pub has_pkexec: bool,
    pub has_setcap: bool,
    pub has_getcap: bool,
    pub tun_device_available: bool,
    pub capabilities_granted: bool,
    pub polkit_policy_installed: bool,
    pub checks: Vec<DiagnosticCheck>,
}

impl LinuxDiagnosticReport {
    pub fn is_ready_for_tun(&self) -> bool {
        self.tun_device_available && self.capabilities_granted
    }

    pub fn can_automate_elevation(&self) -> bool {
        self.has_pkexec && self.has_setcap
    }

    pub fn summary(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "Linux Privilege Diagnostic (Ready: {})",
            if self.is_ready_for_tun() { "YES" } else { "NO" }
        ));
        for check in &self.checks {
            let mark = if check.passed { "[PASS]" } else { "[FAIL]" };
            lines.push(format!("  {} {}: {}", mark, check.name, check.details));
            if let Some(remediation) = &check.remediation {
                lines.push(format!("         Remediation: {}", remediation));
            }
        }
        lines.join("\n")
    }
}

impl fmt::Display for LinuxDiagnosticReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

/// Linux Privilege and Permission Wizard.
pub struct LinuxPrivilegeWizard;

impl LinuxPrivilegeWizard {
    /// Checks if a binary command exists in system PATH.
    pub fn is_command_in_path(cmd: &str) -> bool {
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let full = dir.join(cmd);
                if full.is_file() {
                    return true;
                }
            }
        }
        false
    }

    /// Checks whether `/dev/net/tun` exists and is accessible.
    pub fn check_tun_device() -> (bool, String) {
        let path = Path::new(TUN_DEVICE_PATH);
        if !path.exists() {
            return (
                false,
                format!("{TUN_DEVICE_PATH} does not exist (kernel module 'tun' may not be loaded)"),
            );
        }
        #[cfg(unix)]
        {
            use std::fs::OpenOptions;
            match OpenOptions::new().read(true).write(true).open(path) {
                Ok(_) => (true, format!("{TUN_DEVICE_PATH} is accessible for R/W")),
                Err(e) => (
                    true,
                    format!("{TUN_DEVICE_PATH} exists but requires elevated privileges ({e})"),
                ),
            }
        }
        #[cfg(not(unix))]
        {
            (true, format!("{TUN_DEVICE_PATH} exists"))
        }
    }

    /// Parses output from `getcap <binary>` to detect required capabilities.
    pub fn parse_getcap_output(output: &str) -> bool {
        let s = output.to_lowercase();
        s.contains("cap_net_admin") && s.contains("cap_net_bind_service")
    }

    /// Runs full diagnostic checks against a core binary.
    pub fn diagnose(binary_path: &Path) -> LinuxDiagnosticReport {
        let has_pkexec = Self::is_command_in_path("pkexec");
        let has_setcap = Self::is_command_in_path("setcap");
        let has_getcap = Self::is_command_in_path("getcap");
        let (tun_device_available, tun_details) = Self::check_tun_device();

        // Check capabilities
        let mut capabilities_granted = false;
        let mut cap_details = "Core binary does not have cap_net_admin capabilities".to_string();
        if has_getcap && binary_path.exists() {
            if let Ok(out) = Command::new("getcap").arg(binary_path).output() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if Self::parse_getcap_output(&stdout) {
                    capabilities_granted = true;
                    cap_details = format!("Capabilities verified: {}", stdout.trim());
                }
            }
        } else if !binary_path.exists() {
            cap_details = format!("Binary not found at {}", binary_path.display());
        }

        // Check polkit policy installation
        let polkit_policy_installed = Path::new(POLKIT_POLICY_PATH).exists();

        let mut checks = Vec::new();
        checks.push(DiagnosticCheck {
            name: "Polkit Agent (pkexec)".to_string(),
            passed: has_pkexec,
            details: if has_pkexec {
                "pkexec binary found in PATH".to_string()
            } else {
                "pkexec not found. Install 'polkit' or 'policykit-1' package.".to_string()
            },
            remediation: if !has_pkexec {
                Some("sudo apt install policykit-1 (or pacman -S polkit)".to_string())
            } else {
                None
            },
        });

        checks.push(DiagnosticCheck {
            name: "Capability Tools (libcap)".to_string(),
            passed: has_setcap && has_getcap,
            details: if has_setcap && has_getcap {
                "setcap and getcap tools available".to_string()
            } else {
                "libcap tools missing. Install 'libcap2-bin' or 'libcap'.".to_string()
            },
            remediation: if !has_setcap || !has_getcap {
                Some("sudo apt install libcap2-bin (or pacman -S libcap)".to_string())
            } else {
                None
            },
        });

        checks.push(DiagnosticCheck {
            name: "TUN Device (/dev/net/tun)".to_string(),
            passed: tun_device_available,
            details: tun_details,
            remediation: if !tun_device_available {
                Some("sudo modprobe tun".to_string())
            } else {
                None
            },
        });

        checks.push(DiagnosticCheck {
            name: "Core Binary Capabilities".to_string(),
            passed: capabilities_granted,
            details: cap_details,
            remediation: if !capabilities_granted {
                Some(format!(
                    "pkexec setcap cap_net_admin,cap_net_bind_service+ep \"{}\"",
                    binary_path.display()
                ))
            } else {
                None
            },
        });

        checks.push(DiagnosticCheck {
            name: "Polkit Action Policy".to_string(),
            passed: polkit_policy_installed,
            details: if polkit_policy_installed {
                format!("Policy file present at {POLKIT_POLICY_PATH}")
            } else {
                "Polkit policy file not installed (pkexec will prompt for root password)"
                    .to_string()
            },
            remediation: if !polkit_policy_installed {
                Some(format!("Install policy file to {POLKIT_POLICY_PATH}"))
            } else {
                None
            },
        });

        LinuxDiagnosticReport {
            has_pkexec,
            has_setcap,
            has_getcap,
            tun_device_available,
            capabilities_granted,
            polkit_policy_installed,
            checks,
        }
    }

    /// Generates Polkit policy XML content (`com.musicfrog.infiltrator.policy`).
    pub fn generate_polkit_action_xml(setcap_path: Option<&str>) -> String {
        let setcap_bin = setcap_path.unwrap_or("/usr/sbin/setcap");
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE policyconfig PUBLIC "-//freedesktop//DTD PolicyKit Policy Configuration 1.0//EN"
"http://www.freedesktop.org/standards/PolicyKit/1/policyconfig.dtd">
<policyconfig>
  <vendor>MusicFrog Infiltrator</vendor>
  <vendor_url>https://github.com/musicfrog/infiltrator</vendor_url>
  <icon_name>network-vpn</icon_name>
  <action id="{POLKIT_ACTION_ID}">
    <description>Grant network administration capabilities to Infiltrator core</description>
    <message>Authentication is required to configure network capabilities for TUN mode</message>
    <defaults>
      <allow_any>no</allow_any>
      <allow_inactive>no</allow_inactive>
      <allow_active>auth_admin_keep</allow_active>
    </defaults>
    <annotate key="org.freedesktop.policykit.exec.path">{setcap_bin}</annotate>
    <annotate key="org.freedesktop.policykit.exec.allow_gui">true</annotate>
  </action>
</policyconfig>
"#
        )
    }

    /// Generates a Polkit javascript rule file (`50-musicfrog-infiltrator.rules`).
    pub fn generate_polkit_rule() -> String {
        r#"/* Polkit rule for MusicFrog Infiltrator TUN capability management */
polkit.addRule(function(action, subject) {
    if (action.id == "com.musicfrog.infiltrator.setcap" && subject.isInGroup("wheel")) {
        return polkit.Result.YES;
    }
    if (action.id == "com.musicfrog.infiltrator.setcap" && subject.isInGroup("sudo")) {
        return polkit.Result.YES;
    }
});
"#
        .to_string()
    }

    /// Generates a standalone self-contained installation/permission shell script.
    pub fn generate_install_script(binary_path: &Path) -> String {
        let bin_str = binary_path.to_string_lossy();
        format!(
            r#"#!/bin/bash
# MusicFrog Infiltrator - Linux Privilege & Capability Setup Script
set -euo pipefail

CORE_BIN="{bin_str}"
CAPS="cap_net_admin,cap_net_bind_service+ep"

echo "=== MusicFrog Infiltrator Privilege Setup ==="

# 1. Check root / sudo
if [ "$(id -u)" -ne 0 ]; then
    echo "This script requires root privileges to set capabilities."
    if command -v pkexec >/dev/null 2>&1; then
        echo "Re-running via pkexec..."
        exec pkexec "$0" "$@"
    elif command -v sudo >/dev/null 2>&1; then
        echo "Re-running via sudo..."
        exec sudo "$0" "$@"
    else
        echo "Error: Neither pkexec nor sudo found. Please run as root." >&2
        exit 1
    fi
fi

# 2. Check binary existence
if [ ! -f "$CORE_BIN" ]; then
    echo "Warning: Core binary not found at $CORE_BIN"
fi

# 3. Ensure TUN kernel module is loaded
if [ ! -c /dev/net/tun ]; then
    echo "Loading TUN kernel module..."
    modprobe tun || true
fi

# 4. Apply capabilities
if command -v setcap >/dev/null 2>&1; then
    echo "Granting $CAPS to $CORE_BIN..."
    setcap "$CAPS" "$CORE_BIN"
    echo "Capabilities successfully applied!"
else
    echo "Error: 'setcap' command not found. Please install libcap2-bin or libcap." >&2
    exit 1
fi

# 5. Verify
if command -v getcap >/dev/null 2>&1; then
    echo "Verifying capabilities:"
    getcap "$CORE_BIN"
fi

echo "=== Setup Completed Successfully ==="
"#
        )
    }

    /// Generates a standalone removal shell script.
    pub fn generate_uninstall_script(binary_path: &Path) -> String {
        let bin_str = binary_path.to_string_lossy();
        format!(
            r#"#!/bin/bash
# MusicFrog Infiltrator - Linux Capability Removal Script
set -euo pipefail

CORE_BIN="{bin_str}"

echo "=== MusicFrog Infiltrator Capability Teardown ==="

if [ "$(id -u)" -ne 0 ]; then
    if command -v pkexec >/dev/null 2>&1; then
        exec pkexec "$0" "$@"
    elif command -v sudo >/dev/null 2>&1; then
        exec sudo "$0" "$@"
    else
        echo "Error: Please run as root." >&2
        exit 1
    fi
fi

if command -v setcap >/dev/null 2>&1 && [ -f "$CORE_BIN" ]; then
    echo "Removing capabilities from $CORE_BIN..."
    setcap -r "$CORE_BIN"
    echo "Capabilities removed successfully."
fi
"#
        )
    }

    /// Generates a systemd unit service file (`musicfrog-infiltrator.service`).
    pub fn generate_systemd_service(
        service_name: &str,
        binary_path: &Path,
        config_path: Option<&Path>,
        user: Option<&str>,
    ) -> String {
        let bin_str = binary_path.to_string_lossy();
        let config_arg = if let Some(cfg) = config_path {
            format!(" -f \"{}\"", cfg.to_string_lossy())
        } else {
            String::new()
        };
        let user_line = if let Some(u) = user {
            format!("User={u}\nGroup={u}\n")
        } else {
            String::new()
        };

        format!(
            r#"[Unit]
Description={service_name} (MusicFrog Infiltrator Privileged Core)
After=network.target network-online.target
Wants=network-online.target

[Service]
Type=simple
{user_line}ExecStart={bin_str}{config_arg}
Restart=always
RestartSec=3
AmbientCapabilities=CAP_NET_ADMIN CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_ADMIN CAP_NET_BIND_SERVICE
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=full
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
"#
        )
    }
}
