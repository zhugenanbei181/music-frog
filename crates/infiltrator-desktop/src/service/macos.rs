//! macOS Privileged Helper Tool contract and secure XPC IPC adaptation.
//!
//! Provides specifications and contracts for macOS background privileged daemons
//! managed by `launchd` / `SMJobBless` / `SMAppService`, including Code Signing
//! Designated Requirements, LaunchDaemon property lists, and secure XPC protocol framing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use super::{ServiceCommand, ServiceResponsePayload};
use crate::tun_service::{ServiceModeStatus, UnsupportedPlatformError};

pub const DEFAULT_MACOS_HELPER_BUNDLE_ID: &str = "com.musicfrog.infiltrator.helper";
pub const DEFAULT_MACOS_APP_BUNDLE_ID: &str = "com.musicfrog.infiltrator";
pub const DEFAULT_MACOS_MACH_SERVICE: &str = "com.musicfrog.infiltrator.helper.xpc";
pub const DEFAULT_MACOS_HELPER_PATH: &str =
    "/Library/PrivilegedHelperTools/com.musicfrog.infiltrator.helper";
pub const DEFAULT_MACOS_LAUNCHD_PLIST_PATH: &str =
    "/Library/LaunchDaemons/com.musicfrog.infiltrator.helper.plist";
pub const DEFAULT_MACOS_TEAM_ID: &str = "MUS1CFR0G0";

pub const AUTH_RIGHT_TUN_MANAGE: &str = "com.musicfrog.infiltrator.tun";
pub const AUTH_RIGHT_PROXY_MANAGE: &str = "com.musicfrog.infiltrator.proxy";

/// Specification descriptor for macOS Privileged Helper Tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacPrivilegedHelperSpec {
    pub helper_bundle_id: String,
    pub app_bundle_id: String,
    pub mach_service_name: String,
    pub helper_binary_path: PathBuf,
    pub launchd_plist_path: PathBuf,
    pub team_id: String,
    pub version: String,
}

impl Default for MacPrivilegedHelperSpec {
    fn default() -> Self {
        Self {
            helper_bundle_id: DEFAULT_MACOS_HELPER_BUNDLE_ID.to_string(),
            app_bundle_id: DEFAULT_MACOS_APP_BUNDLE_ID.to_string(),
            mach_service_name: DEFAULT_MACOS_MACH_SERVICE.to_string(),
            helper_binary_path: PathBuf::from(DEFAULT_MACOS_HELPER_PATH),
            launchd_plist_path: PathBuf::from(DEFAULT_MACOS_LAUNCHD_PLIST_PATH),
            team_id: DEFAULT_MACOS_TEAM_ID.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// macOS Privileged Helper contract generator.
pub struct MacPrivilegedHelperContract;

impl MacPrivilegedHelperContract {
    /// Generates the Code Signing Designated Requirement (DR) string.
    pub fn generate_designated_requirement(team_id: &str, bundle_id: &str) -> String {
        format!(
            "anchor apple generic and identifier \"{bundle_id}\" and (certificate leaf[field.1.2.840.113635.100.6.1.9] /* Developer ID */ or anchor apple generic) and certificate 1[field.1.2.840.113635.100.6.2.6] /* Apple Dev Relations */ and certificate leaf[subject.OU] = \"{team_id}\""
        )
    }

    /// Generates launchd daemon plist XML (`/Library/LaunchDaemons/com.musicfrog.infiltrator.helper.plist`).
    pub fn generate_launchd_plist(spec: &MacPrivilegedHelperSpec) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{bundle_id}</string>
    <key>MachServices</key>
    <dict>
        <key>{mach_service}</key>
        <true/>
    </dict>
    <key>ProgramArguments</key>
    <array>
        <string>{helper_bin}</string>
    </array>
    <key>KeepAlive</key>
    <false/>
    <key>StandardErrorPath</key>
    <string>/var/log/musicfrog-infiltrator-helper.err</string>
    <key>StandardOutPath</key>
    <string>/var/log/musicfrog-infiltrator-helper.log</string>
</dict>
</plist>
"#,
            bundle_id = spec.helper_bundle_id,
            mach_service = spec.mach_service_name,
            helper_bin = spec.helper_binary_path.display()
        )
    }

    /// Generates Helper's embedded Info.plist containing `SMAuthorizedClients` array.
    pub fn generate_helper_info_plist(
        spec: &MacPrivilegedHelperSpec,
        app_designated_requirement: &str,
    ) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>{helper_id}</string>
    <key>CFBundleName</key>
    <string>MusicFrog Infiltrator Helper</string>
    <key>CFBundleVersion</key>
    <string>{version}</string>
    <key>SMAuthorizedClients</key>
    <array>
        <string>{app_req}</string>
    </array>
</dict>
</plist>
"#,
            helper_id = spec.helper_bundle_id,
            version = spec.version,
            app_req = app_designated_requirement
        )
    }

    /// Generates App's embedded Info.plist containing `SMPrivilegedExecutables` dictionary.
    pub fn generate_app_info_plist(
        spec: &MacPrivilegedHelperSpec,
        helper_designated_requirement: &str,
    ) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>{app_id}</string>
    <key>CFBundleName</key>
    <string>MusicFrog Infiltrator</string>
    <key>CFBundleVersion</key>
    <string>{version}</string>
    <key>SMPrivilegedExecutables</key>
    <dict>
        <key>{helper_id}</key>
        <string>{helper_req}</string>
    </dict>
