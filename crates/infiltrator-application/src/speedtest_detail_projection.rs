//! One detail fold and localized listing for both peer products.
use crate::speedtest_summary_projection::node_metrics;
use infiltrator_contract::capability::Availability;
use infiltrator_contract::speedtest::{EgressCountryMatch, SpeedtestSnapshot};
use infiltrator_contract::speedtest_details::{SpeedtestDetailRow, SpeedtestDetails};

pub fn project_details(snapshot: &SpeedtestSnapshot) -> SpeedtestDetails {
    SpeedtestDetails {
        rows: snapshot
            .sorted_by_latency()
            .into_iter()
            .map(|node| {
                let metrics = node_metrics(node);
                SpeedtestDetailRow {
                    node_name: node.node_name.clone(),
                    proxy_type: node.proxy_type.clone(),
                    delay: node
                        .delay_ms
                        .map(|v| format!("{v} ms"))
                        .unwrap_or_else(|| "—".into()),
                    jitter: metrics.jitter,
                    loss: metrics.loss,
                    bandwidth: metrics.bandwidth,
                    stars: metrics.stars,
                    egress: node.egress_endpoint_label(),
                    egress_match: node.egress_country_match(),
                }
            })
            .collect(),
        availability: snapshot.availability.clone(),
        failure: snapshot.failure.clone(),
        reported_egress: snapshot.egress_reported_count(),
        mismatches: snapshot.egress_country_mismatches().len(),
    }
}

pub fn match_key(value: EgressCountryMatch) -> &'static str {
    match value {
        EgressCountryMatch::Match => "speedtest_detail_match",
        EgressCountryMatch::Mismatch => "speedtest_detail_mismatch",
        EgressCountryMatch::Unlabelled => "speedtest_detail_unlabelled",
        EgressCountryMatch::Unknown => "speedtest_detail_unknown",
    }
}

pub fn summary(details: &SpeedtestDetails, translate: &impl Fn(&str) -> String) -> String {
    format!(
        "{} {}/{} · {} {}",
        translate("speedtest_detail_egress"),
        details.reported_egress,
        details.rows.len(),
        translate("speedtest_detail_mismatch"),
        details.mismatches
    )
}

pub fn notice(details: &SpeedtestDetails, translate: &impl Fn(&str) -> String) -> Option<String> {
    match &details.availability {
        Some(Availability::Unsupported { reason }) => Some(format!(
            "{}: {reason}",
            translate("speedtest_detail_unsupported")
        )),
        Some(Availability::Unavailable { reason }) => Some(format!(
            "{}: {reason}",
            translate("speedtest_detail_unavailable")
        )),
        _ => details
            .failure
            .as_ref()
            .map(|failure| format!("{}: {failure}", translate("speedtest_detail_failed"))),
    }
}

pub fn listing(details: &SpeedtestDetails, translate: &impl Fn(&str) -> String) -> String {
    let mut lines = Vec::new();
    if let Some(message) = notice(details, translate) {
        lines.push(message);
    }
    if details.rows.is_empty() {
        if lines.is_empty() {
            lines.push(translate("speedtest_detail_empty"));
        }
    } else {
        lines.push(summary(details, translate));
        for row in &details.rows {
            lines.push(format!(
                "{} · {}\n{} {} · {} {} · {} {}\n{} {} · {} {}\n{} {} · {}",
                row.node_name,
                row.proxy_type,
                translate("speedtest_detail_delay"),
                row.delay,
                translate("speedtest_jitter"),
                row.jitter,
                translate("speedtest_packet_loss"),
                row.loss,
                translate("speedtest_bandwidth"),
                row.bandwidth,
                translate("speedtest_detail_stars"),
                row.stars,
                translate("speedtest_detail_egress"),
                row.egress,
                translate(match_key(row.egress_match))
            ));
        }
    }
    lines.join("\n\n")
}

#[cfg(test)]
#[path = "speedtest_detail_projection_test.rs"]
mod tests;
