//! test-intent: behavior
use super::*;
use infiltrator_domain::proxy::{ProxyBase, ProxyGroup, ProxyHistory, Shadowsocks};
use infiltrator_domain::proxy_observation::RuntimeProxyObservation;
use std::collections::BTreeMap;

#[test]
fn controller_observations_keep_unreported_metadata_and_flags_unknown_in_both_peer_folds() {
    let facts: RuntimeProxyObservation = serde_json::from_str(r#"{"name":"leaf","type":"Shadowsocks","history":[{"time":"old","delay":42},{"time":"new","delay":0}]}"#).unwrap();
    let detail = project_proxy_inspection("leaf", &Proxy::Observed(facts));
    assert_eq!(detail.server, None);
    assert_eq!(detail.port, None);
    assert_eq!(detail.cipher, None);
    assert_eq!(detail.alive, None);
    assert_eq!(detail.udp, None);
    assert_eq!(detail.delay_ms, Some(0));
    assert_eq!(detail.rtt.valid_count, 1);
    assert!(detail.can_probe);
    for field in [
        ProxyDetailField::Server,
        ProxyDetailField::Port,
        ProxyDetailField::Cipher,
        ProxyDetailField::Health,
        ProxyDetailField::Udp,
    ] {
        assert_eq!(
            field_value(&detail, field, &str::to_owned),
            "proxy_inspection_unknown"
        );
    }
    assert_eq!(
        field_value(&detail, ProxyDetailField::Delay, &str::to_owned),
        "proxy_inspection_unconfirmed_zero"
    );
}

#[test]
fn details_preserve_observed_metadata_unresolved_zero_and_history_without_inventing_stage_timings()
{
    let proxy = Proxy::Shadowsocks(Shadowsocks {
        server: "node.example.test".into(),
        port: 443,
        cipher: "aes-256-gcm".into(),
        base: ProxyBase {
            alive: false,
            udp: true,
            delay: Some(0),
            history: vec![
                ProxyHistory {
                    time: "old".into(),
                    delay: 24,
                },
                ProxyHistory {
                    time: "new".into(),
                    delay: 0,
                },
            ],
            ..Default::default()
        },
        ..Default::default()
    });
    let detail = project_proxy_inspection("stable", &proxy);
    assert_eq!(detail.server.as_deref(), Some("node.example.test"));
    assert_eq!(detail.port, Some(443));
    assert_eq!(detail.cipher.as_deref(), Some("aes-256-gcm"));
    assert_eq!(detail.alive, Some(false));
    assert_eq!(detail.udp, Some(true));
    assert_eq!(detail.delay_ms, Some(0));
    assert_eq!(
        detail
            .history
            .iter()
            .map(|sample| (sample.time.as_str(), sample.delay_ms))
            .collect::<Vec<_>>(),
        vec![("old", 24), ("new", 0)]
    );
    assert_eq!(detail.history_total, 2);
    let plot = history_plot(&detail);
    assert_eq!(plot.ceiling_ms, 24.0);
    assert_eq!(plot.samples[0], 24.0);
    assert!(plot.samples[1].is_nan());
    assert_eq!(
        (
            detail.rtt.min_ms,
            detail.rtt.max_ms,
            detail.rtt.avg_ms,
            detail.rtt.valid_count
        ),
        (Some(24), Some(24), Some(24), 1)
    );
    assert!(matches!(detail.timing, Availability::Unsupported { .. }));
    assert_eq!(
        field_value(&detail, ProxyDetailField::Delay, &str::to_owned),
        "proxy_inspection_unconfirmed_zero"
    );
    assert!(
        history_listing(&detail, &str::to_owned)
            .contains("new · proxy_inspection_unconfirmed_zero")
    );
}

#[test]
fn egress_comes_only_from_an_explicit_matching_probe_record_and_stale_or_invalid_values_clear() {
    let proxy = Proxy::Shadowsocks(Shadowsocks::default());
    let mut detail = project_proxy_inspection("recorded", &proxy);
    let mut snapshot = SpeedtestSnapshot::demo_fixture();
    let mut record = snapshot.node_results.values().next().unwrap().clone();
    record.node_name = "recorded".into();
    record.proxy_type = "Shadowsocks".into();
    record.outbound_ip = Some("203.0.113.7".into());
    record.outbound_country = Some("US".into());
    record.tested_at_epoch_ms = 1_700_000_000_000;
    snapshot.node_results = BTreeMap::from([("recorded".into(), record)]);
    with_egress_record(&mut detail, &snapshot);
    assert_eq!(detail.egress_ip.as_deref(), Some("203.0.113.7"));
    assert_eq!(
        field_value(&detail, ProxyDetailField::Egress, &str::to_owned),
        "203.0.113.7 (US)"
    );
    assert_eq!(
        field_value(&detail, ProxyDetailField::EgressTime, &str::to_owned),
        "2023-11-14T22:13:20+00:00"
    );
    snapshot
        .node_results
        .get_mut("recorded")
        .unwrap()
        .proxy_type = "VLESS".into();
    with_egress_record(&mut detail, &snapshot);
    assert_eq!(
        (
            detail.egress_ip.clone(),
            detail.egress_country.clone(),
            detail.egress_recorded_at_epoch_ms
        ),
        (None, None, None)
    );
    let record = snapshot.node_results.get_mut("recorded").unwrap();
    record.proxy_type = "Shadowsocks".into();
    record.outbound_ip = Some("invalid-address".into());
    with_egress_record(&mut detail, &snapshot);
    assert_eq!(detail.egress_ip, None);
    snapshot.node_results.clear();
    with_egress_record(&mut detail, &snapshot);
    assert_eq!(
        field_value(&detail, ProxyDetailField::Egress, &str::to_owned),
        "proxy_inspection_unknown"
    );
}

#[test]
fn sparse_unknown_and_group_observations_do_not_make_up_ports_health_udp_or_history() {
    for proxy in [Proxy::Unknown, Proxy::Selector(ProxyGroup::default())] {
        let detail = project_proxy_inspection("missing", &proxy);
        let plot = history_plot(&detail);
        assert_eq!(
            (detail.server, detail.port, detail.cipher),
            (None, None, None)
        );
        assert_eq!(
            (detail.alive, detail.udp, detail.delay_ms),
            (None, None, None)
        );
        assert!(detail.history.is_empty());
        assert!(plot.samples.is_empty());
        assert_eq!(plot.ceiling_ms, 1.0);
    }
    let sparse = inspect_sparse_node(&ProxyNodeSnapshot {
        name: "sparse".into(),
        node_type: "VLESS".into(),
        delay_ms: Some(45),
        alive: None,
        selected: true,
        favorite: false,
        features: vec!["Reality".into()],
    });
    assert_eq!(sparse.delay_ms, Some(45));
    assert_eq!(sparse.alive, None);
    assert_eq!(sparse.udp, None);
    assert!(sparse.history.is_empty());
    assert_eq!(
        field_value(&sparse, ProxyDetailField::Port, &str::to_owned),
        "proxy_inspection_unknown"
    );
}

#[test]
fn recent_history_is_bounded_in_controller_order_and_the_catalogue_uses_stable_names() {
    let proxy = Proxy::Shadowsocks(Shadowsocks {
        base: ProxyBase {
            history: (0..80)
                .map(|index| ProxyHistory {
                    time: index.to_string(),
                    delay: index,
                })
                .collect(),
            ..Default::default()
        },
        ..Default::default()
    });
    let detail = project_proxy_inspection("z", &proxy);
    assert_eq!(detail.history_total, 80);
    assert_eq!(detail.history.len(), INSPECTION_HISTORY_LIMIT);
    assert_eq!(detail.history.first().unwrap().time, "16");
    assert_eq!(detail.history.last().unwrap().time, "79");
    let catalogue = inspection_catalogue(&HashMap::from([
        ("z".into(), proxy),
        ("a".into(), Proxy::Unknown),
    ]));
    assert_eq!(
        catalogue
            .iter()
            .map(|detail| detail.name.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "z"]
    );
}
