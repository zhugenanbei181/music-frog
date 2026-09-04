//! The live mihomo controller pump (BEVY-005 real data seam).
//! Background thread with current-thread tokio runtime samples core endpoints on a fixed interval,
//! folds them into `OverviewProjection`, and publishes via a bounded channel drained into ECS.

use std::sync::mpsc::{Receiver, Sender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bevy::app::{Plugin, Update};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Res, ResMut};
use mihomo_api::client::MihomoClient;
use mihomo_api::types::{ConnectionsResponse, Version};

use crate::history::TrafficHistory;
use crate::pages::overview::OverviewProjectionUpdated;
use crate::projection::{
    OverviewOrigin, OverviewProjection, OverviewSource, OverviewState, ProxyMode, SourceKind,
};

/// Snapshot channel capacity: the pump never blocks on the UI. A full
/// channel means the frame drain fell behind, so the oldest snapshot is
/// dropped and the newest one wins — rates are deltas, nothing is lost by
/// skipping intermediates.
const PUMP_CHANNEL_CAPACITY: usize = 8;

/// `/memory` and `/configs` are read every Nth tick (~3s at the default
/// interval); `/connections` is read every tick.
const SLOW_POLL_EVERY: u32 = 4;

/// Hard ceiling for one `/memory` streaming read: the endpoint sends its
/// first chunk immediately and never closes the body, so the reader must
/// stop on its own. Kept well under the fallback HTTP client's own
/// per-request timeout.
const MEMORY_STREAM_READ_TIMEOUT: Duration = Duration::from_secs(2);

/// The pump's default sampling interval (the charter's ≤1s budget).
pub const DEFAULT_SAMPLE_INTERVAL: Duration = Duration::from_millis(700);

/// One mode command in flight to the pump thread, carrying the typed
/// receipt channel back to the UI.
pub(crate) enum ModeCommand {
    SetMode(ProxyMode, Sender<Result<(), String>>),
}

/// Where and how the pump talks to the core. `sample_interval` is a
/// construction parameter (not env) so tests can shorten it.
#[derive(Clone, Debug)]
pub struct ControllerConfig {
    /// The controller base URL, e.g. `http://127.0.0.1:9099`.
    pub endpoint: String,
    /// Optional bearer secret (`Authorization: Bearer …`).
    pub secret: Option<String>,
    /// Time between sampling ticks.
    pub sample_interval: Duration,
}

impl ControllerConfig {
    /// The config with the default sampling interval.
    pub fn new(endpoint: impl Into<String>, secret: Option<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            secret,
            sample_interval: DEFAULT_SAMPLE_INTERVAL,
        }
    }
}

/// Parse the controller endpoint env value: whitespace-tolerant, must be
/// an `http(s)://` URL with a non-empty host. Pure function — unit-tested
/// below without any env access.
pub fn parse_controller_endpoint(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let rest = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    (!host.is_empty()).then(|| trimmed.to_owned())
}

/// Resolve the controller config from raw env-shaped inputs. `None` keeps
/// the demo frontend (the capture-matrix default). Pure function —
/// unit-tested below without any env access.
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

/// The controller config from the environment, if
/// `INFILTRATOR_BEVY_CONTROLLER` is set (and parses); the secret rides
/// `INFILTRATOR_BEVY_SECRET`.
pub fn controller_config_from_env() -> Option<ControllerConfig> {
    let controller = std::env::var("INFILTRATOR_BEVY_CONTROLLER").ok();
    let secret = std::env::var("INFILTRATOR_BEVY_SECRET").ok();
    controller_config_from_raw(controller.as_deref(), secret.as_deref())
}

/// State shared between the source handle(s) and — for the projection
/// mirror — the pump thread. Dropping the last source drops `cmd_tx`,
/// which disconnects the pump thread's command receive: that is the whole
/// cancellation story.
struct SourceShared {
    last: Arc<Mutex<OverviewProjection>>,
    cmd_tx: Sender<ModeCommand>,
}

/// The live [`OverviewSource`]: clones share one pump. [`OverviewSource::
/// current`] reads the thread's last snapshot (an honest
/// "awaiting first sample" unavailable projection before it); `set_mode`
/// posts into the command channel. Drop the last clone and the pump
/// thread stops — the receiving ends hold no strong reference back.
#[derive(Clone)]
pub struct MihomoOverviewSource {
    shared: Arc<SourceShared>,
    snapshot_rx: Arc<Mutex<Receiver<OverviewProjection>>>,
}

