#[path = "gateway_migration.rs"]
mod gateway_migration;
#[path = "interface_diff.rs"]
pub mod interface_diff;

use self::interface_diff::InterfaceDiffDetector;

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sysinfo::Networks;
use tokio::sync::{Mutex, broadcast, watch};
use tokio::time::{self, Instant};

/// Physical or virtual classification of a network interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InterfaceType {
    Ethernet,
    WiFi,
    Cellular,
    Tun,
    Loopback,
    Bridge,
    Other,
}

impl InterfaceType {
    /// Infers the interface category by matching typical OS interface naming patterns.
    pub fn infer_from_name(name: &str) -> Self {
        let lower = name.to_ascii_lowercase();
        if lower.starts_with("lo") || lower.contains("loopback") {
            Self::Loopback
        } else if lower.starts_with("tun")
            || lower.starts_with("utun")
            || lower.starts_with("wintun")
            || lower.starts_with("meta")
            || lower.starts_with("clash")
            || lower.starts_with("sing")
            || lower.contains("tap")
        {
            Self::Tun
        } else if lower.starts_with("wl")
            || lower.starts_with("wifi")
            || lower.starts_with("wlan")
            || lower.contains("wi-fi")
            || lower.contains("wireless")
        {
            Self::WiFi
        } else if lower.starts_with("wwan")
            || lower.starts_with("rmnet")
            || lower.starts_with("cdc-wdm")
            || lower.starts_with("cellular")
            || lower.contains("mobile")
            || lower.starts_with("pdp_ip")
            || lower.starts_with("ccmni")
        {
            Self::Cellular
        } else if lower.starts_with("eth")
            || lower.starts_with("en")
            || lower.starts_with("lan")
            || lower.contains("ethernet")
        {
            Self::Ethernet
        } else if lower.starts_with("br") || lower.starts_with("bridge") {
            Self::Bridge
        } else {
            Self::Other
        }
    }

    /// Base priority metric (lower value = higher preference in multi-homing routing).
    pub fn default_priority_metric(&self) -> u32 {
        match self {
            Self::Ethernet => 100,
            Self::WiFi => 200,
            Self::Cellular => 300,
            Self::Bridge => 350,
            Self::Other => 400,
            Self::Tun => 500,
            Self::Loopback => 999,
        }
    }

    /// Returns standard default MTU for the interface category.
    pub fn standard_mtu(&self) -> u32 {
        match self {
            Self::Ethernet => 1500,
            Self::WiFi => 1500,
            Self::Cellular => 1420, // 4G/5G mobile networks typically require 1420-1440
            Self::Tun => 9000,
            Self::Bridge => 1500,
            Self::Loopback => 65535,
            Self::Other => 1500,
        }
    }

    pub fn is_tun(&self) -> bool {
        matches!(self, Self::Tun)
    }

    pub fn is_wireless(&self) -> bool {
        matches!(self, Self::WiFi | Self::Cellular)
    }

    pub fn is_cellular(&self) -> bool {
        matches!(self, Self::Cellular)
    }

    pub fn is_physical(&self) -> bool {
        matches!(self, Self::Ethernet | Self::WiFi | Self::Cellular)
    }
}

/// Snapshot of a network interface at a point in time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkInterfaceSnapshot {
    pub name: String,
    pub is_up: bool,
    pub ip_addresses: Vec<String>,
    pub is_default_gateway: bool,
    #[serde(default)]
    pub gateway_ip: Option<String>,
    #[serde(default)]
    pub is_loopback: bool,
    #[serde(default)]
    pub interface_type: Option<InterfaceType>,
    #[serde(default)]
    pub mtu: Option<u32>,
    #[serde(default)]
    pub metric: Option<u32>,
    #[serde(default)]
    pub dns_servers: Vec<String>,
}

