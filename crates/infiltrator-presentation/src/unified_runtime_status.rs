//! Unified runtime status projection across Bevy, Compose, and Slint surfaces (BANDROID-029.2).
//!
//! Evaluates the independent status axes from `SurfaceSnapshot` and `RuntimeStatusSnapshot`
//! according to the Bottom-Up resolution ladder:
//! 1. Demo origin is explicitly flagged as `Demo` and never masquerades as running;
//! 2. Core configuration/process crash maps to `ConfigurationFailed`;
//! 3. VPN consent missing or denied maps to `PermissionDenied`;
//! 4. TUN data-plane fault or revocation maps to `VpnFailed`;
//! 5. Controller API communication failure maps to `ApiDisconnected`;
//! 6. Lifecycle transition in flight maps to `Starting`;
//! 7. Verified active execution maps to `Success` with explicit back-references to
//!    its authoritative facts (`core_generation`, `vpn_generation`, `outbound_id`);
//! 8. Unobserved or stopped state maps to `Unknown`.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::outbound::identity::OutboundId;
use infiltrator_contract::runtime_status::{
    ControllerApiObservation, CoreProcessObservation, ProxyEffectivenessObservation,
    RuntimeStatusSnapshot, TunDataPlaneObservation, VpnAuthorizationObservation,
};
use infiltrator_contract::surface_snapshot::{SurfaceOrigin, SurfaceSnapshot};
use infiltrator_shared::locales::{Lang, Localizer};

/// The 8 unified runtime status semantics shared across Bevy, Compose, and Slint (BANDROID-029.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnifiedRuntimeStatus {
    /// 1. Starting / Connecting (启动中).
    Starting { detail: Option<String> },
    /// 2. Success / Verified Running (成功) - strictly points back to its single authoritative proof source!
    Success {
        core_generation: u64,
        vpn_generation: u64,
        outbound_id: Option<OutboundId>,
    },
    /// 3. Permission Denied / Authorization Required (拒权).
    PermissionDenied { reason: String },
    /// 4. Configuration Failed / Core Process Crash (配置失败).
    ConfigurationFailed { failure: Failure },
    /// 5. Controller API Disconnected / Unreachable (API 断连).
    ApiDisconnected { failure: Option<Failure> },
    /// 6. VPN Data-plane Invalid / Revoked / Failed (VPN 失效).
    VpnFailed { reason: String },
    /// 7. Demo Fixture Mode (demo) - explicitly flagged!
    Demo { fixture_name: String },
    /// 8. Unknown / Initial / Unobserved State (未知).
    Unknown,
}

impl UnifiedRuntimeStatus {
    /// Whether this status represents verified active execution.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// Whether this status represents a demo fixture.
    pub fn is_demo(&self) -> bool {
        matches!(self, Self::Demo { .. })
    }

    /// Whether this status is an in-flight starting transition.
    pub fn is_starting(&self) -> bool {
        matches!(self, Self::Starting { .. })
    }
}

/// Project unified runtime status from a full `SurfaceSnapshot`.
pub fn project_unified_runtime_status(snapshot: &SurfaceSnapshot) -> UnifiedRuntimeStatus {
    project_from_axes(snapshot.origin, &snapshot.runtime_status)
}

