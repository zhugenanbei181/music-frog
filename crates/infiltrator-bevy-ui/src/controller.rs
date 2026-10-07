//! Bevy adapter for the application-owned Overview pump.
//!
//! The worker, Tokio runtime, Mihomo client, sampling cadence, rate
//! calculation, and bounded transport live in
//! `infiltrator_application::overview`. This module only translates the
//! neutral snapshot into a Bevy-facing projection and drains it once per
//! frame.

use crate::history::TrafficHistory;
use crate::pages::overview::OverviewProjectionUpdated;
use crate::projection::{
    OverviewOrigin, OverviewProjection, OverviewSource, OverviewState, SourceKind,
};
use crate::surface::surface_demo::snapshot_from_overview;
use crate::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use bevy::app;
use bevy::app::{Plugin, Update};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Res, ResMut};
use infiltrator_application::overview;
use infiltrator_application::overview::{OverviewConfig, OverviewPump};
use infiltrator_application::shell_readout_application::ShellReadoutApplication;
use infiltrator_application::traffic_scale_application::TrafficScaleApplication;
use infiltrator_application::traffic_waveform_application::TrafficWaveformApplication;
use infiltrator_contract::active_exit::ActiveExitSnapshot;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::proxy_mode::{ProxyModeSnapshot, ProxyModeStatus};
use infiltrator_contract::public_ip::PublicIpProbeSnapshot;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_contract::subscription_quota::SubscriptionQuotaSnapshot;
use infiltrator_contract::system_toggle::SystemToggleSnapshot;
use infiltrator_contract::traffic_topology::TrafficTopologySnapshot;
use std::env::var;
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};
use std::thread::spawn;
use std::time::{Duration, Instant};

/// The application pump's default sampling interval (the charter's ≤1s
/// budget). Kept in the Bevy config for capture/test compatibility.
pub const DEFAULT_SAMPLE_INTERVAL: Duration = Duration::from_millis(700);

/// Bevy-side configuration accepted by the capture and launcher surface.
#[derive(Clone, Debug)]
pub struct ControllerConfig {
    pub endpoint: String,
    pub secret: Option<String>,
    pub sample_interval: Duration,
}

impl ControllerConfig {
    pub fn new(endpoint: impl Into<String>, secret: Option<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            secret,
            sample_interval: DEFAULT_SAMPLE_INTERVAL,
        }
    }
}

pub fn parse_controller_endpoint(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let rest = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    (!host.is_empty()).then(|| trimmed.to_owned())
}

pub fn controller_config_from_raw(
    controller: Option<&str>,
    secret: Option<&str>,
) -> Option<ControllerConfig> {
    let endpoint = parse_controller_endpoint(controller?)?;
    let secret = secret
        .map(str::trim)
        .filter(|secret| !secret.is_empty())
        .map(str::to_owned);
    Some(ControllerConfig::new(endpoint, secret))
}

pub fn controller_config_from_env() -> Option<ControllerConfig> {
    let controller = var("INFILTRATOR_BEVY_CONTROLLER").ok();
    let secret = var("INFILTRATOR_BEVY_SECRET").ok();
    controller_config_from_raw(controller.as_deref(), secret.as_deref())
}

struct SourceShared {
    pump: OverviewPump,
    readout: ShellReadoutApplication,
    waveform: TrafficWaveformApplication,
}

/// Live Overview source presented to the Bevy page layer. The source contains
/// no transport type; its only state is an application pump handle.
#[derive(Clone)]
pub struct MihomoOverviewSource {
    shared: Arc<SourceShared>,
}

impl MihomoOverviewSource {
    pub fn spawn(config: ControllerConfig) -> Self {
        let pump = infiltrator_composition::spawn_mihomo_overview(OverviewConfig {
            endpoint: config.endpoint,
            secret: config.secret,
            sample_interval: config.sample_interval,
        });
        Self {
            shared: Arc::new(SourceShared {
                pump,
                readout: Default::default(),
                waveform: Default::default(),
            }),
        }
    }

