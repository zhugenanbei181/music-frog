//! The demo fixture's runtime baseline: the Core session, the live shell
//! readouts, the traffic waveform, the resource and speedtest observations,
//! and the shared workbench read models every capture paints. Split out of
//! `demo.rs` along the runtime seam so the demo host keeps only window
//! assembly and the snapshot skeleton.

use super::fixtures;
use super::outbound;
use super::public_ip_probe::PublicIpProbeOwner;
use infiltrator_contract::active_exit::ActiveExitSnapshot;
use infiltrator_contract::resources::{CoreGcStatus, CoreResourceSnapshot, ResourceAvailability};
use infiltrator_contract::shell_readout::{ShellObservation, ShellProfile};
use infiltrator_contract::snapshot::CoreSessionObservation;
use infiltrator_contract::speedtest::{
    SpeedtestConcurrency, SpeedtestOutboundResult, SpeedtestResultState, SpeedtestRunState,
    SpeedtestSampleOutcome, SpeedtestSnapshot,
};
use infiltrator_contract::subscription_quota::SubscriptionQuotaSnapshot;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::system_proxy::{SystemProxyObservation, SystemProxySnapshot};
use infiltrator_contract::theme::{SystemAppearance, ThemePreference};
use infiltrator_contract::theme_palette::{ResolvedThemePalette, SystemDynamicColor};
use infiltrator_contract::traffic_waveform::{TrafficSample, TrafficWaveformSnapshot};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::page::{PageData, SurfaceOrigin};
use lurker_core_contract::routing_intent::RoutingIntentSnapshot;
use std::f64::consts::TAU;

/// The fixed instant the demo fixture's Core session began.
///
/// The fixture publishes a measured session length, not a live clock, so a
/// capture run renders the same pixels every time it runs.
const DEMO_SESSION_STARTED_AT_EPOCH_MS: u64 = 1_700_000_000_000;

/// Bytes in one mebibyte, the unit every demo rate is published in.
pub(super) const MB: f64 = 1024.0 * 1024.0;

/// The theme the fixture's hydrated skin selects, as the surface setting token.
pub(super) fn demo_theme_str(skin: &str) -> &'static str {
    match skin {
        "light" => "light",
        "forest" => "forest",
        "amoled" => "amoled",
        _ => "dark",
    }
}

