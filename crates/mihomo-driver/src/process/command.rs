//! Injectable command boundary for the native Mihomo `-t` config check.
//!
//! The codec owns what makes a candidate, but only this module knows how to
//! invoke the exact selected binary. Tests inject a fake command instead of
//! spawning a process.

use async_trait::async_trait;
use lurker_core_ports::error::PortError;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

/// Whether the selected binary accepted the staged candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeConfigCheck {
    /// The selected binary accepted the staged candidate.
    Accepted,
    /// The selected binary rejected the staged candidate.
    Rejected,
}

/// Outcome of one native `-t` invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeConfigCheckOutcome {
    /// Whether the selected binary accepted the staged candidate.
    pub check: NativeConfigCheck,
    /// Trimmed stderr (falling back to stdout) printed by the rejected check.
    pub detail: String,
}

/// Injectable command boundary for the Mihomo `-t` dry run.
#[async_trait]
pub trait MihomoCommandPort: Send + Sync {
    /// Runs the native `-t` dry run of the selected binary.
    async fn check_config(
        &self,
        binary_path: &Path,
        config_path: &Path,
        config_dir: &Path,
    ) -> Result<NativeConfigCheckOutcome, PortError>;
}

/// The production command boundary: spawn the exact selected binary.
#[derive(Clone, Copy, Debug, Default)]
pub struct TokioMihomoCommandPort;

#[async_trait]
impl MihomoCommandPort for TokioMihomoCommandPort {
    async fn check_config(
        &self,
        binary_path: &Path,
        config_path: &Path,
        config_dir: &Path,
    ) -> Result<NativeConfigCheckOutcome, PortError> {
        let output = Command::new(binary_path)
            .arg("-t")
            .arg("-f")
            .arg(config_path)
            .arg("-d")
            .arg(config_dir)
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output()
            .await
            .map_err(|error| {
                PortError::Failed(format!(
                    "run the selected Mihomo binary for native config validation: {error}"
                ))
            })?;
        if output.status.success() {
            return Ok(NativeConfigCheckOutcome {
                check: NativeConfigCheck::Accepted,
                detail: String::new(),
            });
        }
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let detail = if detail.is_empty() {
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        } else {
            detail
        };
        Ok(NativeConfigCheckOutcome {
            check: NativeConfigCheck::Rejected,
            detail,
        })
    }
}
