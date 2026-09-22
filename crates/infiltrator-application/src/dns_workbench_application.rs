//! DNS workbench read-model mapping shared by the surfaces.
//!
//! The domain profile is the single source of truth; these functions turn it
//! into the shared [`DnsPageSnapshot`] (Bevy projection and Iced render cache)
//! and into the shared workbench form draft. Iced's refresh path and the
//! surface reader both call the same mapping, so the two surfaces cannot show
//! different DNS facts.

use infiltrator_contract::dns::{DnsUpstreamProtocol, join_server_list};
use infiltrator_contract::dns_form::DnsWorkbenchForm;
use infiltrator_contract::surface_snapshot::DnsServerSnapshot;
use infiltrator_domain::dns;

/// The protocol chip label for one configured nameserver address.
pub fn dns_protocol(address: &str) -> String {
    DnsUpstreamProtocol::from_address(address)
        .chip_label()
        .to_owned()
}

/// Project the configured upstream tiers into the shared server list.
///
/// Primary nameservers come first, then the fallback tier; each entry carries
/// its shared semantic tags and a `None` latency because the host exposes no
/// per-nameserver latency fact yet.
pub fn dns_servers(config: &dns::DnsConfig) -> Vec<DnsServerSnapshot> {
    let primary = config.nameserver.clone().unwrap_or_default();
    let fallback = config.fallback.clone().unwrap_or_default();
    dns_servers_from_lists(primary, fallback)
}

/// Project explicit primary/fallback name lists (used by the runtime fallback
/// path where only the live config snapshot is available).
pub fn dns_servers_from_lists(
    primary: Vec<String>,
    fallback: Vec<String>,
) -> Vec<DnsServerSnapshot> {
    primary
        .into_iter()
        .map(|address| DnsServerSnapshot {
            protocol: dns_protocol(&address),
            tags: infiltrator_contract::dns::DnsServerTag::classify(&address, false),
            address,
            latency_ms: None,
            is_fallback: false,
        })
        .chain(fallback.into_iter().map(|address| DnsServerSnapshot {
            protocol: dns_protocol(&address),
            tags: infiltrator_contract::dns::DnsServerTag::classify(&address, true),
            address,
            latency_ms: None,
            is_fallback: true,
        }))
        .collect()
}

/// Build the shared DNS page read model from the configured profile.
pub fn dns_page_snapshot(
    config: &dns::DnsConfig,
    fake_ip_range: String,
) -> infiltrator_contract::surface_snapshot::DnsPageSnapshot {
    use infiltrator_contract::surface_snapshot::DnsPageSnapshot;
    DnsPageSnapshot {
        enhanced_mode: infiltrator_contract::dns::DnsEnhancedMode::from_config_value(
            config.enhanced_mode.as_deref(),
        ),
        cache_entries: 0,
        fake_ip_range,
        servers: dns_servers(config),
        switches: dns_core_switches(config),
        filter_mode: infiltrator_contract::dns::DnsFakeIpFilterMode::from_config_value(
            config.fake_ip_filter_mode.as_deref(),
        ),
        default_nameserver: config.default_nameserver.clone().unwrap_or_default(),
        fallback_policy: fallback_policy(config),
        fake_ip_filter: config.fake_ip_filter.clone().unwrap_or_default(),
        proxy_server_nameserver: config.proxy_server_nameserver.clone().unwrap_or_default(),
        direct_nameserver: config.direct_nameserver.clone().unwrap_or_default(),
        cache_flush: infiltrator_contract::dns::DnsCacheFlushReport::default(),
    }
}

/// Map the six profile switches onto the shared switch value object.
pub fn dns_core_switches(config: &dns::DnsConfig) -> infiltrator_contract::dns::DnsCoreSwitches {
    infiltrator_contract::dns::DnsCoreSwitches {
        enable: config.enable.unwrap_or(false),
        ipv6: config.ipv6.unwrap_or(false),
        cache: config.cache.unwrap_or(false),
        use_hosts: config.use_hosts.unwrap_or(false),
        use_system_hosts: config.use_system_hosts.unwrap_or(false),
        respect_rules: config.respect_rules.unwrap_or(false),
    }
}

/// Map `fallback-filter` onto the shared fallback policy.
pub fn fallback_policy(config: &dns::DnsConfig) -> infiltrator_contract::dns::DnsFallbackPolicy {
    let filter = config.fallback_filter.as_ref();
    infiltrator_contract::dns::DnsFallbackPolicy {
        geoip: filter.and_then(|filter| filter.geoip).unwrap_or(false),
        geoip_code: filter
            .and_then(|filter| filter.geoip_code.clone())
            .unwrap_or_default(),
        trigger_ipcidr: filter
            .and_then(|filter| filter.ipcidr.clone())
            .unwrap_or_default(),
    }
}

