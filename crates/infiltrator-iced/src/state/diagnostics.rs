//! Diagnostics owner.

use infiltrator_application::dns_cache_actions::DnsCacheActions;
use infiltrator_application::dns_leak_actions::DnsLeakActionState;
use infiltrator_application::dns_leak_application::DnsLeakApplication;
use infiltrator_application::dns_query_actions::DnsQueryActions;
use infiltrator_application::log_export_actions::LogExportActions;
use infiltrator_application::log_projection::page_from_log_records;
use infiltrator_application::log_search::LogSearchState;

use crate::types::app::Route;
use crate::types::doctor::{CrashWatchdogState, DoctorPanelState};
use crate::types::perf::PerfSnapshot;
use crate::types::runtime::{LogFilterState, PcapCaptureState, RuntimeStreamState};
use infiltrator_application::connection_grouping::ConnectionGroupingState;
use infiltrator_application::connection_rate_application::ConnectionRateApplication;
use infiltrator_contract::dns_cache::DnsCacheFlushReport;
use infiltrator_contract::error::Failure;
use infiltrator_contract::overview_layout::OverviewCardKind;
use infiltrator_contract::speedtest::SpeedtestSnapshot;
use infiltrator_domain::connection_activity::ConnectionActivityTracker;
use infiltrator_domain::connection_rate::ConnectionRates;
use infiltrator_domain::runtime::Connection;
use infiltrator_domain::runtime::{ConnectionSnapshot, MemoryData, TrafficData};
use std::collections::VecDeque;
use std::time::Instant;

/// 诊断域:流量/内存/连接/日志运行态快照与性能 HUD 测量(UI-002)。
pub struct DiagnosticsState {
    pub traffic: Option<TrafficData>,
    pub traffic_history: VecDeque<(u64, u64)>,
    pub memory: Option<MemoryData>,
    pub public_ip: Option<String>,
    pub public_ip_provider: Option<String>,
    pub public_ip_checked_at: Option<String>,
    pub public_ip_error: Option<String>,
    pub connections: Option<ConnectionSnapshot>,
    /// Connections list pagination (mirrors the rules page pattern): the
    /// view renders only the current window so multi-thousand-connection
    /// snapshots never build thousands of widgets at once.
    pub connections_page: usize,
    pub connections_page_size: usize,
    pub logs: VecDeque<String>,
    pub log_level: String,
    pub fps: u32,
    pub last_frame_time: Instant,
    /// Adapter-local phase for the Overview topology flow strip. The shared
    /// snapshot remains the only source of whether a flow exists.
    pub topology_flow_phase: f32,
    pub perf_snapshot: PerfSnapshot,
    pub perf_panel_visible: bool,
    pub perf_nav_started_at: Option<Instant>,
    pub perf_nav_route: Option<Route>,
    pub logs_stream_state: RuntimeStreamState,
    pub log_command_failure: Option<Failure>,
    pub log_search: LogSearchState,
    pub log_export: LogExportActions,
    pub traffic_stream_state: RuntimeStreamState,
    pub connections_stream_state: RuntimeStreamState,
    pub doctor: DoctorPanelState,
    pub inspecting_connection_id: Option<String>,
    pub is_probing_dns_leak: bool,
    pub dns_leak_action: DnsLeakActionState,
    pub dns_leak_capture: Option<DnsLeakApplication>,
    /// DUAL-14-09 (re-scoped): a STUN egress probe started from this surface
    /// is in flight.
    pub is_probing_stun: bool,
    /// Honest per-target report of the last Fake-IP / OS DNS cache flush,
    /// consumed from the shared DNS page read model (DUAL-14-07).
    pub dns_cache_flush: DnsCacheFlushReport,
    pub dns_cache_actions: DnsCacheActions,
    pub dns_query: DnsQueryActions,
    pub pcap_state: PcapCaptureState,
    /// Canonical speedtest read model published by the shared application
    /// engine. This replaces the former UI-local fabricated metrics: the view
    /// renders this snapshot, and `RunSpeedtest` intents drive the engine.
    pub speedtest: SpeedtestSnapshot,
    /// DUAL-06-13: whether the per-node speedtest detail modal is open. Pure
    /// view state; the modal reads the shared `speedtest` snapshot above.
    pub speedtest_detail_open: bool,
    /// Overview card display order from the shared `OverviewLayoutSnapshot`.
    /// Local render projection of the shared contract; the view assembles its
    /// reorderable cards in this order.
    pub overview_card_order: Vec<OverviewCardKind>,
    pub crash_watchdog: CrashWatchdogState,
    pub log_filter: LogFilterState,
    pub connection_groups: ConnectionGroupingState<Connection>,
    /// DUAL-13-11: byte-change tracking that backs idle detection.
    pub connection_activity: ConnectionActivityTracker,
    /// DUAL-13-10/12: the shared rate window fed by successive live snapshots.
    pub connection_rates: ConnectionRateApplication,
    /// DUAL-13-10/12: the instantaneous rates derived from the last snapshot.
    pub connection_rate_book: ConnectionRates,
    /// DUAL-13-10: breathing phase of the high-throughput pulse (0.0..1.0).
    pub connection_pulse_phase: f32,
    /// Configured idle timeout in seconds (one of the shared choices).
    pub connection_idle_timeout_secs: u64,
    /// Idle connections identified by the last sweep, if one has run.
    pub last_idle_sweep: Option<usize>,
}

impl DiagnosticsState {
    pub fn observe_log_records(&mut self) {
        self.log_search.observe(
            0,
            None,
            &page_from_log_records(self.logs.iter().map(String::as_str)),
        );
    }

    /// Whether the last observed snapshot contains a connection above the
    /// shared high-throughput threshold, i.e. whether the pulse needs frames.
    pub fn connection_pulse_active(&self) -> bool {
        self.connection_rate_book.has_high_throughput()
    }
}
