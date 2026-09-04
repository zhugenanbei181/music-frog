//! Privacy leak detection and active connections traffic audit endpoints (`/admin/api/audit`).

use axum::Json;
use chrono::Utc;
use infiltrator_domain::dns_tester::DnsTester;
use std::collections::HashMap;

use crate::admin_api::models::{
    ApiError, AuditProcessTraffic, AuditResponse, AuditTrafficSummary, PrivacyLeakIssue,
};
use crate::admin_api::state::{AdminApiContext, AdminApiState};

struct ProcessStatAccumulator {
    process_name: String,
    upload_bytes: u64,
    download_bytes: u64,
    connections_count: usize,
    direct_connections_count: usize,
}

pub async fn get_audit_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<AuditResponse>, ApiError> {
    let now = Utc::now().to_rfc3339();

    let client = match state.ctx.runtime_client().await {
        Ok(c) => c,
        Err(_) => {
            return Ok(Json(AuditResponse {
                leak_detected: false,
                leaks: Vec::new(),
                traffic_summary: AuditTrafficSummary {
                    upload_total: 0,
                    download_total: 0,
                    active_connections: 0,
                    proxied_bytes: 0,
                    direct_bytes: 0,
                    direct_bypass_ratio: 0.0,
                },
                top_processes: Vec::new(),
                audited_connections_count: 0,
                timestamp: now,
            }));
        }
    };

    let connections_resp = client
        .get_connections()
        .await
        .map_err(|e| ApiError::internal(format!("failed to get runtime connections: {e}")))?;

    let audit_result = analyze_connections_for_audit(
        &connections_resp.connections,
        connections_resp.upload_total,
        connections_resp.download_total,
        now,
    );

    Ok(Json(audit_result))
}

