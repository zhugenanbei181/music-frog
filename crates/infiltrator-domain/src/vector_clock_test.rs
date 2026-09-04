use super::*;
use std::collections::HashMap;

// =========================================================================
// 1. VectorClock unit tests
// =========================================================================

#[test]
fn test_vector_clock_new_and_defaults() {
    let vc = VectorClock::new();
    assert!(vc.is_empty());
    assert_eq!(vc.len(), 0);
    assert_eq!(vc.sum_generations(), 0);
    assert_eq!(vc.get("dev-1"), 0);
}

#[test]
fn test_vector_clock_with_device_and_entries() {
    let vc1 = VectorClock::with_device("dev-A", 5);
    assert_eq!(vc1.get("dev-A"), 5);
    assert_eq!(vc1.len(), 1);
    assert_eq!(vc1.sum_generations(), 5);

    let mut map = HashMap::new();
    map.insert("dev-1".to_string(), 10);
    map.insert("dev-2".to_string(), 20);
    let vc2 = VectorClock::with_entries(map, 12345);
    assert_eq!(vc2.get("dev-1"), 10);
    assert_eq!(vc2.get("dev-2"), 20);
    assert_eq!(vc2.updated_at, 12345);
    assert_eq!(vc2.sum_generations(), 30);
}

#[test]
fn test_vector_clock_increment_and_set() {
    let mut vc = VectorClock::new();
    vc.increment("dev-A");
    assert_eq!(vc.get("dev-A"), 1);
    vc.increment("dev-A");
    assert_eq!(vc.get("dev-A"), 2);
    vc.increment("dev-B");
    assert_eq!(vc.get("dev-B"), 1);

    vc.set("dev-A", 10);
    assert_eq!(vc.get("dev-A"), 10);
}

#[test]
fn test_vector_clock_merge() {
    let mut v1 = VectorClock::new();
    v1.set("dev-A", 3);
    v1.set("dev-B", 1);

    let mut v2 = VectorClock::new();
    v2.set("dev-B", 4);
    v2.set("dev-C", 2);

    v1.merge(&v2);
    assert_eq!(v1.get("dev-A"), 3);
    assert_eq!(v1.get("dev-B"), 4);
    assert_eq!(v1.get("dev-C"), 2);
    assert_eq!(v1.sum_generations(), 9);
}

#[test]
fn test_vector_clock_compare_relations() {
    let mut v1 = VectorClock::new();
    v1.set("dev-A", 2);
    v1.set("dev-B", 2);

    let mut v2 = VectorClock::new();
    v2.set("dev-A", 2);
    v2.set("dev-B", 2);
    assert_eq!(v1.compare(&v2), ClockOrdering::Equal);
    assert!(v1.compare(&v2).is_equal());

    // Dominates
    let mut v_ahead = VectorClock::new();
    v_ahead.set("dev-A", 3);
    v_ahead.set("dev-B", 2);
    assert_eq!(v_ahead.compare(&v1), ClockOrdering::Dominates);
    assert!(v_ahead.compare(&v1).dominates());

    // Subordinate
    assert_eq!(v1.compare(&v_ahead), ClockOrdering::Subordinate);
    assert!(v1.compare(&v_ahead).is_subordinate());

    // Concurrent / Conflict
    let mut v_concurrent = VectorClock::new();
    v_concurrent.set("dev-A", 1);
    v_concurrent.set("dev-B", 3);
    assert_eq!(v1.compare(&v_concurrent), ClockOrdering::Concurrent);
    assert!(v1.compare(&v_concurrent).is_concurrent());
    assert!(v1.compare(&v_concurrent).is_conflict());

    // Disjoint keys
    let mut v_disjoint = VectorClock::new();
    v_disjoint.set("dev-C", 1);
    assert_eq!(v1.compare(&v_disjoint), ClockOrdering::Concurrent);
}

