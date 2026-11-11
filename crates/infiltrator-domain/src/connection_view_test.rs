use super::*;
use crate::runtime::ConnectionMetadata;
use infiltrator_contract::connection::ConnectionRateObservation;

fn connection(id: &str, host: &str, ip: &str, process: &str, up: u64, down: u64) -> Connection {
    Connection {
        id: id.to_string(),
        metadata: ConnectionMetadata {
            network: "tcp".to_string(),
            connection_type: "HTTP".to_string(),
            source_ip: "192.168.1.5".to_string(),
            destination_ip: ip.to_string(),
            source_port: "50000".to_string(),
            destination_port: "443".to_string(),
            host: host.to_string(),
            dns_mode: "normal".to_string(),
            process_path: process.to_string(),
            special_proxy: String::new(),
            destination_geo_ip: None,
            destination_ip_asn: String::new(),
        },
        upload: up,
        download: down,
        start: "2026-09-22T00:00:00Z".to_string(),
        rule: "DomainSuffix".to_string(),
        rule_payload: host.to_string(),
        chains: vec!["PROXY".to_string()],
    }
}

#[test]
fn grouping_mode_round_trips_its_identifier() {
    for mode in ConnectionGroupingMode::ALL {
        assert_eq!(ConnectionGroupingMode::from_identifier(mode.as_str()), mode);
    }
    assert_eq!(
        ConnectionGroupingMode::from_identifier("unknown"),
        ConnectionGroupingMode::Flat
    );
    assert!(ConnectionGroupingMode::Flat.is_flat());
}

#[test]
fn sort_key_round_trips_its_identifier() {
    for key in [
        ConnectionSortKey::DownloadDesc,
        ConnectionSortKey::UploadDesc,
        ConnectionSortKey::DownloadRateDesc,
        ConnectionSortKey::UploadRateDesc,
        ConnectionSortKey::LatestDesc,
        ConnectionSortKey::HostAsc,
    ] {
        assert_eq!(ConnectionSortKey::from_identifier(key.as_str()), key);
    }
    assert_eq!(
        ConnectionSortKey::from_identifier("bogus"),
        ConnectionSortKey::DownloadDesc
    );
}

#[test]
fn rate_sort_keys_rank_through_the_shared_adapter() {
    use crate::connection_rate::{ConnectionRate, ConnectionRates};

    let conns = [
        connection("c1", "a.com", "1.1.1.1", "/bin/a", 100, 500),
        connection("c2", "b.com", "2.2.2.2", "/bin/b", 900, 200),
        connection("c3", "c.com", "3.3.3.3", "/bin/c", 500, 800),
    ];
    let rates = ConnectionRates::from_pairs([
        (
            "c1".to_string(),
            ConnectionRate {
                observation: ConnectionRateObservation::Measured,
                upload_bps: 0.0,
                download_bps: 9_000.0,
                ..Default::default()
            },
        ),
        (
            "c2".to_string(),
            ConnectionRate {
                observation: ConnectionRateObservation::Measured,
                upload_bps: 5_000.0,
                download_bps: 0.0,
                ..Default::default()
            },
        ),
        (
            "c3".to_string(),
            ConnectionRate {
                observation: ConnectionRateObservation::Measured,
                upload_bps: 1_000.0,
                download_bps: 1_000.0,
                ..Default::default()
            },
        ),
    ]);

    let mut rows: Vec<RatedConnection<'_, Connection>> = conns
        .iter()
        .map(|conn| RatedConnection::new(conn, rates.get(&conn.id)))
        .collect();
    sort_connections(&mut rows, ConnectionSortKey::DownloadRateDesc);
    assert_eq!(
        rows.iter().map(|row| row.view_id()).collect::<Vec<_>>(),
        vec!["c1", "c3", "c2"]
    );
    // The adapter still resolves every cumulative accessor through the row.
    assert_eq!(rows[0].view_host(), "a.com");
    assert_eq!(rows[0].view_download_total(), 500);

    sort_connections(&mut rows, ConnectionSortKey::UploadRateDesc);
    assert_eq!(
        rows.iter().map(|row| row.view_id()).collect::<Vec<_>>(),
        vec!["c2", "c3", "c1"]
    );

    // Without a derived rate every row reads zero and the stable id
    // tie-break keeps the order deterministic.
    let empty = ConnectionRates::default();
    let mut rows: Vec<RatedConnection<'_, Connection>> = conns
        .iter()
        .map(|conn| RatedConnection::new(conn, empty.get(&conn.id)))
        .collect();
    sort_connections(&mut rows, ConnectionSortKey::DownloadRateDesc);
    assert_eq!(
        rows.iter().map(|row| row.view_id()).collect::<Vec<_>>(),
        vec!["c1", "c2", "c3"]
    );
}