impl MihomoOverviewSource {
    /// Spawn the pump thread for `config`. Infallible: a controller that
    /// cannot be reached projects [`OverviewState::Unavailable`] with the
    /// typed reason instead of failing construction.
    pub fn spawn(config: ControllerConfig) -> Self {
        let (snapshot_tx, snapshot_rx) = sync_channel::<OverviewProjection>(PUMP_CHANNEL_CAPACITY);
        let snapshot_rx = Arc::new(Mutex::new(snapshot_rx));
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<ModeCommand>();
        let last = Arc::new(Mutex::new(Self::initial_projection()));
        let source = Self {
            shared: Arc::new(SourceShared {
                last: Arc::clone(&last),
                cmd_tx,
            }),
            snapshot_rx: Arc::clone(&snapshot_rx),
        };
        let thread_last = Arc::clone(&last);
        // Path-call thread start (`thread::spawn`, no `.spawn(` method
        // receiver): the bsn! guard bans direct `.spawn(` method receivers
        // crate-wide (BEVY-BSN-003, failing safe on future TaskPool-style
        // spawns), and an OS thread is exactly the infrastructure that law
        // is happy to exempt at the path-call spelling. The unnamed-thread
        // trade (no `Builder::name`) buys guard compliance; a failed spawn
        // panics here instead of being reported through the mirror — the
        // mirror then simply stays on the honest "waiting" projection.
        std::thread::spawn(move || {
            pump_loop(config, cmd_rx, snapshot_tx, snapshot_rx, thread_last);
        });
        source
    }

    /// The projection before the first sample lands: unavailable, but
    /// explicitly *waiting* — not yet a failure verdict.
    fn initial_projection() -> OverviewProjection {
        Self::unavailable_with("等待实时内核首次采样…".to_owned())
    }

    /// A live-origin unavailable projection with the given reason.
    fn unavailable_with(reason: String) -> OverviewProjection {
        OverviewProjection::unavailable(OverviewOrigin::LiveCore, reason)
    }

    /// The bevy-side receiving end of the snapshot channel. Holding this
    /// clone keeps the channel's receiver alive independently of the
    /// source — [`PumpDrainPlugin`] installs it as a resource, and the
    /// cancellation test uses it to prove the pump stops via the command
    /// channel alone.
    pub fn bridge(&self) -> OverviewPumpBridge {
        OverviewPumpBridge {
            snapshot_rx: Arc::clone(&self.snapshot_rx),
        }
    }
}

impl OverviewSource for MihomoOverviewSource {
    fn current(&self) -> OverviewProjection {
        self.shared
            .last
            .lock()
            .expect("pump mirror poisoned")
            .clone()
    }

    fn kind(&self) -> SourceKind {
        SourceKind::LiveCore
    }

    /// Post the mode command to the pump thread. A disconnected channel
    /// (source dropped everywhere while a UI latch still holds a clone —
    /// practically impossible, but the ack drain treats a silent channel
    /// as a failure, so the latch can never wedge).
    fn set_mode(&self, mode: ProxyMode, ack: Sender<Result<(), String>>) {
        let _ = self.shared.cmd_tx.send(ModeCommand::SetMode(mode, ack));
    }
}

/// One-way latch flipped by [`drain_overview_pump`] the first time a live
/// snapshot reaches the UI. The capture marker (see `capture.rs`) gates on
/// it so a live screenshot is only taken after the page actually carries
/// real core data — never the pre-first-sample placeholder.
#[derive(Resource, Debug, Default, PartialEq, Eq)]
pub struct PumpSnapshotSeen(pub bool);

/// The shortest time a failure verdict stays mounted on a live pump:
/// a refused mode switch (or a dropped command channel) must stay visible
/// instead of being washed away by the next ≤3s sampling tick. Recovery
/// waits for **both** — the dwell elapsing *and* a successful sample
/// arriving (whichever is later).
pub const MIN_FAILURE_DWELL: Duration = Duration::from_secs(5);

