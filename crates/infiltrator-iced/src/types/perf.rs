//! Performance instrumentation snapshot (perf-panel telemetry).

#[derive(Debug, Clone, Default)]
pub struct PerfSnapshot {
    pub navigate_to_first_paint_ms: Option<u128>,
    pub rules_cache_build_ms: u128,
    pub rules_with_text_apply_ms: u128,
    pub dns_with_text_apply_ms: u128,
    pub rules_visible_rows: usize,
}

/// Results of the downstream bandwidth and jitter benchmark.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SpeedtestResult {
    pub target_node: String,
    pub bandwidth_mbps: f64,
    pub jitter_ms: f64,
    pub packet_loss_percent: f64,
    pub tier: String,
    pub is_running: bool,
}