    fn application_snapshot(&self) -> CoreSnapshot {
        self.shared.pump.current()
    }

    pub fn bridge(&self) -> OverviewPumpBridge {
        OverviewPumpBridge {
            application: self.shared.pump.bridge(),
            readout: self.shared.readout.clone(),
            waveform: self.shared.waveform.clone(),
        }
    }
}

impl OverviewSource for MihomoOverviewSource {
    fn current(&self) -> OverviewProjection {
        projection_from_snapshot(
            self.application_snapshot(),
            &self.shared.readout,
            &self.shared.waveform,
        )
    }

    fn kind(&self) -> SourceKind {
        SourceKind::LiveCore
    }

    fn set_mode(&self, mode: ProxyMode, ack: Sender<Result<ProxyMode, Failure>>) {
        let (application_tx, application_rx) = channel();
        self.shared.pump.request_mode(mode, application_tx);
        // The application worker owns the async operation. This small bridge
        // only converts its neutral failure into the Bevy page's text receipt.
        spawn(move || {
            let result = application_rx
                .recv()
                .map_err(|error| {
                    Failure::new(
                        ErrorCode::NotReady,
                        format!("Mode command channel closed: {error}"),
                        true,
                    )
                })
                .and_then(|result| result);
            let _ = ack.send(result);
        });
    }
}

/// The Bevy-facing receiving bridge. `drain` is the only operation that knows
/// the underlying pump bridge exists.
#[derive(Resource)]
pub struct OverviewPumpBridge {
    application: overview::OverviewPumpBridge,
    readout: ShellReadoutApplication,
    waveform: TrafficWaveformApplication,
}

impl OverviewPumpBridge {
    fn drain(&self) -> Vec<OverviewProjection> {
        self.application
            .drain()
            .into_iter()
            .map(|snapshot| projection_from_snapshot(snapshot, &self.readout, &self.waveform))
            .collect()
    }
}

/// Latch flipped when the first live snapshot reaches the Bevy world.
#[derive(Resource, Debug, Default, PartialEq, Eq)]
pub struct PumpSnapshotSeen(pub bool);

pub const MIN_FAILURE_DWELL: Duration = Duration::from_secs(5);

#[derive(Resource, Clone, Copy, Debug)]
pub struct FailureDwell {
    pub min_dwell: Duration,
    pub latched_at: Option<Instant>,
}

impl FailureDwell {
    pub fn new(min_dwell: Duration) -> Self {
        Self {
            min_dwell,
            latched_at: None,
        }
    }

    pub fn latch(&mut self, now: Instant) {
        self.latched_at = Some(now);
    }

    pub fn success_may_pass(&self, now: Instant) -> bool {
        match self.latched_at {
            Some(latched_at) => now.duration_since(latched_at) >= self.min_dwell,
            None => true,
        }
    }
}

impl Default for FailureDwell {
    fn default() -> Self {
        Self::new(MIN_FAILURE_DWELL)
    }
}

pub struct PumpDrainPlugin {
    bridge: OverviewPumpBridge,
}

impl PumpDrainPlugin {
    pub fn new(source: &MihomoOverviewSource) -> Self {
        Self {
            bridge: source.bridge(),
        }
    }
}

impl Plugin for PumpDrainPlugin {
    fn build(&self, app: &mut app::App) {
        app.insert_resource(OverviewPumpBridge {
            application: self.bridge.application.clone(),
            readout: self.bridge.readout.clone(),
            waveform: self.bridge.waveform.clone(),
        });
        app.init_resource::<PumpSnapshotSeen>();
        app.init_resource::<FailureDwell>();
        app.init_resource::<TrafficHistory>();
        app.add_systems(Update, drain_overview_pump);
    }
}