#[test]
fn test_vector_clock_serde_roundtrip() {
    let mut vc = VectorClock::new();
    vc.set("node-alpha", 42);
    vc.set("node-beta", 7);

    let json = serde_json::to_string(&vc).expect("json serialize");
    let deserialized: VectorClock = serde_json::from_str(&json).expect("json deserialize");
    assert_eq!(vc, deserialized);

    let yaml = serde_yaml_ng::to_string(&vc).expect("yaml serialize");
    let deserialized_yaml: VectorClock = serde_yaml_ng::from_str(&yaml).expect("yaml deserialize");
    assert_eq!(vc, deserialized_yaml);
}

// =========================================================================
// 2. VersionedDocument unit tests
// =========================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TestProfileConfig {
    name: String,
    port: u16,
}

#[test]
fn test_versioned_document_lifecycle() {
    let cfg = TestProfileConfig {
        name: "Profile-1".to_string(),
        port: 7890,
    };

    let mut doc = VersionedDocument::new(cfg, "actor-1");
    assert_eq!(doc.actor_id(), "actor-1");
    assert_eq!(doc.clock().get("actor-1"), 1);
    assert_eq!(doc.data().name, "Profile-1");
    assert_eq!(doc.data().port, 7890);

    // Direct mutation through data_mut
    doc.data_mut().port = 8080;
    assert_eq!(doc.data().port, 8080);

    // Update through document helper
    let updated_cfg = TestProfileConfig {
        name: "Profile-Updated".to_string(),
        port: 9090,
    };
    doc.update(updated_cfg, "actor-1");
    assert_eq!(doc.clock().get("actor-1"), 2);
    assert_eq!(doc.data().name, "Profile-Updated");
    assert_eq!(doc.data().port, 9090);

    // Update by another actor
    let updated_by_actor2 = TestProfileConfig {
        name: "Profile-Actor2".to_string(),
        port: 9999,
    };
    doc.update(updated_by_actor2, "actor-2");
    assert_eq!(doc.clock().get("actor-1"), 2);
    assert_eq!(doc.clock().get("actor-2"), 1);
    assert_eq!(doc.actor_id(), "actor-2");

    let inner = doc.into_inner();
    assert_eq!(inner.port, 9999);
}

#[test]
fn test_versioned_document_with_clock_and_comparison() {
    let doc1 =
        VersionedDocument::with_clock("val1", "actor-1", VectorClock::with_device("actor-1", 3));
    let doc2 =
        VersionedDocument::with_clock("val2", "actor-1", VectorClock::with_device("actor-1", 5));

    assert_eq!(doc1.compare(&doc2), ClockOrdering::Subordinate);
    assert_eq!(doc2.compare(&doc1), ClockOrdering::Dominates);
}

#[test]
fn test_versioned_document_serialization() {
    let doc = VersionedDocument::new("payload".to_string(), "node-x");
    let json = serde_json::to_string(&doc).expect("serialize doc");
    let deserialized: VersionedDocument<String> =
        serde_json::from_str(&json).expect("deserialize doc");
    assert_eq!(doc, deserialized);
}

// =========================================================================
// 3. 3-Way Merge Engine unit tests
// =========================================================================

#[test]
fn test_merge_3way_clean_local_only_addition() {
    let base = "port: 7890\n";
    let local = "port: 7890\nallow-lan: true\n";
    let remote = "port: 7890\n";

    let res = merge_3way(base, local, remote);
    assert!(res.is_clean());
    assert!(!res.has_conflicts());
    assert!(res.conflicts.is_empty());
    assert!(res.merged_yaml().contains("port: 7890"));
    assert!(res.merged_yaml().contains("allow-lan: true"));
    assert_eq!(res.merged_content(), res.merged_yaml());

    assert!(
        res.chunks
            .iter()
            .any(|c| matches!(c, DiffChunk::LocalOnly { key, .. } if key == "allow-lan"))
    );
}

