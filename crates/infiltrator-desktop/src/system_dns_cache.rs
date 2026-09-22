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
use infiltrator_ports::error::PortError;
use infiltrator_ports::system_dns_cache::SystemDnsCachePort;
use std::process::Command;

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
        tokio::task::spawn_blocking(flush_sync)
            .await
            .map_err(|error| PortError::Io(format!("DNS cache flush worker failed: {error}")))?
    }
}

fn flush_sync() -> Result<bool, PortError> {
    let invocations = flush_invocations(std::env::consts::OS);
    if invocations.is_empty() {
        return Ok(false);
    }
    let mut last_error = None;
    let mut spawned_any = false;
    for (program, args) in invocations {
        match Command::new(program).args(args).output() {
            Ok(output) if output.status.success() => return Ok(true),
            Ok(output) => {
                spawned_any = true;
                last_error = Some(format!(
                    "{program} exited with {}",
                    output
                        .status
                        .code()
                        .map_or_else(|| "a signal".to_owned(), |code| code.to_string())
                ));
            }
            Err(error) => {
                last_error = Some(format!("{program} is not available: {error}"));
            }
        }
    }
    if spawned_any {
        return Err(PortError::Failed(
            last_error.unwrap_or_else(|| "OS DNS cache flush failed".to_owned()),
        ));
    }
    // No platform tool exists on this host: an honest typed unsupported.
    Ok(false)
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
mod tests {
    use super::*;

    #[test]
    fn linux_flush_prefers_resolvectl() {
        let invocations = flush_invocations("linux");
        assert_eq!(invocations[0], ("resolvectl", &["flush-caches"][..]));
        assert_eq!(invocations[1], ("systemd-resolve", &["--flush-caches"][..]));
    }

    #[test]
    fn macos_flush_covers_dscacheutil_and_mdnsresponder() {
        let invocations = flush_invocations("macos");
        assert_eq!(invocations[0], ("dscacheutil", &["-flushcache"][..]));
        assert_eq!(invocations[1], ("killall", &["-HUP", "mDNSResponder"][..]));
    }

    #[test]
    fn windows_flush_uses_ipconfig() {
        assert_eq!(
            flush_invocations("windows"),
            vec![("ipconfig", &["/flushdns"][..])]
        );
    }

    #[test]
    fn an_unknown_platform_reports_typed_unsupported() {
        assert!(flush_invocations("redox").is_empty());
        assert!(flush_invocations("").is_empty());
    }
}