fn drain_overview_pump(
    bridge: Res<OverviewPumpBridge>,
    seen: Option<ResMut<PumpSnapshotSeen>>,
    mut dwell: ResMut<FailureDwell>,
    mut history: Option<ResMut<TrafficHistory>>,
    latest: Option<Res<LatestSurfaceSnapshot>>,
    mut commands: Commands,
) {
    let newest = bridge.drain().into_iter().last();
    let Some(projection) = newest else {
        return;
    };

    let now = Instant::now();
    let is_failure = projection.state == OverviewState::Unavailable;
    if !is_failure && !dwell.success_may_pass(now) {
        return;
    }
    if is_failure {
        dwell.latch(now);
    } else {
        dwell.latched_at = None;
    }
    if let Some(mut seen) = seen {
        seen.0 = true;
    }
    if let Some(history) = history.as_deref_mut()
        && projection.readout.upload_bps.current
        && projection.readout.download_bps.current
    {
        history.push(projection.upload_bps, projection.download_bps);
    }
    if let Some(latest) = latest {
        let mut snapshot =
            snapshot_from_overview(&projection, projection.origin == OverviewOrigin::Demo);
        snapshot.revision = latest.0.revision + 1;
        commands.trigger(SurfaceSnapshotUpdated(snapshot));
    } else {
        commands.trigger(OverviewProjectionUpdated(projection));
    }
}

fn projection_from_snapshot(
    snapshot: CoreSnapshot,
    readout: &ShellReadoutApplication,
    history: &TrafficWaveformApplication,
) -> OverviewProjection {
    let waveform = history.record(&snapshot);
    let scale = TrafficScaleApplication.compute(&waveform);
    let state = match snapshot.lifecycle {
        CoreLifecycle::Running | CoreLifecycle::Ready => OverviewState::Running,
        CoreLifecycle::Stopped => OverviewState::Stopped,
        CoreLifecycle::Starting | CoreLifecycle::Stopping | CoreLifecycle::Failed => {
            OverviewState::Unavailable
        }
    };
    let sampled_at = snapshot
        .sampled_at_epoch_ms
        .and_then(|value| u64::try_from(value).ok())
        .map(Duration::from_millis)
        .unwrap_or(Duration::ZERO);
    OverviewProjection {
        lifecycle: snapshot.lifecycle.clone(),
        readout: readout.project_core_rates(&snapshot, None),
        state,
        upload_bps: snapshot.upload_bps,
        download_bps: snapshot.download_bps,
        active_connections: snapshot.active_connections,
        memory_bytes: snapshot.memory_bytes,
        sampled_at,
        failure: snapshot.failure.map(|failure| failure.message),
        origin: OverviewOrigin::LiveCore,
        core_version: snapshot.core_version,
        traffic_waveform: waveform,
        traffic_scale: scale,
        traffic_topology: TrafficTopologySnapshot::unsupported(
            snapshot.generation,
            snapshot.revision.max(1),
            "overview-only controller source does not include topology facts",
        ),
        layout: Default::default(),
        reconnect_mask: Default::default(),
        viewport: Default::default(),
        public_ip: PublicIpProbeSnapshot::unsupported(
            snapshot.generation,
            snapshot.revision.max(1),
            "overview-only controller source does not include public IP probe facts",
        ),
        active_exit: ActiveExitSnapshot::unsupported(
            snapshot.generation,
            snapshot.revision.max(1),
            "overview-only controller source does not include proxy facts",
        ),
        subscription_quota: SubscriptionQuotaSnapshot::unsupported(
            snapshot.generation,
            snapshot.revision.max(1),
            "overview-only controller source does not include profile quota facts",
        ),
        system_toggles: SystemToggleSnapshot::from_legacy(false, None, snapshot.revision.max(1)),
        proxy_mode: ProxyModeSnapshot {
            current: snapshot.proxy_mode,
            script_available: None,
            status: if snapshot.proxy_mode.is_some() {
                ProxyModeStatus::Ready
            } else {
                ProxyModeStatus::Unobserved
            },
            failure: None,
        },
        speedtest: Default::default(),
        cpu_percent: None,
        total_traffic_bytes: None,
    }
}

#[cfg(test)]
#[path = "controller_unit_tests.rs"]
mod tests;