impl NetworkInterfaceSnapshot {
    pub fn new(
        name: impl Into<String>,
        is_up: bool,
        is_default_gateway: bool,
        ip_addresses: Vec<String>,
    ) -> Self {
        let name_str = name.into();
        let inferred_type = InterfaceType::infer_from_name(&name_str);
        Self {
            name: name_str,
            is_up,
            ip_addresses,
            is_default_gateway,
            gateway_ip: None,
            is_loopback: inferred_type == InterfaceType::Loopback,
            interface_type: Some(inferred_type),
            mtu: None,
            metric: None,
            dns_servers: Vec::new(),
        }
    }

    pub fn with_gateway_ip(mut self, ip: impl Into<String>) -> Self {
        self.gateway_ip = Some(ip.into());
        self
    }

    pub fn with_loopback(mut self, is_loopback: bool) -> Self {
        self.is_loopback = is_loopback;
        self
    }

    pub fn with_interface_type(mut self, iface_type: InterfaceType) -> Self {
        self.interface_type = Some(iface_type);
        self
    }

    pub fn with_mtu(mut self, mtu: u32) -> Self {
        self.mtu = Some(mtu);
        self
    }

    pub fn with_metric(mut self, metric: u32) -> Self {
        self.metric = Some(metric);
        self
    }

    pub fn with_dns_servers(mut self, dns_servers: Vec<String>) -> Self {
        self.dns_servers = dns_servers;
        self
    }

    pub fn inferred_type(&self) -> InterfaceType {
        self.interface_type
            .unwrap_or_else(|| InterfaceType::infer_from_name(&self.name))
    }

    pub fn effective_mtu(&self) -> u32 {
        self.mtu
            .unwrap_or_else(|| self.inferred_type().standard_mtu())
    }
}

/// Action recommended by the gateway migration detector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GatewayMigrationAction {
    None,
    UpdateTunRoutes {
        old_gateway_iface: Option<String>,
        new_gateway_iface: String,
        new_gateway_ip: Option<String>,
    },
    PreventRoutingLoop {
        tun_interface: String,
        fallback_interface: Option<String>,
        reason: String,
    },
    DeadTunMitigation {
        tun_interface: String,
        reason: String,
    },
    RebindGateway {
        interface: String,
        gateway_ip: Option<String>,
    },
    PurgeStaleConnections {
        old_gateway_iface: Option<String>,
        new_gateway_iface: String,
        reason: String,
    },
    ClampMtu {
        interface: String,
        recommended_mtu: u32,
        recommended_mss: u32,
    },
    FlushDnsCache {
        reason: String,
    },
}

/// Event describing a detected gateway migration and required actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayMigrationEvent {
    pub old_gateway_interface: Option<String>,
    pub new_gateway_interface: Option<String>,
    pub old_gateway_ip: Option<String>,
    pub new_gateway_ip: Option<String>,
    pub action_required: GatewayMigrationAction,
}

/// Network interface and routing events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkEvent {
    InterfaceUp(String),
    InterfaceDown(String),
    DefaultGatewayChanged {
        old: Option<String>,
        new: Option<String>,
    },
    IpAddressChanged {
        iface: String,
        new_ips: Vec<String>,
    },
    GatewayMigration(Box<GatewayMigrationEvent>),
    RoutingLoopRiskDetected {
        tun_interface: String,
        gateway_interface: String,
        details: String,
    },
    MtuClampingSuggested {
        interface: String,
        recommended_mtu: u32,
        recommended_mss: u32,
    },
    InterfaceFlapDetected {
        interface: String,
        flaps: u32,
    },
}

/// Type alias for backward compatibility.
pub type InterfaceChangeEvent = NetworkEvent;

/// Result of gateway hot-plug arbitration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayArbitrationDecision {
    pub selected_interface: Option<String>,
    pub selected_gateway_ip: Option<String>,
    pub interface_type: Option<InterfaceType>,
    pub fallback_interfaces: Vec<String>,
    pub action: GatewayMigrationAction,
    pub reason: String,
}

/// Arbitrates default gateway selection during interface hot-plug, roaming, and multi-homing failover.
#[derive(Debug, Clone, Default)]
pub struct GatewayHotplugArbiter;

