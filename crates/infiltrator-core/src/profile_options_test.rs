//! Behavior tests for the per-profile option sidecar pipeline.

use super::*;
use infiltrator_domain::filter::{
    ContentDedupStrategy, DeduplicationStrategy, NodeMutatorConfig, NodeSortOrder,
    SubscriptionFilterPipeline,
};
use infiltrator_domain::mixin::MixinConfig;
use infiltrator_domain::profile_options::{
    FilterDedup, FilterSpec, MultiplierSpec, RenameSpec, strip_rule_lines,
};
use serde_yaml_ng::Value;
use std::path::PathBuf;

fn temp_config_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "infiltrator-options-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_filter_spec_roundtrip_and_to_rule() {
    let spec = FilterSpec {
        include_keywords: vec!["HK".to_string()],
        exclude_keywords: vec!["剩余流量".to_string()],
        rename_rules: vec![RenameSpec {
            pattern: r"香港-(\d+)".to_string(),
            replacement: "HK-$1".to_string(),
        }],
        exclude_types: vec!["trojan".to_string()],
        deduplication: FilterDedup::AppendIndex,
        ..FilterSpec::default()
    };
    let text = serde_yaml_ng::to_string(&spec).unwrap();
    let parsed: FilterSpec = serde_yaml_ng::from_str(&text).unwrap();
    assert_eq!(parsed, spec);

    let rule = parsed.to_rule().unwrap();
    assert_eq!(rule.include_keywords.len(), 1);
    assert_eq!(rule.rename_rules.len(), 1);
    assert_eq!(rule.deduplication, DeduplicationStrategy::AppendIndex);
    let (names, report) = SubscriptionFilterPipeline::new(rule)
        .filter_proxy_names(&["HK-02".to_string(), "香港-01".to_string()]);
    // Whitelist runs before rename: only the name matching `HK` passes.
    assert_eq!(names, vec!["HK-02"]);
    assert_eq!(report.excluded_by_whitelist, 1);
}

#[test]
fn test_invalid_regex_names_the_pattern() {
    let spec = FilterSpec {
        exclude_keywords: vec!["[".to_string()],
        ..FilterSpec::default()
    };
    let error = spec.to_rule().unwrap_err().to_string();
    assert!(
        error.contains('['),
        "error should name the pattern: {error}"
    );
}

#[test]
fn test_empty_options_return_source_unchanged() {
    let source = "# hand written comment\nmode: rule\n";
    let (out, report) = compose_content(source, &ProfileOptions::default()).unwrap();
    assert_eq!(out, source);
    assert!(report.is_none());
}

#[test]
fn test_compose_runs_filter_before_mixin() {
    let subscription =
        "port: 7890\nproxies:\n  - name: HK-01\n    type: ss\n  - name: US-01\n    type: ss\n";
    let options = ProfileOptions {
        mixin: MixinConfig {
            mode: Some("global".to_string()),
            ..MixinConfig::default()
        },
        filter: Some(FilterSpec {
            exclude_keywords: vec!["US".to_string()],
            ..FilterSpec::default()
        }),
    };
    let (out, report) = compose_content(subscription, &options).unwrap();
    let doc: Value = serde_yaml_ng::from_str(&out).unwrap();
    let proxies = doc.get("proxies").unwrap().as_sequence().unwrap();
    assert_eq!(proxies.len(), 1);
    assert_eq!(proxies[0].get("name").unwrap().as_str().unwrap(), "HK-01");
    assert_eq!(doc.get("mode").unwrap().as_str().unwrap(), "global");
    let report = report.unwrap();
    assert_eq!(report.passed, 1);
    assert_eq!(report.excluded_by_blacklist, 1);
}

#[tokio::test]
async fn test_save_load_roundtrip_and_missing_file_default() {
    let dir = temp_config_dir("roundtrip");
    let options = ProfileOptions {
        mixin: MixinConfig {
            mixed_port: Some(7897),
            ..MixinConfig::default()
        },
        filter: Some(FilterSpec {
            include_keywords: vec!["JP".to_string()],
            ..FilterSpec::default()
        }),
    };
    save_options(&dir, "alpha", &options).await.unwrap();
    let loaded = load_options(&dir, "alpha").await.unwrap();
    assert_eq!(loaded, options);

    let missing = load_options(&dir, "ghost").await.unwrap();
    assert_eq!(missing, ProfileOptions::default());
    let _ = tokio::fs::remove_dir_all(&dir).await;
}