/// Publishes the Core/shell/traffic/outbound baseline the capture reads.
pub(super) fn apply_runtime_baseline(snapshot: &mut SurfaceSnapshot) {
    snapshot.origin = SurfaceOrigin::Demo;
    snapshot.failure = None;
    snapshot.core.failure = None;
    snapshot.core.lifecycle = CoreLifecycle::Running;
    snapshot.core.identity = Some(outbound::demo_core_scope().identity);
    snapshot.core.core_version = Some("v1.19.18".into());
    snapshot.core.active_connections = 8;
    snapshot.core.routing_intent = RoutingIntentSnapshot::demo_fixture();
    // The demo fixture publishes the Core session the owner would publish:
    // a fixed start instant and the session length the owner measured. The
    // length is the reference dashboard's own `03:04:49`, so a receipt shows
    // the running session instead of an unobserved dash - and it stays
    // byte-identical between two capture runs.
    snapshot.core.session = CoreSessionObservation::Active {
        started_at_epoch_ms: DEMO_SESSION_STARTED_AT_EPOCH_MS,
        elapsed_ms: 11_089_000,
    };

    snapshot.shell_readout.upload_bps = ShellObservation::Current(2.5 * MB);
    snapshot.shell_readout.download_bps = ShellObservation::Current(12.8 * MB);
    snapshot.shell_readout.profile = ShellObservation::Current(ShellProfile {
        id: "profile-1".into(),
        name: "Alpha Fast (HK/JP/US)".into(),
        subscription: true,
        used_bytes: Some(8 * 1024 * 1024 * 1024),
        total_bytes: Some(100 * 1024 * 1024 * 1024),
        usage_fraction: Some(0.08),
    });
    // The profile card counts the engine's proxies; the fixture publishes the
    // same readback the live composition would report.
    snapshot.shell_readout.proxies = ShellObservation::Current(4);

    // Cross-surface fixture parity: the Iced and Bevy demo hosts both publish
    // `SubscriptionQuotaSnapshot::demo_fixture()`, so their receipts show a drawable
    // quota. Slint left the field at its default, which meant the Slint screenshots
    // showed a *different product state* than the other two surfaces - and the quota
    // bar, the expiry label and the warning grades never reached a Slint pixel.
    // One fixture, three surfaces.
    snapshot.subscription_quota = SubscriptionQuotaSnapshot::demo_fixture();
    snapshot.system_proxy = SystemProxySnapshot::from_observation(
        1,
        SystemProxyObservation {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: None,
        },
    );

    // One readable slow sweep per direction, like the reference dashboard:
    // a real waveform sample is a non-negative rate, so the old fixture's
    // signed sine produced clipped plateaus instead of a continuous curve.
    let samples: Vec<TrafficSample> = (0..60u32)
        .map(|i| {
            let phase = f64::from(i) / 59.0;
            let up = 0.45 + 0.42 * (phase * TAU - 0.4).sin() + 0.14 * (phase * TAU * 2.7).sin();
            let down =
                0.52 + 0.39 * (phase * TAU - 1.1).sin() + 0.10 * (phase * TAU * 1.8 + 0.6).sin();
            TrafficSample {
                sampled_at_epoch_ms: None,
                upload_bps: up.max(0.0) * 2.3 * MB,
                download_bps: down.max(0.0) * 6.8 * MB,
            }
        })
        .collect();
    snapshot.traffic_waveform = TrafficWaveformSnapshot {
        generation: 1,
        revision: samples.len() as u64,
        samples,
    };
    snapshot.traffic_history.today_upload_bytes = (1.6 * 1024.0 * 1024.0 * 1024.0) as u64;
    snapshot.traffic_history.today_download_bytes = (504.8 * 1024.0 * 1024.0) as u64;
    snapshot.traffic_history.today_has_data = true;

    // Four common-service probes, in the reference dashboard's order. The
    // fixture publishes real typed observations so the health, memory and CPU
    // cards are exercised by the capture instead of rendering dashes.
    snapshot.speedtest = SpeedtestSnapshot {
        revision: 1,
        state: SpeedtestRunState::Idle,
        concurrency: SpeedtestConcurrency::default(),
        results: ["Google", "GitHub", "YouTube", "ChatGPT"]
            .iter()
            .zip([951_u32, 1679, 4277, 1242])
            .map(|(name, delay)| SpeedtestOutboundResult {
                outbound_id: OutboundId::new(
                    EngineId::new("mihomo").expect("valid engine id"),
                    name.to_string(),
                )
                .expect("valid demo outbound id"),
                display_name: Some((*name).to_string()),
                protocol_display_name: None,
                samples: vec![SpeedtestSampleOutcome::Measured { delay_ms: delay }],
                state: SpeedtestResultState::Completed,
                tested_at_epoch_ms: 0,
            })
            .collect(),
        recent_history: Vec::new(),
    };
    snapshot.resources = CoreResourceSnapshot::Observed {
        scope: Box::new(outbound::demo_core_scope()),
        memory: ResourceAvailability::Available(208 * 1024 * 1024),
        cpu_percent: ResourceAvailability::Available(24.6),
        memory_soft_limit_bytes: 512 * 1024 * 1024,
        gc: Box::new(CoreGcStatus::NotAttempted),
    };

    snapshot.active_exit = ActiveExitSnapshot::demo_fixture();

    let (inventory, details, preferences) = outbound::demo_outbound_inventory();
    snapshot.outbound_inventory = PageData::ready(inventory);
    snapshot.outbound_details = PageData::ready(details);
    snapshot.outbound_ui_preferences = PageData::ready(preferences);
}

/// Publishes the theme readback the real composition derives from the
/// hydrated skin setting, so every projection paints one authoritative fact.
pub(super) fn apply_theme_readback(snapshot: &mut SurfaceSnapshot, theme_str: &str) {
    // One theme fact, one source: the fixture publishes the readback the
    // real composition derives from this hydrated setting, so every
    // projection paints the fixture skin from the authoritative readback.
    snapshot.theme_readback = ResolvedThemePalette::resolve(
        ThemePreference::from_setting(theme_str),
        SystemAppearance::Unobserved,
        SystemDynamicColor::Unobserved,
    );
}

/// Publishes the shared workbench read models the capture's panels render.
pub(super) fn apply_workbench_read_models(
    snapshot: &mut SurfaceSnapshot,
    public_ip_probe: &PublicIpProbeOwner,
) {
    // The workbench panels of the capture read the shared read models of the
    // businesses they carry, so a receipt shows the interaction and never an
    // empty panel.
    snapshot.dns_leak = fixtures::dns_leak_report();
    snapshot.dns_cache = fixtures::dns_cache_snapshot();
    snapshot.dns_query = fixtures::dns_query_snapshot();
    snapshot.dns_hosts = PageData::ready(fixtures::dns_hosts_profile());
    snapshot.public_ip = public_ip_probe.peek_snapshot();
    snapshot.pages.doctor = PageData::ready(fixtures::doctor_page());
    snapshot.pages.sync = PageData::ready(fixtures::sync_page());
    snapshot.pages.app_routing = PageData::ready(fixtures::app_routing_page());
    snapshot.script_sandbox = Some(fixtures::script_sandbox());
    snapshot.script_export = Some(fixtures::script_export());
    snapshot.yaml_ast_diff = Some(fixtures::yaml_ast_diff());
}