impl GatewayHotplugArbiter {
    /// Evaluates active candidate interfaces and produces an arbitration decision.
    pub fn arbitrate(
        snapshots: &[NetworkInterfaceSnapshot],
        previous_gateway: Option<&str>,
        tun_interface: Option<&str>,
    ) -> GatewayArbitrationDecision {
        let candidates = Self::rank_candidates(snapshots);

        if candidates.is_empty() {
            return GatewayArbitrationDecision {
                selected_interface: None,
                selected_gateway_ip: None,
                interface_type: None,
                fallback_interfaces: Vec::new(),
                action: if let Some(tun) = tun_interface {
                    GatewayMigrationAction::DeadTunMitigation {
                        tun_interface: tun.to_string(),
                        reason: "All physical interfaces are down; no physical route to Internet"
                            .to_string(),
                    }
                } else {
                    GatewayMigrationAction::None
                },
                reason: "No physical interfaces available".to_string(),
            };
        }

        let best = &candidates[0];
        let fallback_names: Vec<String> =
            candidates.iter().skip(1).map(|c| c.name.clone()).collect();

        // Check for TUN routing loop
        if best.inferred_type().is_tun()
            || tun_interface.is_some_and(|t| best.name.eq_ignore_ascii_case(t))
        {
            let physical_fallback = fallback_names.first().cloned();
            return GatewayArbitrationDecision {
                selected_interface: Some(best.name.clone()),
                selected_gateway_ip: best.gateway_ip.clone(),
                interface_type: Some(best.inferred_type()),
                fallback_interfaces: fallback_names,
                action: GatewayMigrationAction::PreventRoutingLoop {
                    tun_interface: best.name.clone(),
                    fallback_interface: physical_fallback,
                    reason: "TUN interface selected as physical default route; routing loop prevention required".to_string(),
                },
                reason: "Routing loop risk on TUN interface".to_string(),
            };
        }

        let is_migrated =
            previous_gateway.is_none() || previous_gateway.is_some_and(|prev| prev != best.name);

        let action = if is_migrated {
            if let Some(_tun) = tun_interface {
                GatewayMigrationAction::UpdateTunRoutes {
                    old_gateway_iface: previous_gateway.map(|s| s.to_string()),
                    new_gateway_iface: best.name.clone(),
                    new_gateway_ip: best.gateway_ip.clone(),
                }
            } else {
                GatewayMigrationAction::RebindGateway {
                    interface: best.name.clone(),
                    gateway_ip: best.gateway_ip.clone(),
                }
            }
        } else {
            GatewayMigrationAction::None
        };

        GatewayArbitrationDecision {
            selected_interface: Some(best.name.clone()),
            selected_gateway_ip: best.gateway_ip.clone(),
            interface_type: Some(best.inferred_type()),
            fallback_interfaces: fallback_names,
            action,
            reason: format!(
                "Selected best candidate '{}' with metric {}",
                best.name,
                best.metric
                    .unwrap_or_else(|| best.inferred_type().default_priority_metric())
            ),
        }
    }

    /// Filters and ranks valid candidate network interfaces (lowest metric first).
    pub fn rank_candidates(
        snapshots: &[NetworkInterfaceSnapshot],
    ) -> Vec<NetworkInterfaceSnapshot> {
        let mut candidates: Vec<NetworkInterfaceSnapshot> = snapshots
            .iter()
            .filter(|i| {
                i.is_up
                    && !i.is_loopback
                    && !i.inferred_type().is_tun()
                    && !i.ip_addresses.is_empty()
            })
            .cloned()
            .collect();

        candidates.sort_by(|a, b| {
            let prio_a = a
                .metric
                .unwrap_or_else(|| a.inferred_type().default_priority_metric());
            let prio_b = b
                .metric
                .unwrap_or_else(|| b.inferred_type().default_priority_metric());
            prio_a.cmp(&prio_b).then_with(|| a.name.cmp(&b.name))
        });

        candidates
    }