#[test]
fn test_merge_3way_clean_remote_only_addition() {
    let base = "port: 7890\n";
    let local = "port: 7890\n";
    let remote = "port: 7890\nmode: rule\n";

    let res = merge_3way(base, local, remote);
    assert!(res.is_clean());
    assert!(res.merged_yaml().contains("port: 7890"));
    assert!(res.merged_yaml().contains("mode: rule"));
    assert!(
        res.chunks
            .iter()
            .any(|c| matches!(c, DiffChunk::RemoteOnly { key, .. } if key == "mode"))
    );
}

#[test]
fn test_merge_3way_clean_non_conflicting_scalar_modifications() {
    let base = "port: 7890\nmode: rule\nlog-level: info\n";
    let local = "port: 7890\nmode: global\nlog-level: info\n"; // local changed mode
    let remote = "port: 8080\nmode: rule\nlog-level: info\n"; // remote changed port

    let res = merge_3way(base, local, remote);
    assert!(res.is_clean());
    assert!(res.conflicts.is_empty());
    assert!(res.merged_yaml().contains("port: 8080"));
    assert!(res.merged_yaml().contains("mode: global"));
    assert!(res.merged_yaml().contains("log-level: info"));
}

#[test]
fn test_merge_3way_clean_both_same_modification() {
    let base = "port: 7890\nmode: rule\n";
    let local = "port: 8080\nmode: global\n";
    let remote = "port: 8080\nmode: global\n";

    let res = merge_3way(base, local, remote);
    assert!(res.is_clean());
    assert!(res.merged_yaml().contains("port: 8080"));
    assert!(res.merged_yaml().contains("mode: global"));
}

#[test]
fn test_merge_3way_clean_key_deletion() {
    let base = "port: 7890\nsocks-port: 7891\nmode: rule\n";
    let local = "port: 7890\nmode: rule\n"; // local deleted socks-port
    let remote = "port: 7890\nsocks-port: 7891\nmode: rule\n";

    let res = merge_3way(base, local, remote);
    assert!(res.is_clean());
    assert!(!res.merged_yaml().contains("socks-port"));
    assert!(res.merged_yaml().contains("port: 7890"));

    // Remote deleted
    let local2 = "port: 7890\nsocks-port: 7891\n";
    let remote2 = "port: 7890\n";
    let res2 = merge_3way(base, local2, remote2);
    assert!(res2.is_clean());
    assert!(!res2.merged_yaml().contains("socks-port"));
}

#[test]
fn test_merge_3way_nested_mapping_field_level_resolution() {
    let base = "\
dns:
  enable: true
  ipv6: false
  listen: 0.0.0.0:53
";
    let local = "\
dns:
  enable: true
  ipv6: true
  listen: 0.0.0.0:53
";
    let remote = "\
dns:
  enable: true
  ipv6: false
  listen: 127.0.0.1:53
";

    let res = merge_3way(base, local, remote);
    assert!(
        res.is_clean(),
        "Nested mapping edits on different fields should merge cleanly"
    );
    assert!(res.conflicts.is_empty());

    let parsed: serde_yaml_ng::Value = serde_yaml_ng::from_str(res.merged_yaml()).unwrap();
    let dns = parsed.get("dns").unwrap().as_mapping().unwrap();

    let enable = dns
        .get(serde_yaml_ng::Value::String("enable".into()))
        .unwrap();
    let ipv6 = dns
        .get(serde_yaml_ng::Value::String("ipv6".into()))
        .unwrap();
    let listen = dns
        .get(serde_yaml_ng::Value::String("listen".into()))
        .unwrap();

    assert_eq!(enable.as_bool(), Some(true));
    assert_eq!(ipv6.as_bool(), Some(true)); // from local
    assert_eq!(listen.as_str(), Some("127.0.0.1:53")); // from remote
}