/// The failure-verdict dwell: once a failure projection becomes visible on
/// a live pump, [`drain_overview_pump`] holds back the routine snapshots
/// that would overwrite it — successful samples are deferred until the
/// dwell has elapsed (a success arriving 200ms into a 5s dwell does *not*
/// clear it; a success arriving after 5s does), while fresh failure
/// snapshots keep the verdict current and refresh the window. Latched by
/// [`drain_overview_pump`] itself (unavailable samples) and by the shell's
/// receipt drain (refused mode commands, via [`FailureDwell::latch`]).
///
/// Demo sources never mount a pump, so the demo failure semantics — hold
/// the verdict until the next refresh — are structurally unchanged.
#[derive(Resource, Clone, Copy, Debug)]
pub struct FailureDwell {
    /// How long a latched verdict defers routine snapshots.
    pub min_dwell: Duration,
    /// When the current verdict became visible; `None` = nothing latched.
    pub latched_at: Option<Instant>,
}

impl FailureDwell {
    /// A dwell with an explicit minimum (tests shorten it; production uses
    /// the [`MIN_FAILURE_DWELL`] default).
    pub fn new(min_dwell: Duration) -> Self {
        Self {
            min_dwell,
            latched_at: None,
        }
    }

    /// Latch (or refresh) the verdict window as of `now`.
    pub fn latch(&mut self, now: Instant) {
        self.latched_at = Some(now);
    }

    /// Whether a *successful* snapshot may pass right now: only once the
    /// latched verdict has dwelled its minimum (an unlatched dwell always
    /// passes). Pure function of the stored state and `now`.
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

/// The bevy resource bridge over the pump's snapshot channel. Drained
/// every frame by [`drain_overview_pump`]; the pump thread is the only
/// producer, so the receive side needs a mutex, not a queue rewrite.
#[derive(Resource)]
pub struct OverviewPumpBridge {
    snapshot_rx: Arc<Mutex<Receiver<OverviewProjection>>>,
}

/// Installs [`OverviewPumpBridge`] and the per-frame drain that turns
/// fresh pump snapshots into [`OverviewProjectionUpdated`] triggers — the
/// page's one and only data-refresh path, unchanged. Add after
/// `PagesPlugin` (any order works; the drain only triggers observers).
pub struct PumpDrainPlugin {
    bridge: OverviewPumpBridge,
}

impl PumpDrainPlugin {
    /// Bridge this source's snapshot channel into the app.
    pub fn new(source: &MihomoOverviewSource) -> Self {
        Self {
            bridge: source.bridge(),
        }
    }
}

impl Plugin for PumpDrainPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.insert_resource(OverviewPumpBridge {
            snapshot_rx: Arc::clone(&self.bridge.snapshot_rx),
        });
        app.init_resource::<PumpSnapshotSeen>();
        app.init_resource::<FailureDwell>();
        // The trend chart's sample ring (shared with PagesPlugin — the
        // drain is its producer, the page's refresh observer its reader).
        app.init_resource::<TrafficHistory>();
        app.add_systems(Update, drain_overview_pump);
    }
}

