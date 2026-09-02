use super::*;
use std::time::Duration;

#[test]
fn test_hook_stage_properties() {
    assert_eq!(HookStage::PreDownload.as_str(), "pre_download");
    assert_eq!(HookStage::PostDownload.as_str(), "post_download");
    assert_eq!(HookStage::PreMerge.as_str(), "pre_merge");
    assert_eq!(HookStage::PostMerge.as_str(), "post_merge");

    assert!(
        HookStage::PreDownload
            .display_name()
            .contains("Pre-Download")
    );
    assert!(
        HookStage::PostDownload
            .display_name()
            .contains("Post-Download")
    );
    assert!(HookStage::PreMerge.display_name().contains("Pre-Merge"));
    assert!(HookStage::PostMerge.display_name().contains("Post-Merge"));

    let json = serde_json::to_string(&HookStage::PostMerge).unwrap();
    assert_eq!(json, "\"post_merge\"");
    let parsed: HookStage = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, HookStage::PostMerge);
}

#[test]
fn test_builtin_presets_coverage() {
    let presets = ScriptEngine::builtin_presets();
    assert_eq!(presets.len(), 4);

    let ids: Vec<&str> = presets.iter().map(|p| p.id).collect();
    assert!(ids.contains(&"remove-ads"));
    assert!(ids.contains(&"auto-country-groups"));
    assert!(ids.contains(&"streaming-groups"));
    assert!(ids.contains(&"direct-china"));

    for preset in &presets {
        assert!(!preset.name.is_empty());
        assert!(!preset.description.is_empty());
        assert!(preset.script_code.contains("function main"));
    }

    let found = ScriptEngine::find_preset("remove-ads");
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, "remove-ads");

    let not_found = ScriptEngine::find_preset("non-existent-preset");
    assert!(not_found.is_none());
}

#[test]
fn test_extension_package_export_import_roundtrip() {
    let ext = ExtensionPackage {
        name: "Auto Grouping".to_string(),
        version: "1.2.0".to_string(),
        author: "Infiltrator Team".to_string(),
        description: "Auto groups proxy nodes by country".to_string(),
        stage: HookStage::PreMerge,
        script_code: "function main(config, profile) { return config; }".to_string(),
        mixin_yaml: Some("rules:\n  - DOMAIN,google.com,PROXY".to_string()),
        tags: vec!["country".to_string(), "grouping".to_string()],
    };

    let json = ScriptEngine::export_extension_package(&ext).unwrap();
    let imported = ScriptEngine::import_extension_package(&json).unwrap();
    assert_eq!(ext, imported);

    // Test alias methods
    let json2 = ScriptEngine::export_extension(&ext).unwrap();
    let imported2 = ScriptEngine::import_extension(&json2).unwrap();
    assert_eq!(ext, imported2);

    // Test JSON missing stage defaults to PreMerge
    let legacy_json = r#"{
        "name": "Legacy",
        "version": "1.0.0",
        "author": "Tester",
        "description": "Legacy package",
        "script_code": "function main(config) { return config; }",
        "mixin_yaml": null
    }"#;
    let imported_legacy: ExtensionPackage =
        ScriptEngine::import_extension_package(legacy_json).unwrap();
    assert_eq!(imported_legacy.stage, HookStage::PreMerge);
    assert!(imported_legacy.tags.is_empty());
}

#[test]
fn test_timeout_enforcement_and_loop_protection() {
    let engine = ScriptEngine::new().with_timeout(Duration::from_millis(100));

    let loops = [
        "function main() { while (true) {} }",
        "function main() { while(true) {} }",
        "function main() { for (;;) {} }",
        "function main() { for(;;) {} }",
        "function main() { while (1) {} }",
        "function main() { while(1) {} }",
    ];

    for script in loops {
        let err = engine
            .execute_transform(script, "port: 7890\nmode: rule")
            .unwrap_err();
        assert!(matches!(err, ScriptError::Timeout(_)));
    }
}

