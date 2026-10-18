//! Native sing-box binary version probing.
//!
//! The probe reads the version the executable reports for itself. It never
//! infers the running version from a download manifest or an executable path.

use async_trait::async_trait;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreVersion;
use lurker_core_ports::error::PortError;
use std::path::Path;
use tokio::process::Command;

/// Reads the version reported by one sing-box executable.
#[async_trait]
pub trait SingBoxBinaryInspector: Send + Sync {
    async fn version(&self, binary: &Path) -> Result<CoreVersion, PortError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessSingBoxBinaryInspector;

#[async_trait]
impl SingBoxBinaryInspector for ProcessSingBoxBinaryInspector {
    async fn version(&self, binary: &Path) -> Result<CoreVersion, PortError> {
        let output = Command::new(binary)
            .arg("-v")
            .output()
            .await
            .map_err(|error| PortError::Failed(format!("run sing-box version probe: {error}")))?;
        if !output.status.success() {
            return Err(PortError::Failed(format!(
                "sing-box version probe exited with {}",
                output.status
            )));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_binary_version(&stdout)
    }
}

/// Parses the first line of `sing-box -v` output into the canonical
/// `v<major>.<minor>.<patch>` identity used by operation evidence locks.
pub fn parse_binary_version(output: &str) -> Result<CoreVersion, PortError> {
    let first_line = output.lines().next().unwrap_or_default().trim();
    let version = first_line
        .strip_prefix("sing-box version ")
        .or_else(|| first_line.strip_prefix("sing-box "))
        .ok_or_else(|| {
            PortError::Failed("sing-box version output has an unexpected engine identity".into())
        })?;
    let version = version.split_whitespace().next().unwrap_or_default();
    let canonical = if version.starts_with('v') {
        version.to_owned()
    } else {
        format!("v{version}")
    };
    validate_version(&canonical)?;
    CoreVersion::new(canonical).map_err(|error| PortError::Failed(error.to_string()))
}

/// Rejects versions that cannot be used as an exact on-disk and identity
/// selector. Accepts only `v<major>.<minor>.<patch>` with optional pre-release
/// or build suffixes.
pub fn validate_version(version: &str) -> Result<(), PortError> {
    let canonical = version.strip_prefix('v').ok_or_else(|| {
        invalid_version("sing-box version must use the canonical v<version> form")
    })?;
    let numeric = canonical.split(['-', '+']).next().unwrap_or_default();
    let components: Vec<&str> = numeric.split('.').collect();
    if canonical.is_empty()
        || components.len() != 3
        || components.iter().any(|component| {
            component.is_empty() || !component.bytes().all(|byte| byte.is_ascii_digit())
        })
        || !canonical
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
    {
        return Err(invalid_version(
            "sing-box version contains invalid filesystem or identity characters",
        ));
    }
    CoreVersion::new(version.to_owned())
        .map(|_| ())
        .map_err(|error| invalid_version(error.to_string()))
}

fn invalid_version(reason: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::Configuration, reason, false))
}

#[cfg(test)]
mod tests {
    use super::parse_binary_version;

    #[test]
    fn probe_requires_sing_box_identity_and_canonical_version() {
        let version =
            parse_binary_version("sing-box version 1.14.3\nEnvironment: go1.25.0 linux/amd64\n")
                .unwrap();
        assert_eq!(version.raw(), "v1.14.3");
        assert!(parse_binary_version("mihomo version v1.14.3").is_err());
        assert!(parse_binary_version("sing-box version unknown").is_err());
        assert!(parse_binary_version("sing-box version 1.14").is_err());
    }
}
