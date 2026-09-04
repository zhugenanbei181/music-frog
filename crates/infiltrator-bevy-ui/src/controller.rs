//! Bevy adapter for the application-owned Overview pump.
//!
//! The worker, Tokio runtime, Mihomo client, sampling cadence, rate
//! calculation, and bounded transport live in
//! `infiltrator_application::overview`. This module only translates the
//! neutral snapshot into a Bevy-facing projection and drains it once per
//! frame.

use bevy::app::{Plugin, Update};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Res, ResMut};
use infiltrator_application::overview::{OverviewConfig, OverviewPump};
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::error::Failure;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use crate::history::TrafficHistory;
use crate::pages::overview::OverviewProjectionUpdated;
use crate::projection::{
    OverviewOrigin, OverviewProjection, OverviewSource, OverviewState, SourceKind,
};

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
    let controller = std::env::var("INFILTRATOR_BEVY_CONTROLLER").ok();
    let secret = std::env::var("INFILTRATOR_BEVY_SECRET").ok();
    controller_config_from_raw(controller.as_deref(), secret.as_deref())
}

struct SourceShared {
    pump: OverviewPump,
}

/// Live Overview source presented to the Bevy page layer. The source contains
/// no transport type; its only state is an application pump handle.
#[derive(Clone)]
pub struct MihomoOverviewSource {
    shared: Arc<SourceShared>,
}

impl MihomoOverviewSource {
    pub fn spawn(config: ControllerConfig) -> Self {
        let pump = OverviewPump::spawn_mihomo(OverviewConfig {
            endpoint: config.endpoint,
            secret: config.secret,
            sample_interval: config.sample_interval,
        });
        Self {
            shared: Arc::new(SourceShared { pump }),
        }
    }

    fn application_snapshot(&self) -> CoreSnapshot {
        self.shared.pump.current()
    }

    pub fn bridge(&self) -> OverviewPumpBridge {
        OverviewPumpBridge {
            application: self.shared.pump.bridge(),
        }
    }
}

impl OverviewSource for MihomoOverviewSource {
    fn current(&self) -> OverviewProjection {
        projection_from_snapshot(self.application_snapshot())
    }

    fn kind(&self) -> SourceKind {
        SourceKind::LiveCore
    }

    fn set_mode(&self, mode: ProxyMode, ack: Sender<Result<(), String>>) {
        let (application_tx, application_rx) = std::sync::mpsc::channel();
        self.shared.pump.request_mode(mode, application_tx);
        // The application worker owns the async operation. This small bridge
        // only converts its neutral failure into the Bevy page's text receipt.
        std::thread::spawn(move || {
            let result = application_rx
                .recv()
                .map_err(|error| format!("mode command channel closed: {error}"))?
                .map(|_| ())
                .map_err(|failure: Failure| failure.message);
            let _ = ack.send(result);
            Ok::<(), String>(())
        });
    }
}

/// The Bevy-facing receiving bridge. `drain` is the only operation that knows
/// the underlying pump bridge exists.
#[derive(Resource)]
pub struct OverviewPumpBridge {
    application: infiltrator_application::overview::OverviewPumpBridge,
}

impl OverviewPumpBridge {
    fn drain(&self) -> Vec<OverviewProjection> {
        self.application
            .drain()
            .into_iter()
            .map(projection_from_snapshot)
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
    fn build(&self, app: &mut bevy::app::App) {
        app.insert_resource(OverviewPumpBridge {
            application: self.bridge.application.clone(),
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
    if let Some(history) = history.as_deref_mut() {
        history.push(projection.upload_bps, projection.download_bps);
    }
    commands.trigger(OverviewProjectionUpdated(projection));
}

fn projection_from_snapshot(snapshot: CoreSnapshot) -> OverviewProjection {
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
        state,
        mode: snapshot.proxy_mode.unwrap_or_default(),
        upload_bps: snapshot.upload_bps,
        download_bps: snapshot.download_bps,
        active_connections: snapshot.active_connections,
        memory_bytes: snapshot.memory_bytes,
        sampled_at,
        failure: snapshot.failure.map(|failure| failure.message),
        origin: OverviewOrigin::LiveCore,
        core_version: snapshot.core_version,
    }
}

#[cfg(test)]
#[path = "controller_unit_tests.rs"]
mod tests;