#[test]
fn test_memory_guard_protection() {
    let engine = ScriptEngine::new().with_max_memory(1024); // 1 KB limit
    let huge_yaml = format!("port: 7890\ncomment: '{}'", "A".repeat(2048));
    let script = "function main(c) { return c; }";

    let err = engine.execute_transform(script, &huge_yaml).unwrap_err();
    assert!(matches!(err, ScriptError::MemoryExceeded(_)));
}

#[test]
fn test_syntax_validation() {
    let engine = ScriptEngine::new();

    let invalid_script = "let a = 42;";
    let err = engine
        .execute_transform(invalid_script, "port: 7890")
        .unwrap_err();
    assert!(matches!(err, ScriptError::Syntax(_)));

    let malformed_regex_script = r#"function main(config) {
        filter_nodes_by_regex(config, "[invalid(regex", true);
        return config;
    }"#;
    let err2 = engine
        .execute_transform(malformed_regex_script, "port: 7890")
        .unwrap_err();
    assert!(matches!(err2, ScriptError::Syntax(_)));
}

#[test]
fn test_add_proxy_group_helper() {
    let mut ast: Value = serde_yaml_ng::from_str("port: 7890\nmode: rule").unwrap();

    // Add new select group
    let proxies = vec!["Node A".to_string(), "Node B".to_string()];
    add_proxy_group(&mut ast, "Proxy Group 1", "select", &proxies, None, None).unwrap();

    // Add new url-test group with URL and interval
    let auto_proxies = vec!["Node A".to_string()];
    add_proxy_group(
        &mut ast,
        "Auto Test",
        "url-test",
        &auto_proxies,
        Some("http://www.gstatic.com/generate_204"),
        Some(300),
    )
    .unwrap();

    // Update existing group
    let updated_proxies = vec!["Node A".to_string(), "Node C".to_string()];
    add_proxy_group(
        &mut ast,
        "Proxy Group 1",
        "fallback",
        &updated_proxies,
        None,
        None,
    )
    .unwrap();

    let pg = ast["proxy-groups"].as_sequence().unwrap();
    assert_eq!(pg.len(), 2);
    assert_eq!(pg[0]["name"].as_str().unwrap(), "Proxy Group 1");
    assert_eq!(pg[0]["type"].as_str().unwrap(), "fallback");
    assert_eq!(pg[0]["proxies"].as_sequence().unwrap().len(), 2);

    assert_eq!(pg[1]["name"].as_str().unwrap(), "Auto Test");
    assert_eq!(pg[1]["type"].as_str().unwrap(), "url-test");
    assert_eq!(
        pg[1]["url"].as_str().unwrap(),
        "http://www.gstatic.com/generate_204"
    );
    assert_eq!(pg[1]["interval"].as_u64().unwrap(), 300);
}

#[test]
fn test_remove_rules_helper() {
    let yaml_str = r#"
rules:
  - DOMAIN-SUFFIX,google.com,PROXY
  - DOMAIN-SUFFIX,doubleclick.net,REJECT
  - DOMAIN-KEYWORD,adservice,REJECT
  - MATCH,DIRECT
"#;
    let mut ast: Value = serde_yaml_ng::from_str(yaml_str).unwrap();

    let removed = remove_rules(&mut ast, "REJECT|adservice").unwrap();
    assert_eq!(removed, 2);

    let rules = ast["rules"].as_sequence().unwrap();
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].as_str().unwrap(), "DOMAIN-SUFFIX,google.com,PROXY");
    assert_eq!(rules[1].as_str().unwrap(), "MATCH,DIRECT");
}

