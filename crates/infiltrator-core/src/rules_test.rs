use super::*;
use infiltrator_domain::rules::{format_rule_entry, parse_rule_entry};
use std::collections::BTreeMap;

#[test]
fn test_parse_rule_entry() {
    let entry = parse_rule_entry("DOMAIN,example.com");
    assert!(entry.enabled);
    assert_eq!(entry.rule, "DOMAIN,example.com");

    let entry = parse_rule_entry("  DOMAIN,example.com  ");
    assert!(entry.enabled);
    assert_eq!(entry.rule, "DOMAIN,example.com  ");

    let entry = parse_rule_entry("# DIRECT");
    assert!(!entry.enabled);
    assert_eq!(entry.rule, "DIRECT");

    let entry = parse_rule_entry(" #  MATCH,Proxy ");
    assert!(!entry.enabled);
    assert_eq!(entry.rule, "MATCH,Proxy ");
}

#[test]
fn test_format_rule_entry() {
    let entry = RuleEntry {
        rule: "DIRECT".to_string(),
        enabled: false,
    };
    assert_eq!(format_rule_entry(&entry), "# DIRECT");

    let entry = RuleEntry {
        rule: "DOMAIN,google.com".to_string(),
        enabled: true,
    };
    assert_eq!(format_rule_entry(&entry), "DOMAIN,google.com");
}

#[test]
fn test_apply_rules_preserves_order() {
    let initial = "rules:\n  - DOMAIN,initial.com\n";
    let rules = vec![
        RuleEntry {
            rule: "DOMAIN,second.com".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "MATCH".to_string(),
            enabled: false,
        },
    ];

    let result = apply_rules_to_yaml(initial, &rules).expect("apply rules");
    assert!(result.contains("DOMAIN,second.com"));
    assert!(result.contains("# MATCH"));
}

#[test]
fn test_apply_rules_empty_removes() {
    let initial = "rules:\n  - DOMAIN,test.com\n";
    let result = apply_rules_to_yaml(initial, &[]).expect("apply empty rules");
    assert!(!result.contains("rules:"));
}

#[test]
fn test_apply_rule_providers_empty_removes() {
    let initial = "rule-providers:\n  custom:\n    type: http\n";
    let providers = BTreeMap::new();
    let result =
        apply_rule_providers_to_yaml(initial, &providers).expect("apply empty rule providers");
    assert!(!result.contains("rule-providers:"));
}

#[tokio::test]
async fn test_rules_io_follows_configs_dir_redirect() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().to_path_buf();
    let cloud = home.join("cloud-sync").join("profiles");
    std::fs::create_dir_all(&cloud).unwrap();
    let guard = crate::settings::test_support::RedirectGuard::acquire(home.clone()).await;
    guard
        .set_configs_dir(&home, Some(cloud.to_str().unwrap()))
        .await;

    let seed = crate::settings::app_config_manager().await.unwrap();
    seed.save("main", "port: 7890\n").await.unwrap();
    seed.set_current("main").await.unwrap();

    let rules = vec![RuleEntry {
        rule: "DOMAIN,test.com".to_string(),
        enabled: true,
    }];
    save_rules(rules).await.unwrap();

    let loaded = load_rules().await.unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].rule, "DOMAIN,test.com");
    assert!(cloud.join("main.yaml").is_file());
    assert!(!home.join("configs").exists());
}
