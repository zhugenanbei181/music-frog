//! Shared lifecycle and source copy; native products replay the same observed facts.
use crate::copy::TextCatalog;
use crate::copy::interpolate;
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::page::SurfaceOrigin;
use lurker_core_contract::redact::redact_line;

pub fn lifecycle_copy(lifecycle: &CoreLifecycle, catalog: &dyn TextCatalog) -> String {
    let key = match lifecycle {
        CoreLifecycle::Stopped => "status_stopped",
        CoreLifecycle::Starting => "status_starting",
        CoreLifecycle::Ready | CoreLifecycle::Running => "status_running",
        CoreLifecycle::Stopping => "core_control_stopping",
        CoreLifecycle::Failed => "status_error",
    };
    catalog.text(key).into_owned()
}

pub fn failure_copy(
    lifecycle: &CoreLifecycle,
    reason: Option<&str>,
    catalog: &dyn TextCatalog,
) -> String {
    match reason.map(str::trim).filter(|reason| !reason.is_empty()) {
        Some(reason) => redact_line(reason, &[]),
        None if *lifecycle == CoreLifecycle::Failed => {
            catalog.text("core_failure_reason_unobserved").into_owned()
        }
        None => String::new(),
    }
}

pub fn source_copy(
    origin: SurfaceOrigin,
    version: Option<&str>,
    catalog: &dyn TextCatalog,
) -> String {
    if origin == SurfaceOrigin::Demo {
        return catalog.text("core_source_demo").into_owned();
    }
    match version.map(str::trim).filter(|version| !version.is_empty()) {
        Some(version) => interpolate(
            catalog.text("core_source_live_version").as_ref(),
            &[("version", &redact_line(version, &[]))],
        ),
        None => catalog
            .text("core_source_live_unknown_version")
            .into_owned(),
    }
}

/// Typed verdict of the overview banner state across the five canonical states (SHOT-OV-07).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverviewBannerVerdict {
    /// 1. Demo fixture: banner first line shows "演示模式" / "Demo Mode", neutral accent.
    Demo,
    /// 2. Missing host / unobserved / failed: danger fill and dot.
    MissingHost,
    /// 3. Core stopped: Stopped text, ink_dim dot.
    CoreStopped,
    /// 4. VPN / Proxy unauthorized or not taking over: Core is running, but proxy/TUN has not taken over.
    ///    Banner shows "内核运行 · 未接管" (or "VPN 未授权" if unauthorized), warning dot (never green!).
    VpnUnauthorized,
    /// 5. Real running: Core is running AND proxy/VPN has actively taken over.
    ///    Banner shows "运行中" / "Running", green success dot.
    RealRunning,
    /// In-flight transition (Starting / Stopping).
    Transitioning,
}

pub fn project_banner_verdict(
    origin: SurfaceOrigin,
    lifecycle: &CoreLifecycle,
    is_proxy_or_vpn_active: bool,
    is_vpn_unauthorized: bool,
) -> OverviewBannerVerdict {
    match lifecycle {
        CoreLifecycle::Failed => OverviewBannerVerdict::MissingHost,
        CoreLifecycle::Stopped => OverviewBannerVerdict::CoreStopped,
        CoreLifecycle::Starting | CoreLifecycle::Stopping => OverviewBannerVerdict::Transitioning,
        CoreLifecycle::Ready | CoreLifecycle::Running => {
            if origin == SurfaceOrigin::Demo {
                OverviewBannerVerdict::Demo
            } else if is_vpn_unauthorized {
                OverviewBannerVerdict::VpnUnauthorized
            } else if is_proxy_or_vpn_active {
                OverviewBannerVerdict::RealRunning
            } else {
                OverviewBannerVerdict::VpnUnauthorized
            }
        }
    }
}

pub fn banner_verdict_copy(
    verdict: OverviewBannerVerdict,
    lifecycle: &CoreLifecycle,
    catalog: &dyn TextCatalog,
    is_vpn_unauthorized: bool,
) -> String {
    match verdict {
        OverviewBannerVerdict::Demo => catalog.text("overview_demo_mode").into_owned(),
        OverviewBannerVerdict::MissingHost => lifecycle_copy(&CoreLifecycle::Failed, catalog),
        OverviewBannerVerdict::CoreStopped => lifecycle_copy(&CoreLifecycle::Stopped, catalog),
        OverviewBannerVerdict::VpnUnauthorized => {
            if is_vpn_unauthorized {
                catalog.text("overview_vpn_unauthorized").into_owned()
            } else {
                catalog.text("overview_running_untaken").into_owned()
            }
        }
        OverviewBannerVerdict::RealRunning => lifecycle_copy(&CoreLifecycle::Running, catalog),
        OverviewBannerVerdict::Transitioning => lifecycle_copy(lifecycle, catalog),
    }
}

#[cfg(test)]
#[path = "core_status_projection_tests.rs"]
mod tests;