/// The per-frame drain: coalesce every queued snapshot into one
/// [`OverviewProjectionUpdated`] trigger (the newest wins — the page is a
/// projection display, not a log). A disconnected channel (pump stopped)
/// simply retires.
///
/// The failure dwell gates the trigger: while a failure verdict is within
/// its dwell window, routine (successful) snapshots are deferred — the
/// page keeps showing the verdict the user must see. See [`FailureDwell`].
/// Every *delivered* snapshot is also appended to the trend chart's rate
/// ring ([`TrafficHistory`]) right at the drain — the chart records
/// exactly what the page was shown (dwell-deferred samples are not
/// displayed, so they are not recorded either). `Option` because the
/// drain is exercisable without the page plugin mounted.
fn drain_overview_pump(
    bridge: Res<OverviewPumpBridge>,
    seen: Option<ResMut<PumpSnapshotSeen>>,
    mut dwell: ResMut<FailureDwell>,
    mut history: Option<ResMut<TrafficHistory>>,
    mut commands: Commands,
) {
    let rx = bridge.snapshot_rx.lock().expect("pump channel poisoned");
    let mut newest: Option<OverviewProjection> = None;
    loop {
        match rx.try_recv() {
            Ok(snapshot) => newest = Some(snapshot),
            Err(std::sync::mpsc::TryRecvError::Empty) => break,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
        }
    }
    drop(rx);
    if let Some(snapshot) = newest {
        let now = Instant::now();
        let is_failure = snapshot.state == OverviewState::Unavailable;
        if !is_failure && !dwell.success_may_pass(now) {
            // A successful sample arrived inside the dwell window: it must
            // not wash the verdict away. Drop it (rates are deltas; the
            // next tick resamples) and keep the latch.
            return;
        }
        if is_failure {
            // Still failing: keep the verdict current and the window fresh.
            dwell.latch(now);
        } else {
            // The verdict cleared: this successful sample is the recovery
            // the user was waiting for.
            dwell.latched_at = None;
        }
        // One-way latch for the capture seam: the first delivered snapshot
        // proves the live page carries real data (see `capture.rs`).
        if let Some(mut seen) = seen {
            seen.0 = true;
        }
        // Record the displayed sample in the trend ring before the page is
        // told about it: by the time the refresh observer reads the ring,
        // this snapshot's rates are already the newest entry.
        if let Some(history) = history.as_deref_mut() {
            history.push(snapshot.upload_bps, snapshot.download_bps);
        }
        commands.trigger(OverviewProjectionUpdated(snapshot));
    }
}

// ---- the pump thread --------------------------------------------------------

/// The pump loop: one sample per tick, commands answered between ticks.
/// Every exit path is channel-driven (see the module docs).
fn pump_loop(
    config: ControllerConfig,
    cmd_rx: Receiver<ModeCommand>,
    snapshot_tx: std::sync::mpsc::SyncSender<OverviewProjection>,
    snapshot_rx: Arc<Mutex<Receiver<OverviewProjection>>>,
    last: Arc<Mutex<OverviewProjection>>,
) {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        let projection = unavailable_live("内核运行时初始化失败");
        *last.lock().expect("pump mirror poisoned") = projection;
        return;
    };
    // Client construction is the only fallible step that has nothing to do
    // with the network (URL parse); retry it each tick until it sticks.
    let mut client: Option<MihomoClient> =
        MihomoClient::new(&config.endpoint, config.secret.clone()).ok();

    let started = Instant::now();
    let mut version: Option<String> = None;
    let mut mode: Option<ProxyMode> = None;
    let mut memory_bytes: Option<u64> = None;
    let mut totals: Option<(u64, u64, Instant)> = None;
    let mut tick: u32 = 0;
    // The fallback HTTP reader (see the real-core fallback notes); same
    // reqwest the seam resolves, a per-request timeout of its own. One-shot
    // fallback reads finish long before it; the /memory streaming reader
    // stops earlier still via MEMORY_STREAM_READ_TIMEOUT.
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok();

    // First sample lands immediately; afterwards one sample per tick.
    loop {
        match cmd_rx.recv_timeout(config.sample_interval) {
            Ok(ModeCommand::SetMode(wanted, ack)) => {
                let receipt = apply_mode(&runtime, &client, http.as_ref(), &config, wanted);
                let _ = ack.send(receipt);
                tick = 0; // force the /configs readback on the next sample
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
        }
        let projection = sample_once(
            &runtime,
            &mut client,
            http.as_ref(),
            &config,
            &mut version,
            &mut mode,
            &mut memory_bytes,
            &mut totals,
            started,
            tick,
        );
        *last.lock().expect("pump mirror poisoned") = projection.clone();
        match snapshot_tx.try_send(projection) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                // The frame drain fell behind: drop the oldest snapshot
                // (the receiver end is shared) so a later tick can deliver
                // the newest — the only one the display needs.
                let drained = snapshot_rx
                    .lock()
                    .expect("pump channel poisoned")
                    .try_recv();
                if drained.is_err() {
                    // Disconnected (bridge and source both gone): retire.
                    return;
                }
            }
            Err(TrySendError::Disconnected(_)) => return,
        }
        tick = tick.wrapping_add(1);
    }
}

/// Convenience: a live-origin unavailable projection from a reason string.
fn unavailable_live(reason: impl Into<String>) -> OverviewProjection {
    OverviewProjection::unavailable(OverviewOrigin::LiveCore, reason)
}