    /// Selects the best candidate snapshot.
    pub fn select_best_candidate(
        snapshots: &[NetworkInterfaceSnapshot],
    ) -> Option<&NetworkInterfaceSnapshot> {
        GatewayPriorityArbiter::select_best_candidate(snapshots)
    }

    /// Computes optimal TUN MTU and MSS from physical MTU.
    pub fn calculate_optimal_mtu(physical_mtu: u32) -> (u32, u32) {
        GatewayPriorityArbiter::calculate_optimal_mtu(physical_mtu)
    }
}

/// Ranks candidate network interfaces during roaming and multi-homing failover.
#[derive(Debug, Clone, Default)]
pub struct GatewayPriorityArbiter;

impl GatewayPriorityArbiter {
    /// Selects the best physical candidate among active network interfaces based on priority metrics.
    pub fn select_best_candidate(
        snapshots: &[NetworkInterfaceSnapshot],
    ) -> Option<&NetworkInterfaceSnapshot> {
        snapshots
            .iter()
            .filter(|i| {
                i.is_up
                    && !i.is_loopback
                    && !i.inferred_type().is_tun()
                    && !i.ip_addresses.is_empty()
            })
            .min_by_key(|i| {
                let prio = i
                    .metric
                    .unwrap_or_else(|| i.inferred_type().default_priority_metric());
                (prio, i.name.clone())
            })
    }

    /// Calculates recommended MTU and TCP MSS for TUN routing given the underlying physical link.
    pub fn calculate_optimal_mtu(physical_mtu: u32) -> (u32, u32) {
        let tun_mtu = physical_mtu.saturating_sub(80).max(1280);
        let mss = tun_mtu.saturating_sub(40);
        (tun_mtu, mss)
    }

    /// Compares preference between two interface snapshots (lower metric = preferred).
    pub fn compare_interface_preference(
        a: &NetworkInterfaceSnapshot,
        b: &NetworkInterfaceSnapshot,
    ) -> std::cmp::Ordering {
        let prio_a = a
            .metric
            .unwrap_or_else(|| a.inferred_type().default_priority_metric());
        let prio_b = b
            .metric
            .unwrap_or_else(|| b.inferred_type().default_priority_metric());
        prio_a.cmp(&prio_b)
    }
}

/// 1000ms Debouncer for interface hot-plugging, DHCP IP assignment, and AP roaming transitions.
/// Accumulates intermediate flap events across a 1000ms stabilization window.
#[derive(Debug, Clone)]
pub struct HotplugDebouncer {
    pub debounce_duration: Duration,
    pending_snapshots: Option<Vec<NetworkInterfaceSnapshot>>,
    pending_since: Option<Instant>,
    last_settled: Vec<NetworkInterfaceSnapshot>,
}

impl Default for HotplugDebouncer {
    fn default() -> Self {
        Self::new()
    }
}

impl HotplugDebouncer {
    /// Default debounce duration (1000ms).
    pub const DEFAULT_HOTPLUG_DEBOUNCE: Duration = Duration::from_millis(1000);

    pub fn new() -> Self {
        Self::with_duration(Self::DEFAULT_HOTPLUG_DEBOUNCE)
    }

    pub fn with_duration(debounce_duration: Duration) -> Self {
        Self {
            debounce_duration,
            pending_snapshots: None,
            pending_since: None,
            last_settled: Vec::new(),
        }
    }

    /// Ingests a new snapshot update.
    /// If changes are detected compared to `last_settled`, starts/resets the 1000ms debounce timer.
    /// Returns `Some(settled)` if debounce duration is 0 or if pending state has settled, or `None` while accumulating.
    pub fn ingest(
        &mut self,
        snapshots: Vec<NetworkInterfaceSnapshot>,
        now: Instant,
    ) -> Option<Vec<NetworkInterfaceSnapshot>> {
        if self.debounce_duration.is_zero() {
            self.last_settled = snapshots.clone();
            self.pending_snapshots = None;
            self.pending_since = None;
            return Some(snapshots);
        }

        if snapshots == self.last_settled {
            self.pending_snapshots = None;
            self.pending_since = None;
            return None;
        }

        if let Some(ref pending) = self.pending_snapshots
            && *pending == snapshots
        {
            return self.poll_settled(now);
        }

        self.pending_snapshots = Some(snapshots);
        self.pending_since = Some(now);
        None
    }