#[test]
fn test_filter_nodes_by_regex_helper() {
    let yaml_str = r#"
proxies:
  - name: "🇭🇰 香港 01"
    type: ss
  - name: "官网 01 - 剩余流量 100G"
    type: ss
  - name: "🇯🇵 日本 01"
    type: ss
  - name: "过期通知 - 请重置"
    type: ss
proxy-groups:
  - name: "PROXIES"
    type: select
    proxies:
      - "🇭🇰 香港 01"
      - "官网 01 - 剩余流量 100G"
      - "🇯🇵 日本 01"
      - "过期通知 - 请重置"
"#;
    let mut ast: Value = serde_yaml_ng::from_str(yaml_str).unwrap();

    // Invert = true: Remove nodes matching spam keywords
    let removed = filter_nodes_by_regex(&mut ast, "官网|剩余|过期|重置", true).unwrap();
    assert_eq!(removed, 2);

    let proxies = ast["proxies"].as_sequence().unwrap();
    assert_eq!(proxies.len(), 2);
    assert_eq!(proxies[0]["name"].as_str().unwrap(), "🇭🇰 香港 01");
    assert_eq!(proxies[1]["name"].as_str().unwrap(), "🇯🇵 日本 01");

    // Verify proxy-groups cleaned up removed proxy references
    let pg_proxies = ast["proxy-groups"][0]["proxies"].as_sequence().unwrap();
    assert_eq!(pg_proxies.len(), 2);
    assert_eq!(pg_proxies[0].as_str().unwrap(), "🇭🇰 香港 01");
    assert_eq!(pg_proxies[1].as_str().unwrap(), "🇯🇵 日本 01");
}

#[test]
fn test_set_dns_mode_helper() {
    let mut ast: Value = serde_yaml_ng::from_str("port: 7890").unwrap();

    set_dns_mode(&mut ast, "fake-ip", true).unwrap();

    assert!(ast["dns"]["enable"].as_bool().unwrap());
    assert_eq!(ast["dns"]["enhanced-mode"].as_str().unwrap(), "fake-ip");

    set_dns_mode(&mut ast, "redir-host", false).unwrap();
    assert!(!ast["dns"]["enable"].as_bool().unwrap());
    assert_eq!(ast["dns"]["enhanced-mode"].as_str().unwrap(), "redir-host");
}

#[test]
fn test_generate_country_proxy_groups_dynamic() {
    let yaml_str = r#"
proxies:
  - name: "🇭🇰 香港 IEPL 01"
    type: ss
  - name: "🇭🇰 香港 BGP 02"
    type: ss
  - name: "🇯🇵 日本 Tokyo 01"
    type: ss
  - name: "🇺🇸 美国 LA 01"
    type: ss
  - name: "🇸🇬 新加坡 SG 01"
    type: ss
  - name: "🇹🇼 台湾 TW 01"
    type: ss
  - name: "🇰🇷 韩国 Seoul 01"
    type: ss
  - name: "🇬🇧 英国 London 01"
    type: ss
  - name: "🇩🇪 德国 Frankfurt 01"
    type: ss
proxy-groups:
  - name: "PROXIES"
    type: select
    proxies:
      - "DIRECT"
"#;
    let mut ast: Value = serde_yaml_ng::from_str(yaml_str).unwrap();

    let groups = generate_country_proxy_groups(&mut ast, true).unwrap();
    assert_eq!(groups.len(), 8);
    assert!(groups.contains(&"🇭🇰 香港".to_string()));
    assert!(groups.contains(&"🇯🇵 日本".to_string()));
    assert!(groups.contains(&"🇺🇸 美国".to_string()));
    assert!(groups.contains(&"🇸🇬 新加坡".to_string()));

    let pg = ast["proxy-groups"].as_sequence().unwrap();
    // 1 original PROXIES + 1 HK auto test + 8 country select groups = 10
    assert_eq!(pg.len(), 10);

    // Verify HK has auto-select group since it has 2 nodes
    let hk_auto = pg
        .iter()
        .find(|g| g["name"].as_str() == Some("🇭🇰 自动选择"));
    assert!(hk_auto.is_some());
    assert_eq!(hk_auto.unwrap()["type"].as_str().unwrap(), "url-test");

    // Verify PROXIES group includes country groups at the front
    let main_pg = pg
        .iter()
        .find(|g| g["name"].as_str() == Some("PROXIES"))
        .unwrap();
    let main_proxies: Vec<&str> = main_pg["proxies"]
        .as_sequence()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(main_proxies.contains(&"🇭🇰 香港"));
    assert!(main_proxies.contains(&"🇯🇵 日本"));
    assert!(main_proxies.contains(&"🇺🇸 美国"));
    assert!(main_proxies.contains(&"🇸🇬 新加坡"));
}