/// One sampling round against the controller. Pure in the mapping sense:
/// all HTTP happens on the pump's runtime, all folding is delegated to the
/// unit-tested functions below.
#[allow(clippy::too_many_arguments)]
fn sample_once(
    runtime: &tokio::runtime::Runtime,
    client: &mut Option<MihomoClient>,
    http: Option<&reqwest::Client>,
    config: &ControllerConfig,
    version: &mut Option<String>,
    mode: &mut Option<ProxyMode>,
    memory_bytes: &mut Option<u64>,
    totals: &mut Option<(u64, u64, Instant)>,
    started: Instant,
    tick: u32,
) -> OverviewProjection {
    // (Re)build the client if a previous construction failed.
    if client.is_none() {
        *client = MihomoClient::new(&config.endpoint, config.secret.clone()).ok();
    }
    let Some(client) = client.as_ref() else {
        return unavailable_live(format!("外部控制器地址无效：{}", config.endpoint));
    };

    // Version: read once, retry every tick until it sticks. Primary read
    // rides the mihomo-api seam; the lenient fallback answers real meta
    // cores (gap #1 in the fallback notes below).
    if version.is_none() {
        match runtime.block_on(client.get_version()) {
            Ok(Version { version: found, .. }) => *version = Some(found),
            Err(_) => {
                if let Ok(live) =
                    runtime.block_on(fallback_json::<LiveVersion>(http, config, "/version"))
                {
                    *version = Some(live.version);
                }
            }
        }
    }

    // Connections snapshot: the only per-tick read. Its cumulative totals
    // feed the rate deltas. Primary read rides the mihomo-api seam; the
    // null-tolerant fallback answers the idle-core shape (fallback #2).
    let connections: ConnectionsResponse = match runtime.block_on(client.get_connections()) {
        Ok(snapshot) => snapshot,
        Err(primary_error) => {
            match runtime.block_on(fallback_json::<IdleConnectionsSnapshot>(
                http,
                config,
                "/connections",
            )) {
                Ok(snapshot) => ConnectionsResponse::from(snapshot),
                Err(_) => {
                    *totals = None; // restart the rate window after recovery
                    return unavailable_live(format!("内核连接采样失败：{primary_error}"));
                }
            }
        }
    };
    let now = Instant::now();
    let (upload_bps, download_bps) = rates_from_totals(
        *totals,
        connections.upload_total,
        connections.download_total,
        now,
    );
    *totals = Some((connections.upload_total, connections.download_total, now));

    // Slow lane: memory and mode readback every few ticks (plus an early
    // retry at tick 1, so a transient first-tick failure still lands before
    // the second second of uptime).
    //
    // /memory rides the streaming fallback ONLY: the seam's
    // `MihomoClient::get_memory` `json()`s a body mihomo serves as an
    // endlessly-open chunked stream, so the primary read cannot complete
    // against a real core (it would burn the full client timeout per call).
    // /configs primary decode fails on real cores (the live `tun` object
    // omits `strict-route`, which `TunConfig` requires); the fallback reads
    // just the `mode` field both lanes need.
    if tick <= 1 || tick.is_multiple_of(SLOW_POLL_EVERY) {
        match runtime.block_on(fallback_streaming_json::<LiveMemory>(
            http, config, "/memory",
        )) {
            Ok(memory) => *memory_bytes = Some(memory.in_use),
            Err(_) => {
                // Keep the last known reading — never fabricate a reset.
            }
        }
        let reported = match runtime.block_on(client.get_config()) {
            Ok(configs) => Some(configs.mode),
            Err(_) => runtime
                .block_on(fallback_json::<LiveConfigMode>(http, config, "/configs"))
                .ok()
                .map(|live| live.mode),
        };
        if let Some(found) = reported.as_deref().and_then(ProxyMode::from_wire) {
            *mode = Some(found);
        }
    }

    OverviewProjection {
        state: crate::projection::OverviewState::Running,
        mode: mode.unwrap_or_default(),
        upload_bps,
        download_bps,
        active_connections: active_connection_count(&connections),
        memory_bytes: *memory_bytes,
        sampled_at: now.duration_since(started),
        failure: None,
        origin: OverviewOrigin::LiveCore,
        core_version: version.clone(),
    }
}