#[test]
fn process_display_name_strips_path_and_extension() {
    assert_eq!(process_display_name("/usr/bin/firefox"), "firefox");
    assert_eq!(
        process_display_name("C:\\Program Files\\Zed\\zed-editor.exe"),
        "zed-editor"
    );
    assert_eq!(process_display_name("   "), "");
}

#[test]
fn endpoint_copy_brackets_unbracketed_ipv6_hosts_only_when_a_port_is_present() {
    assert_eq!(display_endpoint("example.org", "443"), "example.org:443");
    assert_eq!(display_endpoint("2001:db8::1", "443"), "[2001:db8::1]:443");
    assert_eq!(
        display_endpoint("[2001:db8::1]", "443"),
        "[2001:db8::1]:443"
    );
    assert_eq!(display_endpoint("2001:db8::1", ""), "2001:db8::1");
}

#[test]
fn search_matches_domain_ip_and_process() {
    let conn = connection(
        "c1",
        "api.github.com",
        "140.82.121.5",
        "/usr/bin/git",
        100,
        200,
    );
    assert!(matches_search(&conn, ""));
    assert!(matches_search(&conn, "github"));
    assert!(matches_search(&conn, "140.82"));
    assert!(matches_search(&conn, "GIT"));
    assert!(!matches_search(&conn, "cloudflare"));
}

#[test]
fn search_falls_back_to_destination_ip_without_host() {
    let conn = connection("c2", "", "119.29.29.29", "/usr/bin/curl", 0, 0);
    assert!(matches_search(&conn, "119.29"));
    assert_eq!(connection_host(&conn), "119.29.29.29");
}

#[test]
fn aggregation_buckets_by_process_then_upload() {
    let conns = vec![
        connection("c1", "a.com", "1.1.1.1", "/usr/bin/git", 10, 100),
        connection("c2", "b.com", "2.2.2.2", "/usr/bin/git", 30, 200),
        connection("c3", "c.com", "3.3.3.3", "/usr/bin/curl", 25, 50),
    ];
    let by_process = aggregate_connections(&conns, ConnectionGroupingMode::ByProcess);
    assert_eq!(by_process.len(), 2);
    assert_eq!(by_process[0].key, "git");
    assert_eq!(by_process[0].count, 2);
    assert_eq!(by_process[0].upload_total, 40);
    assert_eq!(by_process[0].download_total, 300);
    assert_eq!(by_process[1].key, "curl");

    let by_host = aggregate_connections(&conns, ConnectionGroupingMode::ByHost);
    assert_eq!(by_host.len(), 3);
    assert_eq!(by_host[0].key, "b.com");
    assert_eq!(by_host[0].upload_total, 30);

    assert!(aggregate_connections(&conns, ConnectionGroupingMode::Flat).is_empty());
}

#[test]
fn sort_orders_by_download_upload_and_host() {
    let mut conns = vec![
        connection("c1", "b.com", "1.1.1.1", "/bin/a", 100, 500),
        connection("c2", "a.com", "2.2.2.2", "/bin/b", 900, 200),
        connection("c3", "c.com", "3.3.3.3", "/bin/c", 500, 800),
    ];
    sort_connections(&mut conns, ConnectionSortKey::DownloadDesc);
    assert_eq!(
        conns.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        vec!["c3", "c1", "c2"]
    );
    sort_connections(&mut conns, ConnectionSortKey::UploadDesc);
    assert_eq!(conns[0].id, "c2");
    sort_connections(&mut conns, ConnectionSortKey::HostAsc);
    assert_eq!(
        conns.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        vec!["c2", "c1", "c3"]
    );
}

#[test]
fn quick_rule_spec_prefers_domain_then_ip() {
    let domain = connection("c1", "github.com", "1.1.1.1", "/bin/git", 0, 0);
    let spec = quick_rule_spec(&domain, DEFAULT_QUICK_RULE_TARGET);
    assert_eq!(spec.pattern, "DOMAIN-SUFFIX,github.com");
    assert_eq!(spec.rule_line(), "DOMAIN-SUFFIX,github.com,DIRECT");
    assert!(spec.is_draftable());

    let ip_only = connection("c2", "", "8.8.8.8", "/bin/dig", 0, 0);
    assert_eq!(
        quick_rule_spec(&ip_only, "PROXY").rule_line(),
        "IP-CIDR,8.8.8.8/32,PROXY"
    );

    let empty = connection("c3", "", "", "/bin/x", 0, 0);
    assert!(!quick_rule_spec(&empty, DEFAULT_QUICK_RULE_TARGET).is_draftable());
}