#[test]
fn test_generate_auto_latency_group() {
    let yaml_str = r#"
proxies:
  - name: "🇭🇰 香港 01"
    type: ss
  - name: "🇯🇵 日本 01"
    type: ss
"#;
    let mut ast: Value = serde_yaml_ng::from_str(yaml_str).unwrap();

    let res = generate_auto_latency_group(&mut ast, "⚡ 自动选择", None, None).unwrap();
    assert_eq!(res, Some("⚡ 自动选择".to_string()));

    let pg = ast["proxy-groups"].as_sequence().unwrap();
    assert_eq!(pg[0]["name"].as_str().unwrap(), "⚡ 自动选择");
    assert_eq!(pg[0]["type"].as_str().unwrap(), "url-test");
    assert_eq!(pg[0]["proxies"].as_sequence().unwrap().len(), 2);
}

#[test]
fn test_generate_streaming_proxy_groups() {
    let yaml_str = r#"
proxies:
  - name: "🇭🇰 香港 01"
    type: ss
  - name: "🇺🇸 美国 01"
    type: ss
rules:
  - MATCH,DIRECT
"#;
    let mut ast: Value = serde_yaml_ng::from_str(yaml_str).unwrap();

    let groups = generate_streaming_proxy_groups(&mut ast).unwrap();
    assert_eq!(groups.len(), 4);
    assert!(groups.contains(&"🎬 Netflix".to_string()));
    assert!(groups.contains(&"🐭 Disney+".to_string()));
    assert!(groups.contains(&"📹 YouTube".to_string()));
    assert!(groups.contains(&"🤖 OpenAI".to_string()));

    let rules = ast["rules"].as_sequence().unwrap();
    let rules_strs: Vec<&str> = rules.iter().filter_map(|r| r.as_str()).collect();
    assert!(rules_strs.iter().any(|r| r.contains("netflix.com")));
    assert!(rules_strs.iter().any(|r| r.contains("disneyplus.com")));
    assert!(rules_strs.iter().any(|r| r.contains("youtube.com")));
    assert!(rules_strs.iter().any(|r| r.contains("openai.com")));
}

#[test]
fn test_generate_china_direct_rules() {
    let yaml_str = r#"
rules:
  - MATCH,PROXY
"#;
    let mut ast: Value = serde_yaml_ng::from_str(yaml_str).unwrap();

    generate_china_direct_rules(&mut ast).unwrap();

    let rules = ast["rules"].as_sequence().unwrap();
    let rules_strs: Vec<&str> = rules.iter().filter_map(|r| r.as_str()).collect();
    assert!(rules_strs.contains(&"GEOIP,CN,DIRECT"));
    assert!(rules_strs.contains(&"DOMAIN-SUFFIX,cn,DIRECT"));
    assert!(rules_strs.contains(&"IP-CIDR,192.168.0.0/16,DIRECT,no-resolve"));
    assert_eq!(*rules_strs.last().unwrap(), "MATCH,PROXY");
}

#[test]
fn test_full_preset_execution_remove_ads() {
    let engine = ScriptEngine::new();
    let preset = ScriptEngine::find_preset("remove-ads").unwrap();

    let yaml_input = r#"
proxies:
  - name: "🇭🇰 香港 01"
    type: ss
  - name: "官网通知 - 剩余流量不足"
    type: ss
rules:
  - DOMAIN-SUFFIX,google.com,PROXY
  - DOMAIN-SUFFIX,adservice.google.com,REJECT
"#;

    let output = engine
        .execute_transform(preset.script_code, yaml_input)
        .unwrap();
    assert!(output.contains("🇭🇰 香港 01"));
    assert!(!output.contains("官网通知"));
    assert!(output.contains("google.com,PROXY"));
    assert!(!output.contains("adservice.google.com"));
}

