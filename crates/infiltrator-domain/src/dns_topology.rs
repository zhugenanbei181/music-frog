//! DNS topology anti-leak auditing and diagnostics.

use serde::{Deserialize, Serialize};
use crate::dns::DnsConfig;

/// Diagnostic severity for DNS topology auditing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TopologySeverity {
    Info,
    Warning,
    Error,
}

/// Diagnostic item returned by DNS topology verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsTopologyDiagnostic {
    pub severity: TopologySeverity,
    pub code: String,
    pub message: String,
    pub suggestion: Option<String>,
}

/// Audits the entire DNS configuration against best practice topology anti-leak guidelines.
pub fn validate_dns_topology(config: &DnsConfig) -> Vec<DnsTopologyDiagnostic> {
    let mut diagnostics = Vec::new();

    // Check if DNS is enabled
    if config.enable == Some(false) {
        diagnostics.push(DnsTopologyDiagnostic {
            severity: TopologySeverity::Warning,
            code: "DNS_DISABLED".to_string(),
            message: "DNS service is disabled; standard OS resolution will be used without leak protection".to_string(),
            suggestion: Some("Set enable: true to enforce DNS anti-leak routing".to_string()),
        });
        return diagnostics;
    }

    // Check Tier 1: Bootstrap nameserver
    if config.default_nameserver.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
        diagnostics.push(DnsTopologyDiagnostic {
            severity: TopologySeverity::Warning,
            code: "BOOTSTRAP_DNS_MISSING".to_string(),
            message: "No default-nameserver defined. Encrypted upstreams (DoH/DoT/DoQ) may fail initial domain resolution.".to_string(),
            suggestion: Some("Add pure IP bootstrap servers (e.g., 223.5.5.5, 119.29.29.29, 1.1.1.1)".to_string()),
        });
    }

    // Check Tier 2: Direct nameserver
    if config.direct_nameserver.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
        diagnostics.push(DnsTopologyDiagnostic {
            severity: TopologySeverity::Info,
            code: "DIRECT_NAMESERVER_UNSET".to_string(),
            message: "direct-nameserver is not configured. Direct connections will fall back to general nameservers.".to_string(),
            suggestion: Some("Configure fast local DNS for direct-nameserver (e.g., https://223.5.5.5/dns-query)".to_string()),
        });
    }

    // Check Tier 3: Proxy server nameserver
    if config.proxy_server_nameserver.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
        diagnostics.push(DnsTopologyDiagnostic {
            severity: TopologySeverity::Info,
            code: "PROXY_SERVER_NAMESERVER_UNSET".to_string(),
            message: "proxy-server-nameserver is unset. Proxy node domains will resolve via default-nameserver.".to_string(),
            suggestion: Some("Configure proxy-server-nameserver to isolate node hostname resolution".to_string()),
        });
    }

    // Check Fallback Filter & Fallback alignment
    if config.fallback_filter.is_some() && config.fallback.as_ref().map(|v| v.is_empty()).unwrap_or(true) {
        diagnostics.push(DnsTopologyDiagnostic {
            severity: TopologySeverity::Warning,
            code: "FALLBACK_FILTER_WITHOUT_FALLBACK".to_string(),
            message: "fallback-filter is defined but fallback nameservers list is empty.".to_string(),
            suggestion: Some("Add untainted fallback nameservers (e.g., https://8.8.8.8/dns-query#Proxy)".to_string()),
        });
    }

    // Check Fake-IP mode & store-fake-ip
    if config.enhanced_mode.as_deref() == Some("fake-ip") {
        if config.fake_ip_range.is_none() {
            diagnostics.push(DnsTopologyDiagnostic {
                severity: TopologySeverity::Info,
                code: "FAKE_IP_RANGE_DEFAULT".to_string(),
                message: "fake-ip-range is not explicitly set; Mihomo will use internal default 198.18.0.1/16.".to_string(),
                suggestion: Some("Explicitly specify fake-ip-range: 198.18.0.1/16".to_string()),
            });
        }
        if config.store_fake_ip != Some(true) {
            diagnostics.push(DnsTopologyDiagnostic {
                severity: TopologySeverity::Info,
                code: "FAKE_IP_PERSISTENCE_DISABLED".to_string(),
                message: "store-fake-ip is false. Fake-IP pool will reset on core reload, potentially dropping long-lived connections.".to_string(),
                suggestion: Some("Set store-fake-ip: true to persist mapping cache across restarts".to_string()),
            });
        }
    }

    // Check ECS Subnet safety
    if let Some(ecs) = config.edns_client_subnet.as_deref() {
        let has_foreign_upstreams = config.nameserver.as_ref().map(|ns| {
            ns.iter().any(|s| !s.contains("223.5.5.5") && !s.contains("119.29.29.29") && !s.contains("114.114.114.114"))
        }).unwrap_or(false);

        if has_foreign_upstreams && config.ecs_override_policy.as_deref() != Some("strip") {
            diagnostics.push(DnsTopologyDiagnostic {
                severity: TopologySeverity::Warning,
                code: "ECS_PRIVACY_LEAK_RISK".to_string(),
                message: format!("edns-client-subnet ({}) is broadcast to remote/foreign nameservers, which may leak domestic IP prefix.", ecs),
                suggestion: Some("Set ecs-override-policy: strip for foreign nameservers or use nameserver-policy to restrict ECS".to_string()),
            });
        }
    }

    diagnostics
}