#[test]
fn route_chain_normalizes_and_parses_joined_form() {
    let conn = connection("c1", "a.com", "1.1.1.1", "/bin/a", 0, 0);
    // The test fixture carries a single structured hop.
    let chain = route_chain(&conn);
    assert_eq!(chain.hops(), ["PROXY"]);
    assert_eq!(chain.first(), Some("PROXY"));
    assert_eq!(chain.display(" → "), "PROXY");

    let joined = RouteChain::from_joined("节点选择 -> 香港 01 → PROXY");
    assert_eq!(
        joined.hops(),
        ["节点选择", "香港 01", "PROXY"],
        "both arrow styles split into hops"
    );

    let dirty = RouteChain::from_hops(&[
        " PROXY ".to_string(),
        String::new(),
        "PROXY".to_string(),
        "DIRECT".to_string(),
    ]);
    assert_eq!(dirty.hops(), ["PROXY", "DIRECT"]);
    assert_eq!(dirty.len(), 2);
    assert!(!dirty.is_empty());
    assert!(RouteChain::default().is_empty());
}

#[test]
fn bare_host_strips_port_and_keeps_ipv6() {
    assert_eq!(bare_host("api.github.com:443"), "api.github.com");
    assert_eq!(bare_host("1.1.1.1:53"), "1.1.1.1");
    assert_eq!(bare_host("2001:db8::1"), "2001:db8::1");
    assert_eq!(bare_host("example.com"), "example.com");
    assert_eq!(bare_host(""), "");

    // A `host:port` display key drafts the bare domain, not `host:port`.
    let conn = connection("c1", "api.github.com:443", "1.1.1.1", "/bin/git", 0, 0);
    assert_eq!(
        quick_rule_spec(&conn, "DIRECT").pattern,
        "DOMAIN-SUFFIX,api.github.com"
    );
}

#[test]
fn draft_rule_entry_dedupes_and_rejects_empty() {
    let conn = connection("c1", "github.com", "1.1.1.1", "/bin/git", 0, 0);
    let spec = quick_rule_spec(&conn, DEFAULT_QUICK_RULE_TARGET);
    let mut rules = Vec::new();
    let added = append_draft_rule(&mut rules, &spec).expect("first draft appended");
    assert_eq!(added.rule, "DOMAIN-SUFFIX,github.com,DIRECT");
    assert!(added.enabled);
    // The same rule line is not duplicated.
    assert!(append_draft_rule(&mut rules, &spec).is_none());
    assert_eq!(rules.len(), 1);

    let empty = quick_rule_spec(
        &connection("c2", "", "", "/bin/x", 0, 0),
        DEFAULT_QUICK_RULE_TARGET,
    );
    assert!(append_draft_rule(&mut rules, &empty).is_none());
    assert_eq!(rules.len(), 1);
}

#[test]
fn destination_geo_fact_keeps_the_kernels_three_states() {
    // `null`: the kernel never ran a GEOIP rule for this connection.
    assert_eq!(destination_geo_fact(None), DestinationGeoFact::NotEvaluated);
    // `[]`: it ran and had no record.
    assert_eq!(
        destination_geo_fact(Some(&[])),
        DestinationGeoFact::NoResult
    );
    // Real codes are carried verbatim, in kernel order.
    let codes = vec!["us".to_owned(), "cloudflare".to_owned()];
    assert_eq!(
        destination_geo_fact(Some(&codes)),
        DestinationGeoFact::Codes(codes.clone())
    );
}

#[test]
fn destination_asn_fact_separates_not_evaluated_from_no_result() {
    // `""`: no IP-ASN rule ran.
    assert_eq!(destination_asn_fact(""), DestinationAsnFact::NotEvaluated);
    // `" "`: mihomo evaluated and the ASN database had no record.
    assert_eq!(destination_asn_fact(" "), DestinationAsnFact::NoResult);
    // A real kernel value: raw, trimmed, no invented `AS` prefix.
    assert_eq!(
        destination_asn_fact("15169 Google LLC"),
        DestinationAsnFact::Reported("15169 Google LLC".to_owned())
    );
    assert_eq!(
        destination_asn_fact("15169 "),
        DestinationAsnFact::Reported("15169".to_owned())
    );
}
#[test]
fn spec_converts_to_override_intent() {
    let spec = ConnectionRuleSpec {
        pattern: "DOMAIN-SUFFIX,google.com".to_string(),
        target: "PROXY".to_string(),
    };
    let intent = spec.to_override_intent().unwrap();
    assert_eq!(intent.rule_type, "DOMAIN-SUFFIX");
    assert_eq!(intent.payload, "google.com");
    assert_eq!(intent.target, "PROXY");
    assert_eq!(intent.rule_line(), "DOMAIN-SUFFIX,google.com,PROXY");
}