#[test]
fn test_merge_3way_sequence_append_both_clean() {
    let base = "\
rules:
  - MATCH,DIRECT
";
    let local = "\
rules:
  - MATCH,DIRECT
  - DOMAIN,local.com,PROXY
";
    let remote = "\
rules:
  - MATCH,DIRECT
  - DOMAIN,remote.com,DIRECT
";

    let res = merge_3way(base, local, remote);
    assert!(res.is_clean());
    assert!(res.merged_yaml().contains("DOMAIN,local.com,PROXY"));
    assert!(res.merged_yaml().contains("DOMAIN,remote.com,DIRECT"));
}

#[test]
fn test_merge_3way_scalar_conflict() {
    let base = "port: 7890\n";
    let local = "port: 8080\n";
    let remote = "port: 9090\n";

    let res = merge_3way(base, local, remote);
    assert!(!res.is_clean());
    assert!(res.has_conflicts());
    assert_eq!(res.conflicts.len(), 1);

    let conflict = &res.conflicts[0];
    assert_eq!(conflict.key, "port");
    assert_eq!(conflict.base.as_deref(), Some("7890"));
    assert_eq!(conflict.local, "8080");
    assert_eq!(conflict.remote, "9090");

    assert!(
        res.chunks
            .iter()
            .any(|c| c.is_conflict() && c.key() == "port")
    );
}

#[test]
fn test_merge_3way_modified_vs_deleted_conflict() {
    let base = "port: 7890\n";
    let local = "port: 8080\n";
    let remote = "\n"; // remote deleted port

    let res = merge_3way(base, local, remote);
    assert!(!res.is_clean());
    assert_eq!(res.conflicts.len(), 1);
    assert_eq!(res.conflicts[0].key, "port");
}

#[test]
fn test_merge_3way_malformed_yaml() {
    let base = "port: 7890\n";
    let local = "port: [invalid, yaml:\n";
    let remote = "port: 8080\n";

    let res = merge_3way(base, local, remote);
    assert!(!res.is_clean());
    assert_eq!(res.conflicts[0].key, "$root");
}

#[test]
fn test_merge_3way_empty_inputs() {
    let res = merge_3way("", "", "");
    assert!(res.is_clean());
    assert_eq!(res.merged_yaml().trim(), "");
}

// =========================================================================
// 4. P2P Pairing and Verification unit tests
// =========================================================================

#[test]
fn test_p2p_pairing_code_generator() {
    let code = P2pPairingHelper::generate_pairing_code();
    assert_eq!(code.len(), 6);
    assert!(code.chars().all(|c| c.is_ascii_digit()));
}

#[test]
fn test_p2p_token_generator_and_verification() {
    let secret = "test-cluster-secret";
    let device_id = "device-alpha";
    let expires_at = 2000000000;

    let token = P2pPairingHelper::generate_token(secret, device_id, expires_at);
    assert!(!token.is_empty());

    // Valid verification
    let current_time = 1999999990;
    let res = P2pPairingHelper::verify_token(&token, secret, device_id, current_time, expires_at);
    assert!(res.is_ok());

    // Expired verification
    let expired_time = 2000000001;
    let err_exp =
        P2pPairingHelper::verify_token(&token, secret, device_id, expired_time, expires_at);
    assert_eq!(err_exp, Err(PairingError::Expired));

    // Invalid secret or device
    let err_tok =
        P2pPairingHelper::verify_token(&token, "wrong-secret", device_id, current_time, expires_at);
    assert_eq!(err_tok, Err(PairingError::InvalidToken));
}

