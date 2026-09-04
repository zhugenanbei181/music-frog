use super::evaluate_mihomo_connections;

#[test]
fn test_privacy_leak_mihomo_conversion() {
    let mut mihomo_conn = mihomo_api::types::Connection {
        id: "mihomo-c1".to_string(),
        metadata: mihomo_api::types::ConnectionMetadata {
            network: "udp".to_string(),
            connection_type: "TUN".to_string(),
            source_ip: "198.18.0.1".to_string(),
            destination_ip: "8.8.8.8".to_string(),
            source_port: "54321".to_string(),
            destination_port: "53".to_string(),
            host: "".to_string(),
            dns_mode: "fake-ip".to_string(),
            process_path: "/usr/bin/nslookup".to_string(),
            special_proxy: "".to_string(),
        },
        upload: 100,
        download: 200,
        start: "2026-09-01T00:00:00Z".to_string(),
        rule: "DIRECT".to_string(),
        rule_payload: "".to_string(),
        chains: vec!["DIRECT".to_string()],
    };

    let outcome = evaluate_mihomo_connections(&[mihomo_conn.clone()], &[]);
    assert!(outcome.dns_leak);

    mihomo_conn.rule = "ProxyGroup".to_string();
    mihomo_conn.chains = vec!["ProxyGroup".to_string(), "US-Node".to_string()];
    let clean_outcome = evaluate_mihomo_connections(&[mihomo_conn], &[]);
    assert!(clean_outcome.is_clean());
}
