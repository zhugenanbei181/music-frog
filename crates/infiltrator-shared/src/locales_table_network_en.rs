//! Shared network workbench copy.
use std::borrow::Cow;

pub(super) fn translate_network_en(key: &str) -> Cow<'static, str> {
    match key {
        "dns_field_enable" => "enable (DNS service)".into(),
        "dns_field_ipv6" => "ipv6 (IPv6 resolution)".into(),
        "dns_field_cache" => "cache (DNS memory cache)".into(),
        "dns_field_use_hosts" => "use_hosts (Hosts mappings)".into(),
        "dns_field_use_system_hosts" => "use_system_hosts (System Hosts)".into(),
        "dns_field_respect_rules" => "respect_rules (Routing rules first)".into(),
        "dns_field_enhanced_mode" => "enhanced_mode (Domain mapping mode)".into(),
        "dns_field_filter_mode" => "fake_ip_filter_mode (Fake-IP filter mode)".into(),
        "dns_field_bootstrap" => "default_nameserver (bootstrap, pure IP)".into(),
        "dns_field_nameserver" => "nameserver (DoH / DoT / DoQ / UDP)".into(),
        "dns_field_fallback" => "fallback (Fallback resolvers)".into(),
        "dns_field_geoip" => "fallback_filter.geoip (GEOIP fallback trigger)".into(),
        "dns_field_geoip_code" => "fallback_filter.geoip_code (ISO country code)".into(),
        "dns_field_trigger" => "fallback_filter.ipcidr (GEOIP trigger networks)".into(),
        "dns_field_fake_range" => "fake_ip_range".into(),
        "dns_field_fake_filter" => "fake_ip_filter".into(),
        "dns_field_proxy_nameserver" => "proxy_server_nameserver".into(),
        "dns_field_direct_nameserver" => "direct_nameserver".into(),
        "dns_trigger_count" => "fallback_filter.ipcidr: {count} trigger networks".into(),
        "dns_edit_invalid" => "Local validation failed: {reason}".into(),
        "dns_edit_dirty" => "Unapplied changes; apply to submit the shared patch".into(),
        "dns_edit_queued" => "DNS workbench patch queued; awaiting actual result".into(),
        "dns_edit_unchanged" => "Form matches the observed configuration".into(),
        "dns_edit_host_unavailable" => {
            "Command service is not composed; DNS patch was not submitted".into()
        }
        "network_status_detail" => "{state} · {reason}".into(),
        "network_vpn_details" => "foreground={foreground} · MTU={mtu} · routes={routes} · IPv6={ipv6}".into(),
        "network_privileged_details" => "operations={operations} · injected={injected} · cleanup={cleanup} · rollback={rollback}".into(),
        "network_pac_running" => "Running · {url} · {bytes} bytes".into(),
        "network_pac_disabled" => "Disabled".into(),
        _ => Cow::Owned(key.to_owned()),
    }
}