#[tokio::test]
async fn test_saving_empty_options_removes_sidecar() {
    let dir = temp_config_dir("empty-remove");
    let options = ProfileOptions {
        mixin: MixinConfig {
            ipv6: Some(true),
            ..MixinConfig::default()
        },
        filter: None,
    };
    save_options(&dir, "beta", &options).await.unwrap();
    assert!(options_path(&dir, "beta").exists());

    save_options(&dir, "beta", &ProfileOptions::default())
        .await
        .unwrap();
    assert!(!options_path(&dir, "beta").exists());
    let _ = tokio::fs::remove_dir_all(&dir).await;
}

#[tokio::test]
async fn test_malformed_sidecar_is_an_error_not_silently_dropped() {
    let dir = temp_config_dir("malformed");
    let path = options_path(&dir, "gamma");
    tokio::fs::create_dir_all(path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&path, "mixin: [not, a, mapping")
        .await
        .unwrap();
    assert!(load_options(&dir, "gamma").await.is_err());
    let _ = tokio::fs::remove_dir_all(&dir).await;
}

#[tokio::test]
async fn test_apply_saved_options_uses_the_sidecar() {
    let dir = temp_config_dir("apply");
    save_options(
        &dir,
        "delta",
        &ProfileOptions {
            mixin: MixinConfig {
                allow_lan: Some(true),
                ..MixinConfig::default()
            },
            filter: None,
        },
    )
    .await
    .unwrap();
    let (out, report) = apply_saved_options(&dir, "delta", "mode: rule\n")
        .await
        .unwrap();
    assert!(report.is_none());
    let doc: Value = serde_yaml_ng::from_str(&out).unwrap();
    assert!(doc.get("allow-lan").unwrap().as_bool().unwrap());
    let _ = tokio::fs::remove_dir_all(&dir).await;
}

#[test]
fn test_strip_rule_lines_is_exact_match_and_idempotent() {
    let content =
        "rules:\n  - DOMAIN,example.com,PROXY\n  - MATCH,DIRECT\n  - DOMAIN,example.com,PROXY\n";
    let out = strip_rule_lines(content, &["DOMAIN,example.com,PROXY".to_string()]);
    let doc: Value = serde_yaml_ng::from_str(&out).unwrap();
    let rules = doc.get("rules").unwrap().as_sequence().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].as_str().unwrap(), "MATCH,DIRECT");

    // Non-YAML input and empty removals pass through untouched.
    assert_eq!(
        strip_rule_lines("not: [yaml", &["a".to_string()]),
        "not: [yaml"
    );
    assert_eq!(strip_rule_lines(content, &[]), content);
}

#[tokio::test]
async fn test_delete_options_is_best_effort() {
    let dir = temp_config_dir("delete");
    // Missing file: must not panic nor error.
    delete_options(&dir, "ghost").await;
}

