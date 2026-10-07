//! Behavior cases for ssh.
//! test-intent: behavior

use super::*;
use std::slice::from_ref;

#[test]
fn test_ssh_node_is_typed_and_validated() {
    let node = parse_single(SSH_YAML);
    let ProxyNode::Ssh(ssh) = &node else {
        panic!("ssh node degraded to Other: {node:?}");
    };
    assert_eq!(node.type_name(), "ssh");
    assert_eq!(ssh.username, "root");
    assert_eq!(
        ssh.private_key.as_deref(),
        Some("-----BEGIN OPENSSH PRIVATE KEY-----")
    );
    assert_eq!(ssh.private_key_passphrase.as_deref(), Some("phrase"));
    assert_eq!(
        ssh.host_key_algorithms,
        Some(vec!["ssh-ed25519".to_string()])
    );
    assert_eq!(ssh.dialer_proxy.as_deref(), Some("hop"));
    assert_eq!(ssh.extra.get("fake-field"), Some(&Value::Number(5.into())));
    assert!(node.is_typed());
    assert!(validate(&node).is_empty(), "{:?}", validate(&node));

    assert_proxies_semantic_equivalence(SSH_YAML);
    assert_roundtrip_fixed_point(from_ref(&node));

    // Missing username / identity is reported, never invented.
    let missing = r#"
proxies:
  - name: ssh-bad
    type: ssh
    server: 203.0.113.91
    port: 22
"#;
    let node = parse_single(missing);
    let ProxyNode::Ssh(_) = &node else {
        panic!("ssh node degraded to Other: {node:?}");
    };
    let issues = validate(&node);
    assert!(
        issues.iter().any(|issue| issue.contains("username")),
        "{issues:?}"
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("password or private-key")),
        "{issues:?}"
    );

    // A password-only SSH node is valid.
    let password_only = r#"
proxies:
  - name: ssh-pass
    type: ssh
    server: 203.0.113.92
    port: 22
    username: root
    password: pw
"#;
    let node = parse_single(password_only);
    assert!(validate(&node).is_empty(), "{:?}", validate(&node));
}