    /// Polls whether the pending snapshots have stabilized for >= `debounce_duration`.
    /// Returns `Some(settled_snapshots)` once 1000ms has elapsed.
    pub fn poll_settled(&mut self, now: Instant) -> Option<Vec<NetworkInterfaceSnapshot>> {
        if let (Some(pending), Some(since)) = (&self.pending_snapshots, self.pending_since)
            && now.duration_since(since) >= self.debounce_duration
        {
            let settled = pending.clone();
            self.last_settled = settled.clone();
            self.pending_snapshots = None;
            self.pending_since = None;
            return Some(settled);
        }
        None
    }

    /// Checks if a debounce window is currently active.
    pub fn is_debouncing(&self) -> bool {
        self.pending_snapshots.is_some()
    }

    /// Remaining milliseconds in current debounce window.
    pub fn remaining_debounce_ms(&self, now: Instant) -> Option<u64> {
        if let Some(since) = self.pending_since {
            let elapsed = now.duration_since(since);
            if elapsed < self.debounce_duration {
                Some((self.debounce_duration - elapsed).as_millis() as u64)
            } else {
                Some(0)
            }
        } else {
            None
        }
    }

    /// Flushes pending snapshot immediately without waiting for debounce timeout.
    pub fn force_flush(&mut self) -> Option<Vec<NetworkInterfaceSnapshot>> {
        if let Some(pending) = self.pending_snapshots.take() {
            self.last_settled = pending.clone();
            self.pending_since = None;
            Some(pending)
        } else {
            None
        }
    }

    /// Resets the debouncer state.
    pub fn reset(&mut self) {
        self.pending_snapshots = None;
        self.pending_since = None;
        self.last_settled.clear();
    }
}

/// Protects against rapid Wi-Fi/Cellular AP flapping during signal degradation.
#[derive(Debug, Clone)]
pub struct InterfaceFlapGuard {
    flap_window: Duration,
    max_flaps_per_window: u32,
    flap_history: Vec<(Instant, String)>,
    hold_until: Option<Instant>,
}

impl Default for InterfaceFlapGuard {
    fn default() -> Self {
        Self::new(Duration::from_secs(10), 3)
    }
}

impl InterfaceFlapGuard {
    pub fn new(flap_window: Duration, max_flaps_per_window: u32) -> Self {
        Self {
            flap_window,
            max_flaps_per_window,
            flap_history: Vec::new(),
            hold_until: None,
        }
    }

    /// Records an interface change and returns whether flap threshold is exceeded.
    pub fn record_change(&mut self, iface: &str) -> bool {
        let now = Instant::now();
        self.flap_history
            .retain(|(t, _)| now.duration_since(*t) < self.flap_window);
        self.flap_history.push((now, iface.to_string()));

        let count = self
            .flap_history
            .iter()
            .filter(|(_, name)| name == iface)
            .count() as u32;
        if count >= self.max_flaps_per_window {
            self.hold_until = Some(now + self.flap_window);
            true
        } else {
            false
        }
    }

    /// Checks if the guard is currently holding off rapid changes.
    pub fn is_suppressed(&self) -> bool {
        if let Some(hold) = self.hold_until {
            Instant::now() < hold
        } else {
            false
        }
    }

    /// Clears flap suppression history.
    pub fn reset(&mut self) {
        self.flap_history.clear();
        self.hold_until = None;
    }
}

/// Detects gateway migrations, routing loop risks, and dead TUN interface conditions.
#[derive(Debug, Clone, Default)]
pub struct GatewayMigrationDetector {
    tun_interface_name: Option<String>,
    current_physical_gateway: Option<String>,
    current_gateway_ip: Option<String>,
    current_physical_mtu: Option<u32>,
    is_tun_active: bool,
}

