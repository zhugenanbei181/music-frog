use infiltrator_contract::vpn::{VpnSessionSnapshot, VpnSessionState};
use lurker_core_contract::error::{ErrorCode, Failure};

pub(super) fn confirmed(snapshot: VpnSessionSnapshot) -> Result<(), Failure> {
    match snapshot.state {
        VpnSessionState::PermissionRequired => Err(Failure::new(
            ErrorCode::Permission,
            "Android VPN authorization is required before starting the proxy",
            false,
        )),
        VpnSessionState::Unsupported { reason } => Err(Failure::unsupported(reason)),
        VpnSessionState::Failed { failure } => Err(failure),
        VpnSessionState::Running if snapshot.foreground => Ok(()),
        VpnSessionState::Starting => Err(Failure::new(
            ErrorCode::NotReady,
            "VpnService accepted startup but the proxy data path is not running yet",
            true,
        )),
        _ => Err(Failure::new(
            ErrorCode::NotReady,
            "VpnService did not reach foreground Running state",
            true,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_completes_only_after_foreground_running_readback() {
        let snapshot = VpnSessionSnapshot::running(3, 1500, 1, Vec::new(), false, true);
        assert_eq!(confirmed(snapshot), Ok(()));
    }

    #[test]
    fn accepted_but_starting_is_retryable_not_success() {
        let snapshot = VpnSessionSnapshot {
            state: VpnSessionState::Starting,
            foreground: true,
            ..VpnSessionSnapshot::default()
        };
        let failure = confirmed(snapshot).expect_err("starting is not running");
        assert_eq!(failure.code, ErrorCode::NotReady);
        assert!(failure.retryable);
    }

    #[test]
    fn running_without_foreground_protection_is_retryable_not_success() {
        let snapshot = VpnSessionSnapshot::running(3, 1500, 1, Vec::new(), false, false);
        let failure = confirmed(snapshot).expect_err("foreground state is required");
        assert_eq!(failure.code, ErrorCode::NotReady);
        assert!(failure.retryable);
    }

    #[test]
    fn authorization_required_remains_a_permission_failure() {
        let snapshot = VpnSessionSnapshot {
            state: VpnSessionState::PermissionRequired,
            ..VpnSessionSnapshot::default()
        };
        let failure = confirmed(snapshot).expect_err("VPN consent is required");
        assert_eq!(failure.code, ErrorCode::Permission);
        assert!(!failure.retryable);
    }
}
