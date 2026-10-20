//! Minimal Lurker CLI example (PUB-006.9).
//!
//! The example shows the two crates every third-party surface needs:
//!
//! * `lurker-core-contract` owns the vocabulary (identity, lifecycle);
//! * `lurker-ui-presentation` owns the projection from an observed fact to a
//!   renderable verdict.
//!
//! The terminal program itself owns no business state: it renders what the
//! neutral crates report and never derives a second truth of its own.
//!
//! Run it with:
//!
//! ```text
//! cargo run --example minimal-cli
//! ```

use lurker_core_contract::identity::{CoreIdentity, CoreInstanceId, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::page::SurfaceOrigin;
use lurker_ui_presentation::core_status_projection::{
    OverviewBannerVerdict, project_banner_verdict,
};

fn main() {
    let engine = EngineId::mihomo();
    let identity = CoreIdentity::new(
        engine.clone(),
        CoreInstanceId::new("minimal-cli").expect("instance identifier is valid"),
        None,
    );
    println!("engine:   {}", engine.as_str());
    println!("instance: {}", identity.instance.as_str());

    // The lifecycle is one typed enum: a surface never keeps a second copy of
    // the runtime state as a flag.
    for lifecycle in [CoreLifecycle::Stopped, CoreLifecycle::Running] {
        let verdict = project_banner_verdict(SurfaceOrigin::Live, &lifecycle, true, false);
        println!("{lifecycle:?} -> {}", verdict_text(&verdict));
    }
}

/// Renders the shared verdict vocabulary of the presentation kernel.
fn verdict_text(verdict: &OverviewBannerVerdict) -> &'static str {
    match verdict {
        OverviewBannerVerdict::Demo => "demo fixture",
        OverviewBannerVerdict::MissingHost => "missing host",
        OverviewBannerVerdict::CoreStopped => "core stopped",
        OverviewBannerVerdict::VpnUnauthorized => "vpn unauthorized",
        OverviewBannerVerdict::RealRunning => "real running",
        OverviewBannerVerdict::Transitioning => "transitioning",
    }
}
