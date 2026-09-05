use super::*;
use infiltrator_domain::runtime::{Connection, ConnectionMetadata};

fn make_test_conn(id: &str, host: &str, process: &str, up: u64, down: u64) -> Connection {
    Connection {
        id: id.to_string(),
        metadata: ConnectionMetadata {
            network: "tcp".to_string(),
            connection_type: "TLS".to_string(),
            source_ip: "192.168.1.50".to_string(),
            destination_ip: "1.1.1.1".to_string(),
            source_port: "50000".to_string(),
            destination_port: "443".to_string(),
            host: host.to_string(),
            dns_mode: "fake-ip".to_string(),
            process_path: process.to_string(),
            special_proxy: String::new(),
        },
        upload: up,
        download: down,
        start: "2026-09-01T12:00:00Z".to_string(),
        rule: "DomainSuffix".to_string(),
        rule_payload: "example.com".to_string(),
        chains: vec!["DMIT".to_string(), "PROXY".to_string()],
    }
}

#[test]
fn test_extract_process_name() {
    assert_eq!(extract_process_name("/usr/bin/firefox"), "firefox");
    assert_eq!(extract_process_name("C:\\Program Files\\Zed\\zed-editor.exe"), "zed-editor");
    assert_eq!(extract_process_name("zed-editor"), "zed-editor");
    assert_eq!(extract_process_name(""), "");
    assert_eq!(extract_process_name("   "), "");
}

#[test]
fn test_outbound_target_info() {
    let mut conn = make_test_conn("1", "google.com", "", 100, 200);
    let (target, kind) = outbound_target_info(&conn);
    assert_eq!(target, "DMIT");
    assert_eq!(kind, BadgeKind::Accent);

    conn.chains = vec!["DIRECT".to_string()];
    let (target, kind) = outbound_target_info(&conn);
    assert_eq!(target, "DIRECT");
    assert_eq!(kind, BadgeKind::Success);

    conn.chains = vec!["REJECT".to_string()];
    let (target, kind) = outbound_target_info(&conn);
    assert_eq!(target, "REJECT");
    assert_eq!(kind, BadgeKind::Danger);
}

#[test]
fn test_filter_connection() {
    let conn = make_test_conn("c1", "api.openai.com", "/usr/bin/chromium", 100, 200);
    assert!(filter_connection(&conn, ""));
    assert!(filter_connection(&conn, "openai"));
    assert!(filter_connection(&conn, "chromium"));
    assert!(filter_connection(&conn, "DMIT"));
    assert!(!filter_connection(&conn, "nonexistent"));
}

#[test]
fn test_sort_connections() {
    let mut conns = vec![
        make_test_conn("1", "b.com", "", 100, 500),
        make_test_conn("2", "a.com", "", 900, 200),
        make_test_conn("3", "c.com", "", 500, 800),
    ];

    sort_connections(&mut conns, "download_desc");
    assert_eq!(conns[0].id, "3");
    assert_eq!(conns[1].id, "1");
    assert_eq!(conns[2].id, "2");

    sort_connections(&mut conns, "upload_desc");
    assert_eq!(conns[0].id, "2");
    assert_eq!(conns[1].id, "3");
    assert_eq!(conns[2].id, "1");

    sort_connections(&mut conns, "host_asc");
    assert_eq!(conns[0].id, "2");
    assert_eq!(conns[1].id, "1");
    assert_eq!(conns[2].id, "3");
}

#[test]
fn test_stream_badge_kinds() {
    let _elem_idle: Element<'_, Message> = stream_badge(&RuntimeStreamState::Idle, &Lang("zh-CN"));
    let _elem_connected: Element<'_, Message> = stream_badge(&RuntimeStreamState::Connected, &Lang("zh-CN"));
    let _elem_failed: Element<'_, Message> = stream_badge(&RuntimeStreamState::Failed("err".into()), &Lang("zh-CN"));
}