/// Watches network interfaces by polling system interfaces and emitting diff events.
pub struct NetworkInterfaceWatcher {
    sender: broadcast::Sender<NetworkEvent>,
    stop_tx: watch::Sender<bool>,
    stop_rx: watch::Receiver<bool>,
    detector: Arc<Mutex<GatewayMigrationDetector>>,
    last_snapshots: Arc<Mutex<Vec<NetworkInterfaceSnapshot>>>,
    poll_interval: Duration,
    flap_guard: Arc<Mutex<InterfaceFlapGuard>>,
    debouncer: Arc<Mutex<HotplugDebouncer>>,
}

impl Default for NetworkInterfaceWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkInterfaceWatcher {
    /// Creates a new NetworkInterfaceWatcher with default 2000ms polling interval and 1000ms debounce.
    pub fn new() -> Self {
        Self::with_config(None, Duration::from_millis(2000))
    }

    /// Creates a watcher with a designated TUN interface name.
    pub fn with_tun_name(tun_name: &str) -> Self {
        Self::with_config(Some(tun_name.to_string()), Duration::from_millis(2000))
    }

    /// Creates a watcher with custom TUN interface name and polling interval.
    pub fn with_config(tun_interface_name: Option<String>, poll_interval: Duration) -> Self {
        Self::with_config_and_debounce(
            tun_interface_name,
            poll_interval,
            HotplugDebouncer::DEFAULT_HOTPLUG_DEBOUNCE,
        )
    }

    /// Creates a watcher with custom TUN interface name, polling interval, and hot-plug debounce duration.
    pub fn with_config_and_debounce(
        tun_interface_name: Option<String>,
        poll_interval: Duration,
        debounce_duration: Duration,
    ) -> Self {
        let (sender, _) = broadcast::channel(32);
        let (stop_tx, stop_rx) = watch::channel(false);
        let detector = Arc::new(Mutex::new(GatewayMigrationDetector::new(
            tun_interface_name,
        )));
        let last_snapshots = Arc::new(Mutex::new(Vec::new()));
        let flap_guard = Arc::new(Mutex::new(InterfaceFlapGuard::default()));
        let debouncer = Arc::new(Mutex::new(HotplugDebouncer::with_duration(
            debounce_duration,
        )));

        Self {
            sender,
            stop_tx,
            stop_rx,
            detector,
            last_snapshots,
            poll_interval,
            flap_guard,
            debouncer,
        }
    }

    /// Configures the hot-plug debounce window.
    pub fn with_debounce(self, debounce: Duration) -> Self {
        let debouncer = Arc::new(Mutex::new(HotplugDebouncer::with_duration(debounce)));
        Self { debouncer, ..self }
    }

    /// Returns the hotplug debouncer mutex handle.
    pub fn debouncer(&self) -> Arc<Mutex<HotplugDebouncer>> {
        self.debouncer.clone()
    }

    /// Queries currently available network interfaces on the system using sysinfo.
    pub fn poll_interfaces() -> Vec<NetworkInterfaceSnapshot> {
        let networks = Networks::new_with_refreshed_list();
        let mut snapshots = Vec::new();

        for (name, network_data) in &networks {
            let is_loopback = name.eq_ignore_ascii_case("lo")
                || name.eq_ignore_ascii_case("loopback")
                || name.starts_with("lo");

            let ip_addresses: Vec<String> = network_data
                .ip_networks()
                .iter()
                .map(|ip_net| ip_net.addr.to_string())
                .collect();

            let is_up = !ip_addresses.is_empty();

            snapshots.push(
                NetworkInterfaceSnapshot::new(name.clone(), is_up, false, ip_addresses)
                    .with_loopback(is_loopback),
            );
        }

        snapshots
    }

