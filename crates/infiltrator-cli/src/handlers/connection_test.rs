use infiltrator_domain::runtime::{Connection, ConnectionMetadata};

use super::{ListFilters, apply_filters, process_name, short_id};

fn connection(id: &str, host: &str, process_path: &str, rule: &str) -> Connection {
    Connection {
        id: id.to_string(),
        metadata: ConnectionMetadata {
            network: "tcp".to_string(),
            connection_type: "HTTP".to_string(),
            source_ip: "192.168.1.1".to_string(),
            destination_ip: "1.1.1.1".to_string(),
            source_port: "12345".to_string(),
            destination_port: "443".to_string(),
            host: host.to_string(),
            dns_mode: "normal".to_string(),
            process_path: process_path.to_string(),
            special_proxy: String::new(),
        },
        upload: 1,
        download: 2,
        start: "2026-01-01T00:00:00Z".to_string(),
        chains: vec!["DIRECT".to_string()],
        rule: rule.to_string(),
        rule_payload: String::new(),
    }
}

#[test]
fn filters_combine_with_and_semantics() {
    let connections = vec![
        connection("1", "google.com", "/usr/bin/chrome", "DIRECT"),
        connection("2", "github.com", "/usr/bin/git", "PROXY"),
        connection("3", "googleapis.com", "/usr/bin/git", "PROXY"),
    ];

    let none = ListFilters {
        host: None,
        process: None,
        rule: None,
    };
    assert_eq!(apply_filters(connections.clone(), &none).len(), 3);

    let host = ListFilters {
        host: Some("google".to_string()),
        process: None,
        rule: None,
    };
    assert_eq!(apply_filters(connections.clone(), &host).len(), 2);

    let host_and_process = ListFilters {
        host: Some("google".to_string()),
        process: Some("git".to_string()),
        rule: None,
    };
    let matched = apply_filters(connections.clone(), &host_and_process);
    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0].id, "3");

    let rule = ListFilters {
        host: None,
        process: None,
        rule: Some("DIRECT".to_string()),
    };
    let matched = apply_filters(connections, &rule);
    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0].id, "1");
}

#[test]
fn filter_without_matches_yields_empty() {
    let connections = vec![connection("1", "google.com", "/usr/bin/chrome", "DIRECT")];
    let filters = ListFilters {
        host: Some("baidu".to_string()),
        process: None,
        rule: None,
    };
    assert!(apply_filters(connections, &filters).is_empty());
}

#[test]
fn short_id_keeps_only_the_leading_fragment() {
    assert_eq!(short_id("0123456789abcdef"), "01234567");
    assert_eq!(short_id("abc"), "abc");
}

#[test]
fn process_name_strips_the_directories() {
    assert_eq!(process_name("/usr/bin/google-chrome"), "google-chrome");
    assert_eq!(process_name("C:\\Program Files\\app.exe"), "app.exe");
    assert_eq!(process_name("plain"), "plain");
}