// ---- the real-core fallback readers -----------------------------------------
//
// The mihomo-api seam stays primary for /version, /connections and /configs,
// but the pinned v1.19.18 core trips three of its strict decodes (all
// observed verbatim; mihomo-api is outside this slice's change scope):
//
// 1. `/version` answers `{"meta":true,"version":"v1.19.18"}` — no `premium`
//    key, and `Version` (types.rs:20-23) has no serde default for it.
// 2. `/connections` answers `"connections": null` whenever the tracker is
//    empty, and `ConnectionsResponse.connections` (types.rs:160-166) lacks
//    the null tolerance only the WebSocket `ConnectionSnapshot`
//    (types.rs:148-157) carries.
// 3. `/configs` carries the full live `tun` object without a
//    `strict-route` key, which `TunConfig` (types.rs:49-56) requires.
//
// Additionally `/memory` is NOT a one-shot endpoint: mihomo serves it as an
// endlessly-open chunked stream, so `MihomoClient::get_memory`'s `.json()`
// can never complete against a real core — it is read there through the
// first-chunk streaming reader only.
//
// Each fallback DTO decodes exactly the field(s) the pump folds into
// projections; anything else the core adds is ignored.

/// The fallback GET: same endpoint, same bearer secret. `http` is `None`
/// only if the fallback client could not be built at pump start (reqwest
/// build failure — pathological).
async fn send_fallback_get(
    http: Option<&reqwest::Client>,
    config: &ControllerConfig,
    path: &str,
) -> Result<reqwest::Response, String> {
    let http = http.ok_or_else(|| "回读客户端不可用".to_owned())?;
    let url = format!("{}{}", config.endpoint.trim_end_matches('/'), path);
    let mut request = http.get(url);
    if let Some(secret) = &config.secret {
        request = request.bearer_auth(secret);
    }
    let response = request.send().await.map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    Ok(response)
}

/// One-shot JSON read (body has an end).
async fn fallback_json<T: serde::de::DeserializeOwned>(
    http: Option<&reqwest::Client>,
    config: &ControllerConfig,
    path: &str,
) -> Result<T, String> {
    let response = send_fallback_get(http, config, path).await?;
    response
        .json::<T>()
        .await
        .map_err(|error| error.to_string())
}

/// Streaming-endpoint read: parse as soon as the buffered chunks form a
/// complete JSON value, under a hard timeout (the endpoint never closes the
/// body on its own).
async fn fallback_streaming_json<T: serde::de::DeserializeOwned>(
    http: Option<&reqwest::Client>,
    config: &ControllerConfig,
    path: &str,
) -> Result<T, String> {
    let response = send_fallback_get(http, config, path).await?;
    let read = async {
        let mut response = response;
        let mut buffer = Vec::new();
        loop {
            match response.chunk().await.map_err(|error| error.to_string())? {
                Some(chunk) => {
                    buffer.extend_from_slice(&chunk);
                    if let Ok(value) = serde_json::from_slice::<T>(&buffer) {
                        return Ok(value);
                    }
                }
                None => return Err("流在完整响应前关闭".to_owned()),
            }
        }
    };
    tokio::time::timeout(MEMORY_STREAM_READ_TIMEOUT, read)
        .await
        .map_err(|_| "流式读取超时".to_owned())?
}

/// The `/memory` stream's per-update payload; only `inuse` feeds the
/// projection's memory chip.
#[derive(Debug, serde::Deserialize)]
struct LiveMemory {
    #[serde(rename = "inuse", default)]
    in_use: u64,
}

/// The `/configs` shape the mode readback needs — deliberately blind to the
/// `tun`/`sniffer` subtrees that break the seam's strict structs.
#[derive(Debug, serde::Deserialize)]
struct LiveConfigMode {
    mode: String,
}

/// The minimal `/version` shape the pump actually needs.
#[derive(Debug, serde::Deserialize)]
struct LiveVersion {
    version: String,
}

/// The `/connections` payload mihomo serves when its tracker is EMPTY:
/// `"connections": null`. Reuses the seam's `Connection` entries so a
/// populated tracker decodes identically.
#[derive(Debug, serde::Deserialize)]
struct IdleConnectionsSnapshot {
    #[serde(rename = "downloadTotal", default)]
    download_total: u64,
    #[serde(rename = "uploadTotal", default)]
    upload_total: u64,
    #[serde(default, deserialize_with = "null_as_empty_connections")]
    connections: Vec<mihomo_api::types::Connection>,
}

