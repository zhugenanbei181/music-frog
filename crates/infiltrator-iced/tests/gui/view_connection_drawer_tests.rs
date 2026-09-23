//! DUAL-13-05: kernel-provided target ASN / geolocation rendering tests.

use super::*;

#[test]
fn test_kernel_asn_label_separates_not_evaluated_from_no_result() {
    let zh = Lang("zh-CN");
    let en = Lang("en");

    // The kernel sent `""`: no IP-ASN rule ran for this connection.
    assert_eq!(
        kernel_asn_label("", &zh),
        "内核未对本次连接求值（需 GEOIP/IP-ASN 规则）"
    );
    // It sent `" "` (number + space + organization): it evaluated and had no
    // record for the IP.
    assert_eq!(kernel_asn_label(" ", &zh), "内核已求值 · 无该 IP 的记录");
    // A real kernel value is rendered verbatim: the client adds no `AS` prefix
    // and never substitutes an ASN of its own.
    assert_eq!(
        kernel_asn_label("15169 Google LLC", &zh),
        "15169 Google LLC"
    );
    assert_eq!(
        kernel_asn_label("15169 Google LLC", &en),
        "15169 Google LLC"
    );
}

#[test]
fn test_kernel_geo_label_keeps_the_kernels_three_states() {
    let zh = Lang("zh-CN");

    // `null`: the kernel never queried a GEOIP rule.
    assert_eq!(
        kernel_geo_label(None, &zh),
        "内核未对本次连接求值（需 GEOIP/IP-ASN 规则）"
    );
    // `[]`: queried with no record.
    assert_eq!(
        kernel_geo_label(Some(&[]), &zh),
        "内核已求值 · 无该 IP 的记录"
    );
    // Real codes: rendered in kernel order with no client-side mapping.
    let codes = vec!["us".to_owned(), "cloudflare".to_owned()];
    assert_eq!(kernel_geo_label(Some(&codes), &zh), "us, cloudflare");
}

#[test]
fn test_drawer_modal_renders_every_connection_without_fabricating() {
    use infiltrator_domain::runtime::{Connection, ConnectionMetadata};

    let (mut state, _) = AppState::new();
    state.diag.connections = Some(infiltrator_domain::runtime::ConnectionSnapshot {
        connections: vec![
            Connection {
                id: "c-kernel".to_owned(),
                metadata: ConnectionMetadata {
                    destination_ip: "142.250.72.14".to_owned(),
                    destination_port: "443".to_owned(),
                    destination_ip_asn: "15169 Google LLC".to_owned(),
                    destination_geo_ip: Some(vec!["us".to_owned()]),
                    ..ConnectionMetadata::default()
                },
                ..Connection::default()
            },
            Connection {
                id: "c-silent".to_owned(),
                metadata: ConnectionMetadata::default(),
                ..Connection::default()
            },
        ],
        ..infiltrator_domain::runtime::ConnectionSnapshot::default()
    });

    // Rendering both connections must not panic and must not require any
    // client-side GeoIP database.
    let _kernel = connection_drawer_modal(&state, "c-kernel");
    let _silent = connection_drawer_modal(&state, "c-silent");
    // An unknown id stays an empty placeholder, not a fabricated drawer.
    let _missing = connection_drawer_modal(&state, "c-missing");
}
