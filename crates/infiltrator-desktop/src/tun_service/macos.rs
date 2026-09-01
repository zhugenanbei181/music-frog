//! macOS implementation — honestly unsupported in 0.20.
//!
//! A real macOS TUN route requires a Network Extension (or a nerds-style
//! privileged helper). The previous implementation was a fake sudo path:
//! `sudo launchctl` cannot prompt in a GUI session, so it always failed,
//! sometimes while pretending to succeed. All verbs now fail honestly with
//! `UnsupportedPlatformError`, and status checks report `Unsupported`.

use super::{ServiceModeStatus, UnsupportedPlatformError};
use anyhow::Result;
use std::path::Path;

fn unsupported(action: &'static str) -> anyhow::Error {
    UnsupportedPlatformError {
        action,
        platform: "macOS",
    }
    .into()
}

pub(super) fn check_status_for(_binary_path: &Path) -> ServiceModeStatus {
    ServiceModeStatus::Unsupported
}

pub(super) fn install_service(_bin_path: &Path) -> Result<()> {
    Err(unsupported("install_service"))
}

pub(super) fn uninstall_service(_bin_path: &Path) -> Result<()> {
    Err(unsupported("uninstall_service"))
}

pub(super) fn start_service() -> Result<()> {
    Err(unsupported("start_service"))
}

pub(super) fn stop_service() -> Result<()> {
    Err(unsupported("stop_service"))
}

#[cfg(test)]
mod tests {
    use super::super::TunServiceManager;
    use super::*;
    use std::path::Path;

    #[test]
    fn test_tun_macos_status_is_unsupported() {
        assert_eq!(
            TunServiceManager::check_status_for(Path::new("/opt/mihomo/core")),
            ServiceModeStatus::Unsupported
        );
        assert_eq!(
            TunServiceManager::check_status(),
            ServiceModeStatus::Unsupported
        );
    }

    #[test]
    fn test_tun_macos_verbs_return_typed_unsupported() {
        let results = vec![
            TunServiceManager::install_service(Path::new("/opt/mihomo/core")),
            TunServiceManager::uninstall_service(Path::new("/opt/mihomo/core")),
            TunServiceManager::start_service(),
            TunServiceManager::stop_service(),
        ];
        for res in results {
            let err = res.expect_err("macOS verbs must fail honestly");
            let typed = err
                .downcast_ref::<UnsupportedPlatformError>()
                .expect("error must be the typed UnsupportedPlatformError");
            assert_eq!(typed.platform, "macOS");
        }
    }
}
