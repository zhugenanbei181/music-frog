//! Desktop operating-system DNS resolver cache adapter.
//!
//! The Flush action is user-initiated, so this adapter runs the platform's
//! documented cache-flush command and reports the honest outcome:
//!
//! * Linux: `resolvectl flush-caches`, then the legacy
//!   `systemd-resolve --flush-caches`;
//! * macOS: `dscacheutil -flushcache` and the `mDNSResponder` HUP;
//! * Windows: `ipconfig /flushdns`.
//!
//! A platform without any of these binaries answers `Ok(false)` — the typed
//! unsupported the application publishes — instead of claiming a refresh.

use async_trait::async_trait;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_ports::error::PortError;
use infiltrator_ports::system_dns_cache::SystemDnsCachePort;
use std::env::consts::OS;
use std::io::{self, ErrorKind};
use std::process::{Command, Output};
use tokio::task::spawn_blocking;

/// Stateless desktop adapter; the host composition shares one instance.
#[derive(Clone, Copy, Debug, Default)]
pub struct DesktopSystemDnsCache;

impl DesktopSystemDnsCache {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SystemDnsCachePort for DesktopSystemDnsCache {
    async fn flush_system_cache(&self) -> Result<bool, PortError> {
        spawn_blocking(flush_sync)
            .await
            .map_err(|error| PortError::Io(format!("DNS cache flush worker failed: {error}")))?
    }
}

fn flush_sync() -> Result<bool, PortError> {
    flush_with(OS, |program, args| {
        Command::new(program).args(args).output()
    })
}

/// Linux tools are alternatives; both macOS cache layers must complete.
fn flush_with(
    target: &str,
    mut execute: impl FnMut(&str, &[&str]) -> io::Result<Output>,
) -> Result<bool, PortError> {
    let invocations = flush_invocations(target);
    if invocations.is_empty() {
        return Ok(false);
    }
    let sequence = target == "macos";
    let mut completed = 0;
    let mut first_failure = None;
    for (program, args) in invocations {
        match execute(program, args) {
            Ok(output) if output.status.success() => {
                completed += 1;
                if !sequence {
                    return Ok(true);
                }
            }
            Ok(output) => {
                let detail: String = String::from_utf8_lossy(&output.stderr)
                    .trim()
                    .chars()
                    .take(512)
                    .collect();
                let message = format!(
                    "{program} exited with {}: {detail}",
                    output
                        .status
                        .code()
                        .map_or_else(|| "a signal".into(), |code| code.to_string())
                );
                let folded = detail.to_lowercase();
                let permission = [
                    "permission denied",
                    "access denied",
                    "access is denied",
                    "not authorized",
                    "not authorised",
                    "operation not permitted",
                    "authentication is required",
                ]
                .iter()
                .any(|needle| folded.contains(needle));
                let failure = if permission {
                    PortError::PermissionDenied(message)
                } else {
                    PortError::Rejected(Failure::new(ErrorCode::Internal, message, true))
                };
                if sequence || permission {
                    return Err(failure);
                }
                if first_failure.is_none() {
                    first_failure = Some(failure);
                }
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                if sequence {
                    return if completed == 0 {
                        Ok(false)
                    } else {
                        Err(PortError::Rejected(Failure::new(
                            ErrorCode::NotReady,
                            format!(
                                "system cache clearing partly completed, but {program} is unavailable"
                            ),
                            true,
                        )))
                    };
                }
            }
            Err(error) => {
                let failure = if error.kind() == ErrorKind::PermissionDenied {
                    PortError::PermissionDenied(format!("{program}: {error}"))
                } else {
                    PortError::Io(format!("{program}: {error}"))
                };
                if sequence || error.kind() == ErrorKind::PermissionDenied {
                    return Err(failure);
                }
                if first_failure.is_none() {
                    first_failure = Some(failure);
                }
            }
        }
    }
    if sequence || completed > 0 {
        Ok(true)
    } else if let Some(failure) = first_failure {
        Err(failure)
    } else {
        Ok(false)
    }
}

/// The platform flush commands in preference order, selected by target OS.
pub(crate) fn flush_invocations(target: &str) -> Vec<(&'static str, &'static [&'static str])> {
    match target {
        "linux" => vec![
            ("resolvectl", &["flush-caches"]),
            ("systemd-resolve", &["--flush-caches"]),
        ],
        "macos" => vec![
            ("dscacheutil", &["-flushcache"]),
            ("killall", &["-HUP", "mDNSResponder"]),
        ],
        "windows" => vec![("ipconfig", &["/flushdns"])],
        _ => Vec::new(),
    }
}

#[cfg(test)]
#[path = "system_dns_cache_test.rs"]
mod tests;