#[test]
fn test_p2p_create_payload_and_uri_roundtrip() {
    let endpoint = "192.168.1.120:9099";
    let cert_fingerprint = "SHA256:1122334455667788";
    let ttl_secs = 600;
    let device_id = Some("device-macbook".to_string());
    let sync_scope = SyncScope::SelectedProfiles(vec!["work".to_string(), "gaming".to_string()]);
    let secret = Some("pairing-key-xyz");

    let payload = P2pPairingHelper::create_payload(
        endpoint,
        cert_fingerprint,
        ttl_secs,
        device_id.clone(),
        sync_scope.clone(),
        secret,
    );

    assert_eq!(payload.endpoint, endpoint);
    assert_eq!(payload.cert_fingerprint, cert_fingerprint);
    assert_eq!(payload.device_id, device_id);
    assert_eq!(payload.sync_scope, sync_scope);
    assert!(payload.token.is_some());

    let uri = P2pPairingHelper::format_pairing_uri(&payload);
    assert!(uri.starts_with("infiltrator-p2p://192.168.1.120:9099"));
    assert!(uri.contains(&format!("code={}", payload.pairing_code)));
    assert!(uri.contains("fp=SHA256:1122334455667788"));
    assert!(uri.contains("scope=profiles:work,gaming"));

    let parsed = P2pPairingHelper::parse_pairing_uri(&uri).expect("parse URI");
    assert_eq!(parsed.endpoint, payload.endpoint);
    assert_eq!(parsed.pairing_code, payload.pairing_code);
    assert_eq!(parsed.cert_fingerprint, payload.cert_fingerprint);
    assert_eq!(parsed.expires_at, payload.expires_at);
    assert_eq!(parsed.device_id, payload.device_id);
    assert_eq!(parsed.sync_scope, payload.sync_scope);
    assert_eq!(parsed.token, payload.token);
}

#[test]
fn test_p2p_parse_uri_scopes() {
    let base_payload = P2pPairingPayload {
        pairing_code: "123456".to_string(),
        endpoint: "10.0.0.5:8000".to_string(),
        cert_fingerprint: "FP".to_string(),
        expires_at: 1000,
        device_id: None,
        sync_scope: SyncScope::SubscriptionsOnly,
        token: None,
    };
    let uri_subs = P2pPairingHelper::format_pairing_uri(&base_payload);
    let parsed_subs = P2pPairingHelper::parse_pairing_uri(&uri_subs).unwrap();
    assert_eq!(parsed_subs.sync_scope, SyncScope::SubscriptionsOnly);

    let mut rules_payload = base_payload.clone();
    rules_payload.sync_scope = SyncScope::CustomRulesOnly;
    let uri_rules = P2pPairingHelper::format_pairing_uri(&rules_payload);
    let parsed_rules = P2pPairingHelper::parse_pairing_uri(&uri_rules).unwrap();
    assert_eq!(parsed_rules.sync_scope, SyncScope::CustomRulesOnly);
}

#[test]
fn test_p2p_verify_pairing_code() {
    let payload = P2pPairingPayload {
        pairing_code: "654321".to_string(),
        endpoint: "10.0.0.1:9999".to_string(),
        cert_fingerprint: "SHA256:DEADBEEF".to_string(),
        expires_at: 10000,
        device_id: None,
        sync_scope: SyncScope::Full,
        token: None,
    };

    // Valid code and not expired
    assert!(P2pPairingHelper::verify_pairing_code(&payload, "654321", 5000).is_ok());

    // Wrong code
    assert_eq!(
        P2pPairingHelper::verify_pairing_code(&payload, "000000", 5000),
        Err(PairingError::InvalidCode)
    );

    // Expired
    assert_eq!(
        P2pPairingHelper::verify_pairing_code(&payload, "654321", 10001),
        Err(PairingError::Expired)
    );
}

#[test]
fn test_p2p_parse_uri_errors() {
    assert!(matches!(
        P2pPairingHelper::parse_pairing_uri("http://localhost"),
        Err(PairingError::InvalidUri(_))
    ));
    assert!(matches!(
        P2pPairingHelper::parse_pairing_uri("infiltrator-p2p://?code=123"),
        Err(PairingError::InvalidEndpoint(_))
    ));
    assert!(matches!(
        P2pPairingHelper::parse_pairing_uri("infiltrator-p2p://192.168.1.1:8000?fp=abc"),
        Err(PairingError::MissingField(f)) if f == "code"
    ));
    assert!(matches!(
        P2pPairingHelper::parse_pairing_uri("infiltrator-p2p://192.168.1.1:8000?code=123"),
        Err(PairingError::MissingField(f)) if f == "fp"
    ));
}