#[test]
fn test_full_preset_execution_auto_country_groups() {
    let engine = ScriptEngine::new();
    let preset = ScriptEngine::find_preset("auto-country-groups").unwrap();

    let yaml_input = r#"
proxies:
  - name: "🇭🇰 香港 01"
    type: ss
  - name: "🇭🇰 香港 02"
    type: ss
  - name: "🇯🇵 日本 01"
    type: ss
  - name: "🇺🇸 美国 01"
    type: ss
  - name: "🇸🇬 新加坡 01"
    type: ss
"#;

    let output = engine
        .execute_transform(preset.script_code, yaml_input)
        .unwrap();
    assert!(output.contains("🇭🇰 香港"));
    assert!(output.contains("🇯🇵 日本"));
    assert!(output.contains("🇺🇸 美国"));
    assert!(output.contains("🇸🇬 新加坡"));
    assert!(output.contains("🇭🇰 自动选择"));
}

#[test]
fn test_full_preset_execution_streaming_groups() {
    let engine = ScriptEngine::new();
    let preset = ScriptEngine::find_preset("streaming-groups").unwrap();

    let yaml_input = r#"
proxies:
  - name: "🇭🇰 香港 01"
    type: ss
rules:
  - MATCH,DIRECT
"#;

    let output = engine
        .execute_transform(preset.script_code, yaml_input)
        .unwrap();
    assert!(output.contains("🎬 Netflix"));
    assert!(output.contains("🐭 Disney+"));
    assert!(output.contains("📹 YouTube"));
    assert!(output.contains("🤖 OpenAI"));
    assert!(output.contains("DOMAIN-SUFFIX,netflix.com"));
}

#[test]
fn test_full_preset_execution_direct_china() {
    let engine = ScriptEngine::new();
    let preset = ScriptEngine::find_preset("direct-china").unwrap();

    let yaml_input = r#"
rules:
  - MATCH,PROXY
"#;

    let output = engine
        .execute_transform(preset.script_code, yaml_input)
        .unwrap();
    assert!(output.contains("GEOIP,CN,DIRECT"));
    assert!(output.contains("DOMAIN-SUFFIX,cn,DIRECT"));
    assert!(output.contains("IP-CIDR,192.168.0.0/16,DIRECT,no-resolve"));
}

#[test]
fn test_custom_script_execution_with_console_logs_and_stages() {
    let engine = ScriptEngine::new();
    let script = r#"function main(config, profile) {
    console.log("Hook started");
    set_dns_mode(config, "fake-ip", true);
    add_proxy_group(config, "Custom Group", "select", ["Node 1", "Node 2"]);
    console.log("Hook finished successfully");
    return config;
}"#;

    let yaml_input = "port: 7890\nmode: rule";
    let res = engine
        .execute_transform_detailed(script, yaml_input, HookStage::PostDownload)
        .unwrap();

    assert!(res.success);
    assert_eq!(res.stage, HookStage::PostDownload);
    assert_eq!(
        res.console_logs,
        vec![
            "Hook started".to_string(),
            "Hook finished successfully".to_string()
        ]
    );
    assert!(res.transformed_yaml.contains("enhanced-mode: fake-ip"));
    assert!(res.transformed_yaml.contains("Custom Group"));
}

#[test]
fn test_execute_transform_static() {
    let script = r#"function main(config, profile) {
    console.log("Static helper invoked");
    return config;
}"#;
    let yaml_input = "port: 7890";
    let res =
        ScriptEngine::execute_transform_static(script, yaml_input, Duration::from_millis(500))
            .unwrap();
    assert!(res.success);
    assert_eq!(res.console_logs, vec!["Static helper invoked".to_string()]);
}