</dict>
</plist>
"#,
            app_id = spec.app_bundle_id,
            version = spec.version,
            helper_id = spec.helper_bundle_id,
            helper_req = helper_designated_requirement
        )
    }

    /// Returns the Authorization Right specifications for macOS Security Framework.
    pub fn authorization_rights_spec() -> HashMap<&'static str, &'static str> {
        let mut map = HashMap::new();
        map.insert(
            AUTH_RIGHT_TUN_MANAGE,
            "Authenticate to configure TUN network routes and virtual adapter interfaces",
        );
        map.insert(
            AUTH_RIGHT_PROXY_MANAGE,
            "Authenticate to configure macOS system network proxy settings",
        );
        map
    }
}

/// Secure XPC message protocol representation for macOS Privileged Helper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct XpcMessage {
    pub protocol_version: u32,
    pub message_id: String,
    pub required_right: Option<String>,
    pub client_pid: u32,
    pub client_bundle_id: String,
    #[serde(flatten)]
    pub command: ServiceCommand,
}

impl XpcMessage {
    pub fn new(
        message_id: impl Into<String>,
        client_bundle_id: impl Into<String>,
        command: ServiceCommand,
    ) -> Self {
        let required_right = match &command {
            ServiceCommand::StartTun { .. } | ServiceCommand::StopTun => {
                Some(AUTH_RIGHT_TUN_MANAGE.to_string())
            }
            ServiceCommand::SetSystemProxy { .. } | ServiceCommand::ClearSystemProxy => {
                Some(AUTH_RIGHT_PROXY_MANAGE.to_string())
            }
            _ => None,
        };

        Self {
            protocol_version: 1,
            message_id: message_id.into(),
            required_right,
            client_pid: std::process::id(),
            client_bundle_id: client_bundle_id.into(),
            command,
        }
    }
}

/// Secure XPC response protocol representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct XpcResponse {
    pub protocol_version: u32,
    pub in_reply_to: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<ServiceResponsePayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl XpcResponse {
    pub fn ok(in_reply_to: impl Into<String>, payload: ServiceResponsePayload) -> Self {
        Self {
            protocol_version: 1,
            in_reply_to: in_reply_to.into(),
            success: true,
            payload: Some(payload),
            error: None,
        }
    }

    pub fn error(in_reply_to: impl Into<String>, msg: impl Into<String>) -> Self {
        Self {
            protocol_version: 1,
            in_reply_to: in_reply_to.into(),
            success: false,
            payload: None,
            error: Some(msg.into()),
        }
    }
}

/// macOS Privileged Helper diagnostics and status inspector.
pub struct MacHelperDoctor;

impl MacHelperDoctor {
    /// Checks the installation and readiness of the macOS Privileged Helper.
    pub fn check_helper_status(spec: &MacPrivilegedHelperSpec) -> ServiceModeStatus {
        let helper_exists = spec.helper_binary_path.exists();
        let plist_exists = spec.launchd_plist_path.exists();

        if helper_exists && plist_exists {
            ServiceModeStatus::InstalledStopped
        } else if !helper_exists && !plist_exists {
            ServiceModeStatus::NotInstalled
        } else {
            ServiceModeStatus::MissingPrivilege
        }
    }

    /// Verifies if the active OS supports Privileged Helper execution.
    pub fn verify_support(action: &'static str) -> Result<(), UnsupportedPlatformError> {
        #[cfg(target_os = "macos")]
        {
            Err(UnsupportedPlatformError {
                action,
                platform: "macOS",
            })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = action;
            Ok(())
        }
    }
}