pub(crate) fn analyze_connections_for_audit(
    connections: &[mihomo_api::types::Connection],
    upload_total: u64,
    download_total: u64,
    timestamp: String,
) -> AuditResponse {
    let mut proxied_bytes: u64 = 0;
    let mut direct_bytes: u64 = 0;
    let mut leaks = Vec::new();
    let mut process_stats: HashMap<String, ProcessStatAccumulator> = HashMap::new();

    for conn in connections {
        let is_direct = conn.rule.eq_ignore_ascii_case("DIRECT")
            || conn.chains.iter().any(|c| c.eq_ignore_ascii_case("DIRECT"));
        let is_reject = conn.rule.eq_ignore_ascii_case("REJECT")
            || conn.chains.iter().any(|c| c.eq_ignore_ascii_case("REJECT"));

        let total_flow = conn.upload.saturating_add(conn.download);
        if is_direct {
            direct_bytes = direct_bytes.saturating_add(total_flow);
        } else if !is_reject {
            proxied_bytes = proxied_bytes.saturating_add(total_flow);
        }

        let proc_name = extract_process_name(&conn.metadata.process_path, &conn.metadata.host);

        let stat = process_stats
            .entry(proc_name.clone())
            .or_insert_with(|| ProcessStatAccumulator {
                process_name: proc_name.clone(),
                upload_bytes: 0,
                download_bytes: 0,
                connections_count: 0,
                direct_connections_count: 0,
            });
        stat.upload_bytes += conn.upload;
        stat.download_bytes += conn.download;
        stat.connections_count += 1;
        if is_direct {
            stat.direct_connections_count += 1;
        }

        // Privacy Leak Rule 1: DNS query bypassing proxy to external DNS
        let is_dns_port = conn.metadata.destination_port == "53"
            || (conn.metadata.network.eq_ignore_ascii_case("udp")
                && conn.metadata.destination_port == "53");
        let is_external_ip = !conn.metadata.destination_ip.starts_with("127.")
            && conn.metadata.destination_ip != "::1"
            && !conn.metadata.destination_ip.is_empty();

        if is_dns_port && is_direct && is_external_ip {
            leaks.push(PrivacyLeakIssue {
                id: format!("leak.dns.{}", conn.id),
                severity: "high".to_string(),
                category: "dns".to_string(),
                title: "Unencrypted DNS Query Bypassing Proxy (DNS Leak)".to_string(),
                detail: format!(
                    "Plaintext DNS query to {}:{} from process '{}' bypassed proxy rules directly",
                    conn.metadata.destination_ip, conn.metadata.destination_port, proc_name
                ),
                affected_target: Some(format!(
                    "{}:{}",
                    conn.metadata.destination_ip, conn.metadata.destination_port
                )),
                process_name: Some(proc_name.clone()),
                recommendation: "Enable Fake-IP or route DNS queries through encrypted upstream DNS / TUN mode"
                    .to_string(),
            });
        }

        // Privacy Leak Rule 2: Fake-IP address routed DIRECT (routing loop / misconfiguration)
        if is_direct
            && !conn.metadata.destination_ip.is_empty()
            && DnsTester::check_fake_ip_range(&conn.metadata.destination_ip, "198.18.0.0/15")
        {
            leaks.push(PrivacyLeakIssue {
                id: format!("leak.fake_ip.{}", conn.id),
                severity: "high".to_string(),
                category: "fake_ip".to_string(),
                title: "Direct Connection to Fake-IP Address".to_string(),
                detail: format!(
                    "Connection to Fake-IP {} from process '{}' was routed via DIRECT instead of proxy handler",
                    conn.metadata.destination_ip, proc_name
                ),
                affected_target: Some(conn.metadata.destination_ip.clone()),
                process_name: Some(proc_name.clone()),
                recommendation: "Review routing rules to ensure domain/fake-ip mapping routes to proxy group"
                    .to_string(),
            });
        }

        // Privacy Leak Rule 3: Cleartext HTTP traffic transferring significant data
        if conn.metadata.destination_port == "80"
            && (conn.upload > 10_000 || conn.download > 50_000)
        {
            leaks.push(PrivacyLeakIssue {
                id: format!("leak.cleartext.{}", conn.id),
                severity: "low".to_string(),
                category: "cleartext".to_string(),
                title: "Unencrypted HTTP Traffic Detected".to_string(),
                detail: format!(
                    "Process '{}' transferred unencrypted data over port 80 to '{}'",
                    proc_name, conn.metadata.host
                ),
                affected_target: Some(conn.metadata.host.clone()),
                process_name: Some(proc_name.clone()),
                recommendation: "Use HTTPS/TLS where possible to prevent eavesdropping on plaintext traffic"
                    .to_string(),
            });
        }
    }

    let leak_detected = leaks
        .iter()
        .any(|i| i.severity == "high" || i.severity == "medium");

    let mut top_processes: Vec<AuditProcessTraffic> = process_stats
        .into_values()
        .map(|s| {
            let total = s.upload_bytes + s.download_bytes;
            let ratio = if s.connections_count > 0 {
                s.direct_connections_count as f64 / s.connections_count as f64
            } else {
                0.0
            };
            AuditProcessTraffic {
                process_name: s.process_name,
                upload_bytes: s.upload_bytes,
                download_bytes: s.download_bytes,
                total_bytes: total,
                connections_count: s.connections_count,
                direct_bypass_ratio: ratio,
            }
        })
        .collect();

    top_processes.sort_by_key(|p| std::cmp::Reverse(p.total_bytes));
    top_processes.truncate(20);

    let total_classified = proxied_bytes + direct_bytes;
    let direct_bypass_ratio = if total_classified > 0 {
        direct_bytes as f64 / total_classified as f64
    } else {
        0.0
    };

    AuditResponse {
        leak_detected,
        leaks,
        traffic_summary: AuditTrafficSummary {
            upload_total,
            download_total,
            active_connections: connections.len(),
            proxied_bytes,
            direct_bytes,
            direct_bypass_ratio,
        },
        top_processes,
        audited_connections_count: connections.len(),
        timestamp,
    }
}

fn extract_process_name(process_path: &str, host: &str) -> String {
    let trimmed_path = process_path.trim();
    if !trimmed_path.is_empty() {
        if let Some(name) = std::path::Path::new(trimmed_path)
            .file_name()
            .and_then(|n| n.to_str())
        {
            return name.to_string();
        }
        return trimmed_path.to_string();
    }
    let trimmed_host = host.trim();
    if !trimmed_host.is_empty() {
        return trimmed_host.to_string();
    }
    "unknown".to_string()
}