#[tokio::test]
async fn test_options_wrapper_follows_configs_dir_redirect() {
    let _guard = mihomo_platform::TEST_LOCK.lock().await;
    let original_home = mihomo_platform::paths::get_home_dir().unwrap();
    let home = temp_config_dir("redirect-home");
    let cloud_dir = home.join("cloud-profiles");
    std::fs::create_dir_all(&cloud_dir).unwrap();
    assert!(mihomo_platform::paths::set_home_dir_override(home.clone()));

    let env_key = mihomo_config::manager::paths::CONFIGS_DIR_ENV;
    let prev_env = std::env::var(env_key).ok();
    unsafe { std::env::remove_var(env_key) };

    let settings_file = crate::settings::settings_path(&home).unwrap();
    let settings = crate::settings::AppSettings {
        configs_dir: Some(cloud_dir.to_string_lossy().to_string()),
        ..crate::settings::AppSettings::default()
    };
    crate::settings::save_settings(&settings_file, &settings)
        .await
        .unwrap();

    let subscription =
        "port: 7890\nproxies:\n  - name: HK-01\n    type: ss\n  - name: US-01\n    type: ss\n";
    let options = ProfileOptions {
        filter: Some(FilterSpec {
            exclude_keywords: vec!["US".to_string()],
            ..FilterSpec::default()
        }),
        ..ProfileOptions::default()
    };
    save_options(&cloud_dir, "redirected", &options)
        .await
        .unwrap();

    // The sidecar lives in the redirected directory; resolving it through the
    // wrapper must follow settings.configs_dir, not `<home>/configs`.
    let (out, report) = apply_saved_options_for("redirected", subscription)
        .await
        .unwrap();
    let doc: Value = serde_yaml_ng::from_str(&out).unwrap();
    let proxies = doc.get("proxies").unwrap().as_sequence().unwrap();
    assert_eq!(proxies.len(), 1);
    assert_eq!(proxies[0].get("name").unwrap().as_str().unwrap(), "HK-01");
    assert_eq!(report.unwrap().excluded_by_blacklist, 1);

    match prev_env {
        Some(value) => unsafe { std::env::set_var(env_key, value) },
        None => unsafe { std::env::remove_var(env_key) },
    }
    mihomo_platform::paths::set_home_dir_override(original_home);
    let _ = tokio::fs::remove_dir_all(&home).await;
}

#[test]
fn test_advanced_filter_spec_compilation_and_execution() {
    let spec = FilterSpec {
        normalize_country_code: true,
        remove_emojis: true,
        allowed_ports: Some(vec![443, 8443]),
        drop_private_ip: true,
        multiplier_rules: vec![MultiplierSpec {
            pattern: r"VIP".to_string(),
            multiplier: 2.0,
        }],
        node_mutator: Some(NodeMutatorConfig {
            force_tls: Some(true),
            force_udp: Some(true),
            client_fingerprint: Some("chrome".to_string()),
            ..Default::default()
        }),
        sort_by: NodeSortOrder::CountryCode,
        content_dedup: ContentDedupStrategy::KeepLowerMultiplier,
        ..Default::default()
    };

    let text = serde_yaml_ng::to_string(&spec).unwrap();
    let parsed: FilterSpec = serde_yaml_ng::from_str(&text).unwrap();
    assert!(parsed.normalize_country_code);
    assert!(parsed.remove_emojis);
    assert_eq!(parsed.allowed_ports, Some(vec![443, 8443]));
    assert!(parsed.drop_private_ip);

    let rule = parsed.to_rule().unwrap();
    assert!(rule.normalize_country_code);
    assert!(rule.remove_emojis);
    assert_eq!(rule.multiplier_rules.len(), 1);

    let yaml = r#"
proxies:
  - name: "🇭🇰 香港 VIP 01"
    type: ss
    server: 1.1.1.1
    port: 443
  - name: "US Normal"
    type: ss
    server: 192.168.1.1
    port: 443
  - name: "🇯🇵 Tokyo Fast"
    type: ss
    server: 2.2.2.2
    port: 80
"#;

    let pipeline = SubscriptionFilterPipeline::new(rule);
    let (out, rep) = pipeline.apply_to_yaml(yaml).unwrap();
    let doc: Value = serde_yaml_ng::from_str(&out).unwrap();
    let proxies = doc.get("proxies").unwrap().as_sequence().unwrap();
    // 192.168.1.1 dropped by private IP, port 80 dropped by port filter
    assert_eq!(proxies.len(), 1);
    assert_eq!(rep.passed, 1);
    assert_eq!(rep.excluded_by_server, 1);
    assert_eq!(rep.excluded_by_port, 1);
    assert_eq!(rep.mutated, 1);
    let first = proxies[0].as_mapping().unwrap();
    assert_eq!(first.get("name").unwrap().as_str().unwrap(), "[HK] 香港 VIP 01 [2x]");
    assert!(first.get("tls").unwrap().as_bool().unwrap());
    assert!(first.get("udp").unwrap().as_bool().unwrap());
    assert_eq!(first.get("client-fingerprint").unwrap().as_str().unwrap(), "chrome");
}