/// Seed the shared workbench form draft from the configured profile.
pub fn form_from_config(config: &dns::DnsConfig) -> DnsWorkbenchForm {
    let snapshot = dns_page_snapshot(config, config.fake_ip_range.clone().unwrap_or_default());
    DnsWorkbenchForm::from_snapshot(&snapshot)
}

/// Canonical raw editor string for a list field, shared by both surfaces.
pub fn join_editor_list(entries: &[String]) -> String {
    join_server_list(entries)
}

/// The honest DNS cache flush report the page read model publishes.
pub fn cache_flush_report(
    dns_cache: Option<&crate::dns_cache_application::DnsCacheApplication>,
) -> infiltrator_contract::dns::DnsCacheFlushReport {
    dns_cache
        .map(crate::dns_cache_application::DnsCacheApplication::last_report)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::dns::DnsFallbackPolicy;

    fn config() -> dns::DnsConfig {
        dns::DnsConfig {
            enable: Some(true),
            ipv6: Some(true),
            nameserver: Some(vec![
                "https://doh.pub/dns-query".to_owned(),
                "223.5.5.5".to_owned(),
            ]),
            fallback: Some(vec!["tls://1.0.0.1:853".to_owned()]),
            default_nameserver: Some(vec!["223.5.5.5".to_owned()]),
            proxy_server_nameserver: Some(vec!["tls://223.5.5.5:853".to_owned()]),
            direct_nameserver: Some(vec!["system".to_owned()]),
            fallback_filter: Some(dns::FallbackFilter {
                geoip: Some(true),
                geoip_code: Some("CN".to_owned()),
                ipcidr: Some(vec!["192.168.0.0/16".to_owned()]),
                ..dns::FallbackFilter::default()
            }),
            fake_ip_range: Some("198.18.0.1/16".to_owned()),
            fake_ip_filter: Some(vec!["*.lan".to_owned()]),
            ..dns::DnsConfig::default()
        }
    }

    #[test]
    fn snapshot_exposes_every_workbench_tier_and_protocol_chip() {
        let snapshot = dns_page_snapshot(&config(), "198.18.0.1/16".to_owned());
        assert_eq!(snapshot.servers.len(), 3);
        assert_eq!(snapshot.servers[0].protocol, "DoH");
        assert_eq!(snapshot.servers[1].protocol, "UDP");
        assert_eq!(snapshot.servers[2].protocol, "DoT");
        assert!(snapshot.servers[2].is_fallback);
        assert_eq!(snapshot.default_nameserver, vec!["223.5.5.5".to_owned()]);
        assert_eq!(
            snapshot.proxy_server_nameserver,
            vec!["tls://223.5.5.5:853".to_owned()]
        );
        assert_eq!(snapshot.direct_nameserver, vec!["system".to_owned()]);
        assert_eq!(snapshot.fake_ip_filter, vec!["*.lan".to_owned()]);
        assert_eq!(
            snapshot.fallback_policy,
            DnsFallbackPolicy {
                geoip: true,
                geoip_code: "CN".to_owned(),
                trigger_ipcidr: vec!["192.168.0.0/16".to_owned()],
            }
        );
        assert!(snapshot.switches.enable);
        assert!(snapshot.switches.ipv6);
    }

    #[test]
    fn form_maps_both_directions_without_losing_a_field() {
        let form = form_from_config(&config());
        assert_eq!(form.nameserver, "https://doh.pub/dns-query, 223.5.5.5");
        assert_eq!(form.fallback, "tls://1.0.0.1:853");
        assert_eq!(form.bootstrap_nameserver, "223.5.5.5");
        assert_eq!(form.fake_ip_range, "198.18.0.1/16");
        assert_eq!(form.fallback_policy.trigger_ipcidr, "192.168.0.0/16");
        assert_eq!(
            form.patch().nameserver.as_deref().map(<[String]>::len),
            Some(2)
        );
        assert!(form.validate().is_empty());
        assert_eq!(join_editor_list(&["a".to_owned(), "b".to_owned()]), "a, b");
    }

    #[tokio::test]
    async fn cache_flush_report_follows_the_shared_application_state() {
        use infiltrator_contract::dns::{DnsCacheFlushReport, DnsFlushOutcome};
        assert_eq!(cache_flush_report(None), DnsCacheFlushReport::default());

        let application = crate::dns_cache_application::DnsCacheApplication::unconfigured();
        let report = application.flush_all().await.expect("flush");
        assert!(matches!(
            report.fake_ip,
            DnsFlushOutcome::Unsupported { .. }
        ));
        assert_eq!(cache_flush_report(Some(&application)), report);
    }
}
