//! Shared typed detail projection for both native UI surfaces.

use infiltrator_contract::speedtest::{SpeedtestPhase, SpeedtestSnapshot};
use infiltrator_contract::speedtest_details::{SpeedtestDetailRow, SpeedtestDetails};

pub fn project_details(snapshot: &SpeedtestSnapshot) -> SpeedtestDetails {
    SpeedtestDetails {
        rows: snapshot
            .sorted_by_latency()
            .into_iter()
            .map(|result| {
                let calculation = result.calculation();
                SpeedtestDetailRow {
                    outbound_id: result.outbound_id.clone(),
                    display_name: result.display_name.clone(),
                    protocol_display_name: result.protocol_display_name.clone(),
                    delay: result.delay_ms(),
                    jitter_ms: calculation
                        .as_ref()
                        .filter(|value| value.successful_probes >= 2)
                        .map(|value| value.jitter_ms),
                    loss_percent: calculation.map(|value| value.loss_percent),
                    sample_count: result.samples.len(),
                    state: result.state.clone(),
                    samples: result.samples.clone(),
                }
            })
            .collect(),
        phase: snapshot.phase(),
    }
}

pub fn listing(details: &SpeedtestDetails, translate: &impl Fn(&str) -> String) -> String {
    if details.rows.is_empty() {
        return match &details.phase {
            SpeedtestPhase::Idle | SpeedtestPhase::Completed => translate("speedtest_detail_empty"),
            SpeedtestPhase::Probing => translate("speedtest_phase_probing"),
            SpeedtestPhase::CancelRequested => translate("speedtest_phase_cancel_requested"),
            SpeedtestPhase::Cancelled => translate("speedtest_phase_cancelled"),
            SpeedtestPhase::Failed { failure } => format!(
                "{}: {}",
                translate("speedtest_phase_failed"),
                failure.message
            ),
            SpeedtestPhase::Unavailable { failure } => format!(
                "{}: {}",
                translate("speedtest_phase_unavailable"),
                failure.message
            ),
        };
    }
    details
        .rows
        .iter()
        .map(|row| {
            let name = row
                .display_name
                .as_deref()
                .unwrap_or_else(|| row.outbound_id.value());
            let protocol = row.protocol_display_name.as_deref().unwrap_or("—");
            format!(
                "{name} · {protocol}\n{} {} · {} {} · {} {} · {} {}",
                translate("speedtest_detail_delay"),
                row.delay
                    .map(|delay| format!("{delay} ms"))
                    .unwrap_or_else(|| "—".to_owned()),
                translate("speedtest_jitter"),
                row.jitter_ms
                    .map(|jitter| format!("{jitter:.1} ms"))
                    .unwrap_or_else(|| "—".to_owned()),
                translate("speedtest_packet_loss"),
                row.loss_percent
                    .map(|loss| format!("{loss:.1}%"))
                    .unwrap_or_else(|| "—".to_owned()),
                translate("speedtest_detail_samples"),
                row.sample_count,
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
#[path = "speedtest_detail_projection_test.rs"]
mod tests;