    /// Starts the interface watcher background task.
    pub fn start(&self) -> broadcast::Receiver<NetworkEvent> {
        let rx = self.sender.subscribe();
        let sender = self.sender.clone();
        let mut stop_rx = self.stop_rx.clone();
        let detector = self.detector.clone();
        let last_snapshots = self.last_snapshots.clone();
        let poll_interval = self.poll_interval;
        let flap_guard = self.flap_guard.clone();
        let debouncer = self.debouncer.clone();

        tokio::spawn(async move {
            let mut interval = time::interval(poll_interval);
            interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let current = Self::poll_interfaces();
                        let now = Instant::now();
                        let mut prev_guard = last_snapshots.lock().await;
                        let mut detector_guard = detector.lock().await;
                        let mut guard = flap_guard.lock().await;
                        let mut deb_guard = debouncer.lock().await;

                        // Ingest into 1000ms debouncer
                        let settled = deb_guard.ingest(current.clone(), now)
                            .or_else(|| deb_guard.poll_settled(now));

                        if let Some(settled_snapshots) = settled {
                            if !prev_guard.is_empty() && !guard.is_suppressed() {
                                let events = InterfaceDiffDetector::compute_diff_with_detector(
                                    &prev_guard,
                                    &settled_snapshots,
                                    &mut detector_guard,
                                );

                                for ev in events {
                                    if let NetworkEvent::InterfaceUp(ref name) | NetworkEvent::InterfaceDown(ref name) = ev
                                        && guard.record_change(name)
                                    {
                                        let _ = sender.send(NetworkEvent::InterfaceFlapDetected {
                                            interface: name.clone(),
                                            flaps: 3,
                                        });
                                    }
                                    let _ = sender.send(ev);
                                }
                            }
                            *prev_guard = settled_snapshots;
                        } else if prev_guard.is_empty() {
                            *prev_guard = current;
                        }
                    }
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });

        rx
    }

    /// Emits a network event manually.
    pub fn emit(
        &self,
        event: NetworkEvent,
    ) -> Result<usize, broadcast::error::SendError<NetworkEvent>> {
        self.sender.send(event)
    }

    /// Subscribes to network events.
    pub fn subscribe(&self) -> broadcast::Receiver<NetworkEvent> {
        self.sender.subscribe()
    }

    /// Ingests a new set of snapshots, computing diffs and emitting any detected events.
    pub async fn update_snapshots(
        &self,
        new_snapshots: Vec<NetworkInterfaceSnapshot>,
    ) -> Vec<NetworkEvent> {
        let mut prev_guard = self.last_snapshots.lock().await;
        let mut detector_guard = self.detector.lock().await;

        let events = InterfaceDiffDetector::compute_diff_with_detector(
            &prev_guard,
            &new_snapshots,
            &mut detector_guard,
        );

        for ev in &events {
            let _ = self.sender.send(ev.clone());
        }

        *prev_guard = new_snapshots;
        events
    }

    /// Ingests a new set of snapshots through the 1000ms debouncer.
    /// Returns emitted events if settled, or empty vec if currently debouncing.
    pub async fn update_snapshots_debounced(
        &self,
        new_snapshots: Vec<NetworkInterfaceSnapshot>,
        now: Instant,
    ) -> Vec<NetworkEvent> {
        let mut deb_guard = self.debouncer.lock().await;
        let settled = deb_guard
            .ingest(new_snapshots, now)
            .or_else(|| deb_guard.poll_settled(now));

        if let Some(settled_snaps) = settled {
            let mut prev_guard = self.last_snapshots.lock().await;
            let mut detector_guard = self.detector.lock().await;

            let events = InterfaceDiffDetector::compute_diff_with_detector(
                &prev_guard,
                &settled_snaps,
                &mut detector_guard,
            );

            for ev in &events {
                let _ = self.sender.send(ev.clone());
            }

            *prev_guard = settled_snaps;
            events
        } else {
            Vec::new()
        }
    }

    /// Stops the interface watcher background task.
    pub fn stop(&self) {
        let _ = self.stop_tx.send(true);
    }
}

#[cfg(test)]
#[path = "interface_watcher_test.rs"]
mod tests;
