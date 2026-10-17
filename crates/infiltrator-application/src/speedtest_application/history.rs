use super::*;

pub(super) fn build_history_record(
    run_id: SpeedtestRunId,
    results: &[SpeedtestOutboundResult],
    target: &SpeedtestTarget,
) -> HistoricalSpeedtestRecord {
    let calculations = results
        .iter()
        .filter_map(|result| result.calculation())
        .collect::<Vec<_>>();
    let latencies = calculations
        .iter()
        .filter(|calculation| calculation.successful_probes > 0)
        .map(|calculation| calculation.mean_rtt_ms)
        .collect::<Vec<_>>();
    let jitters = calculations
        .iter()
        .filter(|calculation| calculation.successful_probes > 1)
        .map(|calculation| calculation.jitter_ms)
        .collect::<Vec<_>>();
    HistoricalSpeedtestRecord {
        run_id,
        timestamp_epoch_ms: current_epoch_ms(),
        target: Some(target.clone()),
        total_targets: results.len(),
        completed_targets: results
            .iter()
            .filter(|result| {
                matches!(
                    &result.state,
                    SpeedtestResultState::Completed | SpeedtestResultState::TimedOut
                )
            })
            .count(),
        avg_latency_ms: average(&latencies),
        avg_jitter_ms: average(&jitters),
    }
}

fn average(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

pub(super) fn push_history_record(
    history: &mut Vec<HistoricalSpeedtestRecord>,
    record: HistoricalSpeedtestRecord,
) {
    history.push(record);
    if history.len() > MAX_HISTORY_ENTRIES {
        history.remove(0);
    }
}

pub(super) fn bounded_history(
    mut history: Vec<HistoricalSpeedtestRecord>,
) -> Vec<HistoricalSpeedtestRecord> {
    if history.len() > MAX_HISTORY_ENTRIES {
        history.drain(0..history.len() - MAX_HISTORY_ENTRIES);
    }
    history
}

pub(super) fn invalid_target(message: impl Into<String>) -> Failure {
    Failure::new(ErrorCode::InvalidInput, message, false)
}

pub(super) fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
