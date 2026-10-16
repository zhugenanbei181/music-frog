use super::*;

    #[test]
    fn stop_ffi_success_requires_worker_exit_readback() {
        assert_eq!(
            worker_stop_status(WorkerStopOutcome::Stopped),
            FfiStatus::ok()
        );
        assert_eq!(
            worker_stop_status(WorkerStopOutcome::TimedOut).code,
            FfiErrorCode::NotReady,
        );
        assert_eq!(
            worker_stop_status(WorkerStopOutcome::Unavailable).code,
            FfiErrorCode::NotReady,
        );
    }

    #[test]
    fn tun_settings_patch_only_contains_tun_configuration() {
        let patch = VpnTunSettingsPatch {
            mtu: Some(1420),
            auto_route: Some(false),
            strict_route: Some(true),
            stack_update: VpnTunStackUpdate::Set {
                stack: VpnTunStack::System,
            },
            auto_detect_interface: Some(true),
        };
        let result = build_tun_patch(&patch);
        assert_eq!(result.mtu, Some(1420));
        assert_eq!(result.auto_route, Some(false));
        assert_eq!(result.strict_route, Some(true));
        assert_eq!(result.stack, TunStackUpdate::Set(TunStack::System),);
        assert_eq!(result.auto_detect_interface, Some(true));
    }

    #[test]
    fn tun_stack_update_keeps_clear_and_set_distinct_at_the_owner_boundary() {
        let keep = build_tun_patch(&VpnTunSettingsPatch {
            mtu: None,
            auto_route: None,
            strict_route: None,
            stack_update: VpnTunStackUpdate::Keep,
            auto_detect_interface: None,
        });
        assert_eq!(keep.stack, TunStackUpdate::Keep);

        let clear = build_tun_patch(&VpnTunSettingsPatch {
            mtu: None,
            auto_route: None,
            strict_route: None,
            stack_update: VpnTunStackUpdate::Clear,
            auto_detect_interface: None,
        });
        assert_eq!(clear.stack, TunStackUpdate::Clear);
    }