#[test]
fn test_error_handling_malformed_yaml() {
    let engine = ScriptEngine::new();
    let script = "function main(config) { return config; }";
    let bad_yaml = ": [invalid yaml ::::";

    let err = engine.execute_transform(script, bad_yaml).unwrap_err();
    assert!(matches!(err, ScriptError::Runtime(_)));
}

#[test]
fn test_base64_shim() {
    let data = b"Infiltrator Sandboxed Scripting";
    let encoded = Base64Shim::encode(data);
    let decoded = Base64Shim::decode(&encoded).unwrap();
    assert_eq!(decoded, data);

    let url_encoded = Base64Shim::encode_url_safe(data);
    let url_decoded = Base64Shim::decode_url_safe(&url_encoded).unwrap();
    assert_eq!(url_decoded, data);
}

#[test]
fn test_fetch_permission_shim() {
    use std::collections::HashSet;
    let mut perms = HashSet::new();

    // Without NetworkAccess permission
    let err = FetchPermissionShim::check_permission(&perms, "https://api.example.com/data", None)
        .unwrap_err();
    assert!(matches!(err, ScriptError::Runtime(_)));

    // With NetworkAccess permission
    perms.insert(PluginPermission::NetworkAccess);
    assert!(
        FetchPermissionShim::check_permission(&perms, "https://api.example.com/data", None).is_ok()
    );

    // With domain allowlist matching
    let allowlist = ["example.com", "github.com"];
    assert!(
        FetchPermissionShim::check_permission(
            &perms,
            "https://api.example.com/data",
            Some(&allowlist)
        )
        .is_ok()
    );
    assert!(
        FetchPermissionShim::check_permission(&perms, "https://github.com/repo", Some(&allowlist))
            .is_ok()
    );

    // With domain allowlist blocking unknown domain
    let err2 = FetchPermissionShim::check_permission(
        &perms,
        "https://malicious.net/evil",
        Some(&allowlist),
    )
    .unwrap_err();
    assert!(matches!(err2, ScriptError::Runtime(_)));
}

#[test]
fn test_script_circuit_breaker() {
    let mut breaker = ScriptCircuitBreaker::new(3, Duration::from_millis(50));
    assert!(!breaker.is_tripped());
    assert_eq!(breaker.consecutive_failures(), 0);

    breaker.record_failure();
    assert_eq!(breaker.consecutive_failures(), 1);
    assert!(!breaker.is_tripped());

    breaker.record_failure();
    assert_eq!(breaker.consecutive_failures(), 2);
    assert!(!breaker.is_tripped());

    breaker.record_failure();
    assert_eq!(breaker.consecutive_failures(), 3);
    assert!(breaker.is_tripped());

    // While tripped within cooldown
    assert!(breaker.is_tripped());

    // After cooldown elapsed
    std::thread::sleep(Duration::from_millis(60));
    assert!(!breaker.is_tripped());

    // Success resets count
    breaker.record_success();
    assert_eq!(breaker.consecutive_failures(), 0);
    assert!(!breaker.is_tripped());
}

#[test]
fn test_script_context_builder() {
    let ctx = ScriptContext::new(HookStage::PreMerge)
        .with_profile("Subscription Profile")
        .with_permission(PluginPermission::NetworkAccess)
        .with_permission(PluginPermission::ModifyRules)
        .with_env("ENVIRONMENT", "production");

    assert_eq!(ctx.stage, HookStage::PreMerge);
    assert_eq!(ctx.profile_name.as_deref(), Some("Subscription Profile"));
    assert!(ctx.has_permission(PluginPermission::NetworkAccess));
    assert!(ctx.has_permission(PluginPermission::ModifyRules));
    assert!(!ctx.has_permission(PluginPermission::FileSystemRead));
    assert_eq!(
        ctx.environment.get("ENVIRONMENT").map(String::as_str),
        Some("production")
    );
}

