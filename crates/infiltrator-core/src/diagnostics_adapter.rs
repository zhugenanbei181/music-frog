//! Transport adapter for diagnostics input.
//!
//! The calculators and privacy evaluator are transport-independent and live
//! in `infiltrator-domain::diagnostics`. This module converts Mihomo's wire
//! connection DTO into the domain input model; no algorithm belongs here.

use infiltrator_domain::diagnostics::{
    DiagnosticConnection, DnsResolutionLog, LeakTestOutcome, PrivacyLeakDetectionSuite,
};

pub fn evaluate_mihomo_connections(
    connections: &[mihomo_api::types::Connection],
    dns_logs: &[DnsResolutionLog],
) -> LeakTestOutcome {
    let converted = connections
        .iter()
        .map(|connection| DiagnosticConnection {
            id: connection.id.clone(),
            network: connection.metadata.network.clone(),
            source_ip: connection.metadata.source_ip.clone(),
            destination_ip: connection.metadata.destination_ip.clone(),
            destination_port: connection.metadata.destination_port.parse().unwrap_or(0),
            host: connection.metadata.host.clone(),
            rule: connection.rule.clone(),
            chains: connection.chains.clone(),
            process_path: if connection.metadata.process_path.trim().is_empty() {
                None
            } else {
                Some(connection.metadata.process_path.clone())
            },
        })
        .collect::<Vec<_>>();
    PrivacyLeakDetectionSuite::evaluate(&converted, dns_logs)
}

#[cfg(test)]
#[path = "diagnostics_adapter_test.rs"]
mod tests;