/// `null → empty vec` for the tracker list. Pure function.
fn null_as_empty_connections<'de, D>(
    deserializer: D,
) -> Result<Vec<mihomo_api::types::Connection>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = <Option<Vec<mihomo_api::types::Connection>> as serde::Deserialize>::deserialize(
        deserializer,
    )?;
    Ok(opt.unwrap_or_default())
}

impl From<IdleConnectionsSnapshot> for ConnectionsResponse {
    fn from(snapshot: IdleConnectionsSnapshot) -> Self {
        ConnectionsResponse {
            download_total: snapshot.download_total,
            upload_total: snapshot.upload_total,
            connections: snapshot.connections,
        }
    }
}

/// Execute `PATCH /configs {"mode": …}` on the pump's runtime, then verify
/// by readback: the receipt answers for the core's *actual* state, not the
/// HTTP transport. (`mihomo-api`'s `patch_config` is fire-and-forget — it
/// only errors on transport failures — so a 400 refusal is detected here by
/// the `/configs` readback still reporting the old mode. The readback itself
/// tries the seam's `get_config` first and falls back to the lenient
/// `LiveConfigMode` reader, because the seam's strict `TunConfig` rejects
/// real cores' `/configs` bodies — fallback #3 in the module notes.) A
/// verified switch also refreshes the pump's own mode mirror immediately.
fn apply_mode(
    runtime: &tokio::runtime::Runtime,
    client: &Option<MihomoClient>,
    http: Option<&reqwest::Client>,
    config: &ControllerConfig,
    wanted: ProxyMode,
) -> Result<(), String> {
    let client = client
        .as_ref()
        .ok_or_else(|| "内核客户端未就绪".to_owned())?;
    runtime
        .block_on(client.patch_config(serde_json::json!({
            "mode": wanted.to_wire(),
        })))
        .map_err(|error| format!("模式切换请求失败：{error}"))?;
    let reported = match runtime.block_on(client.get_config()) {
        Ok(configs) => Some(configs.mode),
        Err(_) => runtime
            .block_on(fallback_json::<LiveConfigMode>(http, config, "/configs"))
            .ok()
            .map(|live| live.mode),
    };
    let Some(reported) = reported else {
        return Err("模式回读失败：控制器无响应".to_owned());
    };
    match ProxyMode::from_wire(&reported) {
        Some(actual) if actual == wanted => Ok(()),
        Some(actual) => Err(format!("内核拒绝模式切换：仍为 {}", actual.to_wire())),
        None => Err(format!("模式回读无法解析：{reported}")),
    }
}

// ---- pure mapping functions (headless-testable without any app) -------------

/// Active connections of a `/connections` snapshot. Pure function.
pub(crate) fn active_connection_count(snapshot: &ConnectionsResponse) -> u32 {
    // u32 saturates far beyond any real connection table; no fabricated
    // wraparound on hostile input.
    u32::try_from(snapshot.connections.len()).unwrap_or(u32::MAX)
}

/// Uplink/downlink rates from consecutive cumulative totals. The first
/// snapshot has no window (rates 0.0); a shrinking total (core restart)
/// restarts the window at 0.0 instead of projecting a negative or giant
/// spike. Pure function.
pub(crate) fn rates_from_totals(
    previous: Option<(u64, u64, Instant)>,
    upload_total: u64,
    download_total: u64,
    now: Instant,
) -> (f64, f64) {
    let Some((prev_up, prev_down, prev_at)) = previous else {
        return (0.0, 0.0);
    };
    let elapsed = now.duration_since(prev_at).as_secs_f64();
    if !elapsed.is_finite() || elapsed <= 0.0 {
        return (0.0, 0.0);
    }
    let rate = |prev: u64, total: u64| -> f64 {
        if total < prev {
            0.0 // the core restarted its counters
        } else {
            (total - prev) as f64 / elapsed
        }
    };
    (rate(prev_up, upload_total), rate(prev_down, download_total))
}

#[cfg(test)]
#[path = "controller_unit_tests.rs"]
mod tests;