#[test]
fn test_extension_package_checksum_verification() {
    let pkg = ExtensionPackage {
        name: "Test Package".to_string(),
        version: "1.0.0".to_string(),
        author: "Infiltrator".to_string(),
        description: "Test description".to_string(),
        stage: HookStage::PreMerge,
        script_code: "function main(config) { return config; }".to_string(),
        mixin_yaml: Some("dns:\n  enable: true".to_string()),
        tags: vec!["test".to_string()],
    };

    let checksum = pkg.calculate_checksum();
    assert!(!checksum.is_empty());
    assert!(pkg.verify_checksum(&checksum));
    assert!(!pkg.verify_checksum("invalid_checksum"));
}

#[test]
fn test_rename_nodes_by_regex() {
    let yaml_str = r#"
proxies:
  - name: "[VIP] 🇭🇰 HK 01"
    type: ss
  - name: "[VIP] 🇯🇵 JP 02"
    type: ss
proxy-groups:
  - name: "PROXIES"
    type: select
    proxies:
      - "[VIP] 🇭🇰 HK 01"
      - "[VIP] 🇯🇵 JP 02"
"#;
    let mut ast: Value = serde_yaml_ng::from_str(yaml_str).unwrap();
    let count = rename_nodes_by_regex(&mut ast, r"\[VIP\]\s*", "").unwrap();
    assert_eq!(count, 2);

    let proxies = ast["proxies"].as_sequence().unwrap();
    assert_eq!(proxies[0]["name"].as_str().unwrap(), "🇭🇰 HK 01");
    assert_eq!(proxies[1]["name"].as_str().unwrap(), "🇯🇵 JP 02");

    let pg_proxies = ast["proxy-groups"][0]["proxies"].as_sequence().unwrap();
    assert_eq!(pg_proxies[0].as_str().unwrap(), "🇭🇰 HK 01");
    assert_eq!(pg_proxies[1].as_str().unwrap(), "🇯🇵 JP 02");
}

#[test]
fn test_remove_proxy_group_and_rules_prepend_append() {
    let yaml_str = r#"
proxy-groups:
  - name: "Group 1"
    type: select
    proxies: ["DIRECT"]
  - name: "Group 2"
    type: select
    proxies: ["DIRECT"]
rules:
  - MATCH,DIRECT
"#;
    let mut ast: Value = serde_yaml_ng::from_str(yaml_str).unwrap();

    let removed = remove_proxy_group(&mut ast, "Group 1").unwrap();
    assert!(removed);
    assert_eq!(ast["proxy-groups"].as_sequence().unwrap().len(), 1);

    prepend_rule(&mut ast, "DOMAIN-SUFFIX,google.com,PROXY").unwrap();
    append_rule(&mut ast, "FINAL,DIRECT").unwrap();

    let rules = ast["rules"].as_sequence().unwrap();
    assert_eq!(rules.len(), 3);
    assert_eq!(rules[0].as_str().unwrap(), "DOMAIN-SUFFIX,google.com,PROXY");
    assert_eq!(rules[1].as_str().unwrap(), "MATCH,DIRECT");
    assert_eq!(rules[2].as_str().unwrap(), "FINAL,DIRECT");
}

#[test]
fn test_script_validation() {
    let valid_script = r#"function main(config, profile) {
        rename_nodes_by_regex(config, "test", "demo");
        filter_nodes_by_regex(config, "ad", true);
        return config;
    }"#;
    let res = ScriptEngine::validate_script(valid_script);
    assert!(res.valid);
    assert!(res.entry_point_found);
    assert_eq!(res.directives_count, 2);

    let loop_script = "function main(config) { while(true) {} }";
    let res2 = ScriptEngine::validate_script(loop_script);
    assert!(!res2.valid);
    assert!(res2.error.unwrap().contains("Infinite loop"));

    let no_entry = "let a = 123;";
    let res3 = ScriptEngine::validate_script(no_entry);
    assert!(!res3.valid);
    assert!(res3.error.unwrap().contains("Missing entry point"));
}
