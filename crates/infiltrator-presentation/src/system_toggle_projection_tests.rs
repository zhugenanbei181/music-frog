use super::{compact_label, compact_status_line, from_surface, intent};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::runtime_control::RuntimeControlStatus;
use infiltrator_contract::surface::HostKind;
use infiltrator_contract::surface::SurfaceKind;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::system_proxy::SystemProxyStatus;
use infiltrator_contract::system_toggle::{SystemToggle, SystemToggleSnapshot, SystemToggleState};
use lurker_core_contract::error::{ErrorCode, Failure};

#[test]
fn surface_fold_preserves_unobserved_and_failure_states() {
    let mut surface = SurfaceSnapshot::unavailable(
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
        Failure::unsupported("surface unavailable"),
    );
    assert_eq!(from_surface(&surface).tun, SystemToggleState::Unknown);
    surface.system_proxy.status = SystemProxyStatus::Enabled;
    surface.runtime_control.status = RuntimeControlStatus::Ready;
    surface.runtime_control.tun_enabled = Some(false);
    let observed = from_surface(&surface);
    assert_eq!(observed.system_proxy, SystemToggleState::Enabled);
    assert_eq!(observed.tun, SystemToggleState::Disabled);
    assert_eq!(observed.revision, surface.revision);
}

#[test]
fn action_admission_requires_authoritative_non_pending_readback() {
    let observed = SystemToggleSnapshot::from_legacy(true, Some(false), 7);
    assert_eq!(
        intent(&observed, SystemToggle::SystemProxy, false),
        Ok(CommandIntent::SetSystemProxy { enabled: false })
    );
    assert_eq!(
        intent(&observed, SystemToggle::Tun, true),
        Ok(CommandIntent::ToggleTun { enabled: true })
    );
    let pending = observed.with_pending(SystemToggle::SystemProxy, false);
    assert_eq!(
        intent(&pending, SystemToggle::SystemProxy, false)
            .expect_err("pending actions must be fenced")
            .code,
        ErrorCode::InvalidState
    );
    let unknown = SystemToggleSnapshot::from_legacy(false, None, 7);
    assert_eq!(
        intent(&unknown, SystemToggle::Tun, false).expect_err("unobserved controls must be fenced"),
        Failure::new(
            ErrorCode::NotReady,
            "system toggle has no authoritative host readback",
            true
        )
    );
}
#[test]
fn compact_status_localizes_actual_states_and_never_fakes_failed_or_unknown_as_off() {
    let enabled = SystemToggleState::Enabled;
    let pending = SystemToggleState::Pending { desired: true };
    assert_eq!(
        compact_status_line(&enabled, &pending, "zh-CN"),
        "系统代理: 开 · TUN: …"
    );
    assert_eq!(
        compact_status_line(&enabled, &pending, "en-US"),
        "Proxy: On · TUN: …"
    );
    assert_eq!(compact_label(&SystemToggleState::Disabled, "en-US"), "Off");
    for state in [
        SystemToggleState::Unknown,
        SystemToggleState::Unsupported {
            failure: Failure::unsupported("host absent"),
        },
        SystemToggleState::Failed {
            failure: Failure::unsupported("read denied"),
        },
    ] {
        assert_eq!(compact_label(&state, "zh-CN"), "—");
        assert_eq!(compact_label(&state, "en-US"), "—");
    }
}