/// Project unified runtime status from raw origin and `RuntimeStatusSnapshot` axes.
pub fn project_from_axes(
    origin: SurfaceOrigin,
    runtime: &RuntimeStatusSnapshot,
) -> UnifiedRuntimeStatus {
    // 1. Demo origin is explicitly flagged
    if origin == SurfaceOrigin::Demo
        || matches!(
            runtime.proxy_effectiveness,
            ProxyEffectivenessObservation::DemoFixture
        )
    {
        return UnifiedRuntimeStatus::Demo {
            fixture_name: "Demo Fixture".into(),
        };
    }

    // 2. Core configuration / process crash
    if let CoreProcessObservation::Failed { failure } = &runtime.core_process {
        return UnifiedRuntimeStatus::ConfigurationFailed {
            failure: failure.clone().unwrap_or_else(|| {
                Failure::new(ErrorCode::InvalidState, "Core process failed", true)
            }),
        };
    }

    // 3. Permission denied / Authorization required
    match &runtime.vpn_authorization {
        VpnAuthorizationObservation::Required => {
            return UnifiedRuntimeStatus::PermissionDenied {
                reason: "VPN permission required".into(),
            };
        }
        VpnAuthorizationObservation::Unavailable { failure } => {
            return UnifiedRuntimeStatus::PermissionDenied {
                reason: failure.message.clone(),
            };
        }
        _ => {}
    }
    if matches!(
        runtime.tun_data_plane,
        TunDataPlaneObservation::PermissionRequired
    ) {
        return UnifiedRuntimeStatus::PermissionDenied {
            reason: "TUN permission required".into(),
        };
    }

    // 4. VPN data plane failure / revocation
    match &runtime.tun_data_plane {
        TunDataPlaneObservation::Failed { failure }
        | TunDataPlaneObservation::Unavailable { failure } => {
            return UnifiedRuntimeStatus::VpnFailed {
                reason: failure.clone(),
            };
        }
        TunDataPlaneObservation::Revoked => {
            return UnifiedRuntimeStatus::VpnFailed {
                reason: "VPN authorization revoked by system".into(),
            };
        }
        TunDataPlaneObservation::Inconsistent {
            lifecycle,
            readiness,
        } => {
            return UnifiedRuntimeStatus::VpnFailed {
                reason: format!("Inconsistent VPN state: {lifecycle:?} / {readiness:?}"),
            };
        }
        _ => {}
    }

    // 5. Controller API disconnected
    match &runtime.controller_api {
        ControllerApiObservation::Unavailable { failure }
        | ControllerApiObservation::Unsupported { failure } => {
            return UnifiedRuntimeStatus::ApiDisconnected {
                failure: Some(failure.clone()),
            };
        }
        ControllerApiObservation::NotRunning
            if matches!(
                runtime.core_process,
                CoreProcessObservation::Running | CoreProcessObservation::Ready
            ) =>
        {
            return UnifiedRuntimeStatus::ApiDisconnected { failure: None };
        }
        _ => {}
    }

    // 6. Starting / In-flight transitions
    if matches!(
        runtime.core_process,
        CoreProcessObservation::Starting | CoreProcessObservation::Stopping
    ) || matches!(
        runtime.tun_data_plane,
        TunDataPlaneObservation::StartRequested
            | TunDataPlaneObservation::Starting { .. }
            | TunDataPlaneObservation::CleanupPending { .. }
    ) {
        return UnifiedRuntimeStatus::Starting {
            detail: match &runtime.tun_data_plane {
                TunDataPlaneObservation::Starting { readiness } => Some(format!("{readiness:?}")),
                _ => None,
            },
        };
    }

    // 7. Success: Core Running + Controller API Connected + TUN Ready
    if let (
        CoreProcessObservation::Running | CoreProcessObservation::Ready,
        ControllerApiObservation::Connected {
            core_generation, ..
        },
        TunDataPlaneObservation::Ready { generation },
    ) = (
        &runtime.core_process,
        &runtime.controller_api,
        &runtime.tun_data_plane,
    ) {
        let outbound_id = match &runtime.proxy_effectiveness {
            ProxyEffectivenessObservation::ProxiedTrafficObserved { outbound_id, .. } => {
                Some(outbound_id.clone())
            }
            _ => None,
        };
        return UnifiedRuntimeStatus::Success {
            core_generation: *core_generation,
            vpn_generation: generation.value(),
            outbound_id,
        };
    }

    // 8. Unknown / Unobserved
    UnifiedRuntimeStatus::Unknown
}

