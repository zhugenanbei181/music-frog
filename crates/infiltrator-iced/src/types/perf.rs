//! Performance instrumentation snapshot (perf-panel telemetry).

#[derive(Debug, Clone, Default)]
pub struct PerfSnapshot {
    pub navigate_to_first_paint_ms: Option<u128>,
    pub rules_cache_build_ms: u128,
    pub rules_with_text_apply_ms: u128,
    pub dns_with_text_apply_ms: u128,
    pub rules_visible_rows: usize,
}