/// Format the unified status into localized human-readable copy.
pub fn format_unified_status(status: &UnifiedRuntimeStatus, language: &str) -> String {
    let lang = Lang(language);
    match status {
        UnifiedRuntimeStatus::Starting { .. } => lang.tr("status_starting").into_owned(),
        UnifiedRuntimeStatus::Success { .. } => lang.tr("status_running").into_owned(),
        UnifiedRuntimeStatus::PermissionDenied { .. } => {
            lang.tr("vpn_status_permission").into_owned()
        }
        UnifiedRuntimeStatus::ConfigurationFailed { .. } => lang.tr("status_error").into_owned(),
        UnifiedRuntimeStatus::ApiDisconnected { .. } => {
            lang.tr("net_roam_status_failed").into_owned()
        }
        UnifiedRuntimeStatus::VpnFailed { .. } => lang.tr("vpn_status_failed").into_owned(),
        UnifiedRuntimeStatus::Demo { .. } => lang.tr("core_source_demo").into_owned(),
        UnifiedRuntimeStatus::Unknown => lang.tr("status_stopped").into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::core_identity::EngineId;
    use infiltrator_contract::error::ErrorCode;
    use infiltrator_contract::surface::{HostKind, SurfaceKind};
    use infiltrator_contract::vpn::VpnGeneration;

    #[test]
    fn demo_origin_is_explicitly_flagged_and_never_masquerades_as_running() {
        let mut snapshot = SurfaceSnapshot::unavailable(
            SurfaceKind::BevyAndroid,
            HostKind::Android,
            Failure::new(ErrorCode::NotReady, "initial", true),
        );
        snapshot.origin = SurfaceOrigin::Demo;
        let status = project_unified_runtime_status(&snapshot);
        assert!(status.is_demo());
        assert!(!status.is_success());
        assert_eq!(
            format_unified_status(&status, "zh-CN"),
            "演示数据 · 未接实时内核"
        );
        assert_eq!(
            format_unified_status(&status, "en-US"),
            "Demo data · No live core"
        );
    }

    #[test]
    fn starting_transition_is_distinct_and_not_running() {
        let runtime = RuntimeStatusSnapshot {
            core_process: CoreProcessObservation::Starting,
            ..RuntimeStatusSnapshot::default()
        };
        let status = project_from_axes(SurfaceOrigin::Live, &runtime);
        assert!(status.is_starting());
        assert!(!status.is_success());
        assert_eq!(format_unified_status(&status, "zh-CN"), "启动中...");
        assert_eq!(format_unified_status(&status, "en-US"), "Starting...");
    }

    #[test]
    fn success_state_strictly_retains_fact_identities() {
        let runtime = RuntimeStatusSnapshot {
            core_process: CoreProcessObservation::Running,
            controller_api: ControllerApiObservation::Connected {
                core_generation: 42,
                core_revision: 7,
            },
            vpn_authorization: VpnAuthorizationObservation::Granted,
            tun_data_plane: TunDataPlaneObservation::Ready {
                generation: VpnGeneration::new(108),
            },
            proxy_effectiveness: ProxyEffectivenessObservation::ProxiedTrafficObserved {
                core_generation: 42,
                outbound_id: OutboundId::new(EngineId::mihomo(), "proxy-hk").unwrap(),
                active_connections: 5,
            },
            ..RuntimeStatusSnapshot::default()
        };
        let status = project_from_axes(SurfaceOrigin::Live, &runtime);
        assert_eq!(
            status,
            UnifiedRuntimeStatus::Success {
                core_generation: 42,
                vpn_generation: 108,
                outbound_id: Some(OutboundId::new(EngineId::mihomo(), "proxy-hk").unwrap()),
            }
        );
        assert!(status.is_success());
        assert_eq!(format_unified_status(&status, "zh-CN"), "运行中");
        assert_eq!(format_unified_status(&status, "en-US"), "Running");
    }

    #[test]
    fn permission_denied_projects_consistently() {
        let runtime = RuntimeStatusSnapshot {
            vpn_authorization: VpnAuthorizationObservation::Required,
            ..RuntimeStatusSnapshot::default()
        };
        let status = project_from_axes(SurfaceOrigin::Live, &runtime);
        assert!(matches!(
            status,
            UnifiedRuntimeStatus::PermissionDenied { .. }
        ));
        assert!(!status.is_success());
        assert_eq!(format_unified_status(&status, "zh-CN"), "等待授权");
        assert_eq!(
            format_unified_status(&status, "en-US"),
            "Waiting for consent"
        );
    }

    #[test]
    fn configuration_failed_projects_failure_cause() {
        let runtime = RuntimeStatusSnapshot {
            core_process: CoreProcessObservation::Failed {
                failure: Some(Failure::new(
                    ErrorCode::Configuration,
                    "yaml parse error",
                    true,
                )),
            },
            ..RuntimeStatusSnapshot::default()
        };
        let status = project_from_axes(SurfaceOrigin::Live, &runtime);
        assert!(matches!(
            status,
            UnifiedRuntimeStatus::ConfigurationFailed { .. }
        ));
        assert_eq!(format_unified_status(&status, "zh-CN"), "运行出错");
        assert_eq!(format_unified_status(&status, "en-US"), "Runtime Error");
    }

    #[test]
    fn api_disconnected_projects_distinct_variant() {
        let runtime = RuntimeStatusSnapshot {
            core_process: CoreProcessObservation::Running,
            controller_api: ControllerApiObservation::Unavailable {
                failure: Failure::new(ErrorCode::Network, "connection refused", true),
            },
            ..RuntimeStatusSnapshot::default()
        };
        let status = project_from_axes(SurfaceOrigin::Live, &runtime);
        assert!(matches!(
            status,
            UnifiedRuntimeStatus::ApiDisconnected { .. }
        ));
        assert!(!status.is_success());
    }

    #[test]
    fn vpn_revoked_projects_vpn_failed() {
        let runtime = RuntimeStatusSnapshot {
            tun_data_plane: TunDataPlaneObservation::Revoked,
            ..RuntimeStatusSnapshot::default()
        };
        let status = project_from_axes(SurfaceOrigin::Live, &runtime);
        assert!(matches!(status, UnifiedRuntimeStatus::VpnFailed { .. }));
        assert_eq!(format_unified_status(&status, "zh-CN"), "失败");
        assert_eq!(format_unified_status(&status, "en-US"), "Failed");
    }

    #[test]
    fn unknown_initial_state_projects_unknown() {
        let runtime = RuntimeStatusSnapshot::default();
        let status = project_from_axes(SurfaceOrigin::Live, &runtime);
        assert_eq!(status, UnifiedRuntimeStatus::Unknown);
        assert_eq!(format_unified_status(&status, "zh-CN"), "已停止");
        assert_eq!(format_unified_status(&status, "en-US"), "Stopped");
    }
}
