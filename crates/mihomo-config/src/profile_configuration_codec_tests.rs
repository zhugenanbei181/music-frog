//! Behavior checks for the engine/format/schema-scoped configuration codec.
//! test-intent: behavior

use super::{apply, read, read_rule_workspace};
use infiltrator_contract::dns::DnsSettingsPatch;
use infiltrator_contract::dns_hosts::DnsHostEntry;
use infiltrator_contract::provider_cache::{KernelEtagSupportSnapshot, KernelEtagSupportState};
use infiltrator_contract::rule_location::RuleLocation;
use infiltrator_domain::{dns, rules, sniffer, tun};
use infiltrator_ports::profile_document_codec::{
    ProfileConfigurationQuery, ProfileConfigurationReadout, ProfileConfigurationUpdate,
    ProfileRuleWorkspaceUpdate,
};
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileDocumentRevision, ProfileDocumentSourceIdentity,
    ProfileDocumentSourceKind, ProfileId, ProfileSidecarRevision, ProfileSourceId,
    ProfileWorkspaceIdentity,
};
use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};
use std::collections::BTreeMap;

fn identity(engine: &str, format: &str, schema: u32) -> ProfileDocumentIdentity {
    ProfileDocumentIdentity::new(
        ProfileId::new("main").expect("profile ID"),
        EngineId::new(engine).expect("engine ID"),
        ProfileDocumentSourceIdentity::new(
            ProfileDocumentSourceKind::Local,
            ProfileSourceId::new("local-source").expect("source ID"),
        ),
        ProfileDocumentRevision::new("sha256:source").expect("document revision"),
        ProfileFormatId::new(format).expect("format ID"),
        ProfileSchemaVersion::new(schema).expect("schema version"),
    )
}

fn mihomo() -> ProfileDocumentIdentity {
    identity("mihomo", "mihomo-yaml", 1)
}

#[test]
fn typed_read_distinguishes_absent_declaration_and_reads_yaml_owned_by_adapter() {
    let identity = mihomo();
    let etag = read(
        &identity,
        b"mode: rule\n",
        ProfileConfigurationQuery::EtagSupport,
    )
    .expect("read absent ETag declaration");
    assert!(matches!(
        etag,
        ProfileConfigurationReadout::EtagSupport(KernelEtagSupportSnapshot {
            state: KernelEtagSupportState::NotDeclared,
            declared: None,
        })
    ));

    let tun = read(
        &identity,
        b"tun:\n  enable: true\n  mtu: 9000\n",
        ProfileConfigurationQuery::Tun,
    )
    .expect("read TUN config");
    assert!(matches!(
        tun,
        ProfileConfigurationReadout::Tun(tun::TunConfig {
            enable: Some(true),
            mtu: Some(9000),
            ..
        })
    ));

    let endpoint = read(
        &identity,
        b"mixed-port: 7890\nsocks-port: 7891\nport: 7892\n",
        ProfileConfigurationQuery::ProxyEndpoint,
    )
    .expect("read proxy endpoint");
    assert!(matches!(
        endpoint,
        ProfileConfigurationReadout::ProxyEndpoint(Some(endpoint))
            if endpoint == "socks5://127.0.0.1:7890"
    ));
    let absent = read(
        &identity,
        b"mode: rule\n",
        ProfileConfigurationQuery::ProxyEndpoint,
    )
    .expect("read absent proxy endpoint");
    assert!(matches!(
        absent,
        ProfileConfigurationReadout::ProxyEndpoint(None)
    ));
}

#[test]
fn typed_dns_settings_update_preserves_other_yaml_and_returns_exact_decoded_readback() {
    let identity = mihomo();
    let result = apply(
        &identity,
        b"mixed-port: 7890\ndns:\n  future-setting: preserve-me\n",
        ProfileConfigurationUpdate::DnsSettings(DnsSettingsPatch {
            nameserver: Some(vec!["https://dns.example/query".into()]),
            hosts: Some(vec![DnsHostEntry {
                domain: "local.example".into(),
                address: "192.0.2.2".into(),
            }]),
            ..DnsSettingsPatch::default()
        }),
    )
    .expect("typed update");
    let document = String::from_utf8(result.document_bytes.clone()).expect("UTF-8 YAML");
    assert!(document.contains("mixed-port: 7890"));
    assert!(document.contains("future-setting: preserve-me"));
    assert!(document.contains("local.example: 192.0.2.2"));
    match result.readback {
        ProfileConfigurationReadout::Dns(config) => {
            assert_eq!(
                config.nameserver,
                Some(vec!["https://dns.example/query".into()])
            );
        }
        other => panic!("unexpected readback: {other:?}"),
    }
    let hosts = read(
        &identity,
        &result.document_bytes,
        ProfileConfigurationQuery::Hosts,
    )
    .expect("read exact committed bytes");
    assert!(matches!(
        hosts,
        ProfileConfigurationReadout::Hosts { entries, legacy_entries }
            if entries == vec![DnsHostEntry {
                domain: "local.example".into(),
                address: "192.0.2.2".into(),
            }] && legacy_entries.is_empty()
    ));
}

#[test]
fn typed_rules_sections_round_trip_through_the_mihomo_codec_without_persistence() {
    let identity = mihomo();
    let mut rule_providers = BTreeMap::new();
    rule_providers.insert(
        "ads".to_owned(),
        serde_json::json!({"type": "inline", "behavior": "domain"}),
    );
    let rules_update = apply(
        &identity,
        b"mode: rule\ncustom-root: preserve-me\nrule-providers: {}\n",
        ProfileConfigurationUpdate::RuleProviders(rule_providers.clone()),
    )
    .expect("typed rule-provider update");
    assert!(String::from_utf8_lossy(&rules_update.document_bytes).contains("preserve-me"));
    assert!(matches!(
        rules_update.readback,
        ProfileConfigurationReadout::RuleProviders(value) if value == rule_providers
    ));

    let mut proxy_providers = BTreeMap::new();
    proxy_providers.insert(
        "edge".to_owned(),
        serde_json::json!({"type": "http", "url": "https://example.test/proxies.yaml"}),
    );
    let proxy_update = apply(
        &identity,
        &rules_update.document_bytes,
        ProfileConfigurationUpdate::ProxyProviders(proxy_providers.clone()),
    )
    .expect("typed proxy-provider update");
    assert!(matches!(
        proxy_update.readback,
        ProfileConfigurationReadout::ProxyProviders(value) if value == proxy_providers
    ));

    let sniffer_update = apply(
        &identity,
        &proxy_update.document_bytes,
        ProfileConfigurationUpdate::Sniffer(sniffer::SnifferConfig {
            enable: Some(true),
            ..Default::default()
        }),
    )
    .expect("typed sniffer update");
    assert!(matches!(
        sniffer_update.readback,
        ProfileConfigurationReadout::Sniffer(config) if config.enable == Some(true)
    ));
    assert!(String::from_utf8_lossy(&sniffer_update.document_bytes).contains("preserve-me"));
}

#[test]
fn unsupported_engine_format_and_schema_fail_before_yaml_decode() {
    for foreign in [
        identity("sing-box", "mihomo-yaml", 1),
        identity("mihomo", "mihomo-json", 1),
        identity("mihomo", "mihomo-yaml", 2),
    ] {
        for query in [
            ProfileConfigurationQuery::Dns,
            ProfileConfigurationQuery::ProxyEndpoint,
        ] {
            let error = read(&foreign, b"not: [valid YAML", query)
                .expect_err("foreign identity must fail before parsing");
            assert_eq!(error.error_code(), ErrorCode::Unsupported);
        }
    }
}

#[test]
fn invalid_document_or_typed_update_has_no_mutation_result() {
    let identity = mihomo();
    let invalid_document = apply(
        &identity,
        b"tun: [\n",
        ProfileConfigurationUpdate::Tun(tun::TunConfigPatch {
            mtu: Some(9000),
            ..Default::default()
        }),
    )
    .expect_err("invalid source rejected");
    assert_eq!(invalid_document.error_code(), ErrorCode::Configuration);

    let invalid_update = apply(
        &identity,
        b"tun:\n  enable: true\n",
        ProfileConfigurationUpdate::Tun(tun::TunConfigPatch {
            mtu: Some(0),
            ..Default::default()
        }),
    )
    .expect_err("invalid typed update rejected");
    assert_eq!(invalid_update.error_code(), ErrorCode::Configuration);
}

#[test]
fn configuration_codec_does_not_erase_unknown_dns_options() {
    let result = apply(
        &mihomo(),
        b"dns:\n  future-setting: preserve-me\n  enable: false\n",
        ProfileConfigurationUpdate::Dns(dns::DnsConfigPatch {
            enable: Some(true),
            ..Default::default()
        }),
    )
    .expect("typed DNS update");
    let output = String::from_utf8(result.document_bytes).expect("UTF-8 YAML");
    assert!(output.contains("future-setting: preserve-me"));
    assert!(output.contains("enable: true"));
}

#[test]
fn rule_workspace_projection_keeps_profile_sidecar_identity_and_engine_owned_tables() {
    let workspace = ProfileWorkspaceIdentity::new(mihomo(), ProfileSidecarRevision::Absent);
    let projected = read_rule_workspace(
        &workspace,
        b"proxies:\n  - name: edge\n    type: ss\nproxy-groups:\n  - name: auto\n    type: select\nrules:\n  - MATCH,auto\nsub-rules:\n  regional:\n    - DOMAIN,example.com,DIRECT\n",
    )
    .expect("typed rule projection");
    assert_eq!(projected.source.profile, "main");
    assert_eq!(projected.rules.len(), 1);
    assert_eq!(projected.targets, vec!["DIRECT", "REJECT", "edge", "auto"]);
    assert_eq!(
        projected.sub_rules["regional"][0].rule,
        "DOMAIN,example.com,DIRECT"
    );
}

#[test]
fn rule_workspace_projection_rejects_a_foreign_schema_before_parsing() {
    let workspace = ProfileWorkspaceIdentity::new(
        identity("mihomo", "mihomo-json", 1),
        ProfileSidecarRevision::Absent,
    );
    let error =
        read_rule_workspace(&workspace, b"not: [valid YAML").expect_err("foreign format rejected");
    assert_eq!(error.error_code(), ErrorCode::Unsupported);
}

#[test]
fn typed_rule_workspace_mutations_return_adapter_readback_for_exact_candidate_bytes() {
    let workspace = ProfileWorkspaceIdentity::new(mihomo(), ProfileSidecarRevision::Absent);
    let source =
        b"rules:\n  - MATCH,DIRECT\nsub-rules:\n  regional:\n    - DOMAIN,example.com,DIRECT\n";
    let replacement_rules = vec![rules::RuleEntry {
        rule: "MATCH,REJECT".to_owned(),
        enabled: true,
    }];
    let root_update = super::apply_rule_workspace(
        &workspace,
        source,
        ProfileRuleWorkspaceUpdate::ReplaceRules(replacement_rules.clone()),
    )
    .expect("typed root rule edit");
    assert_eq!(root_update.readback.rules, replacement_rules);
    assert_eq!(
        root_update.readback.source.profile,
        workspace.document.profile_id.as_str()
    );
    assert!(
        root_update
            .document_bytes
            .windows(12)
            .any(|bytes| bytes == b"MATCH,REJECT")
    );

    let named_update = super::apply_rule_workspace(
        &workspace,
        source,
        ProfileRuleWorkspaceUpdate::ReplaceNamedRule {
            location: RuleLocation::named("regional".to_owned(), 0),
            expected_rule: "DOMAIN,example.com,DIRECT".to_owned(),
            replacement: "DOMAIN-SUFFIX,example.com,REJECT".to_owned(),
        },
    )
    .expect("typed named rule edit");
    assert_eq!(
        named_update.readback.sub_rules["regional"][0].rule,
        "DOMAIN-SUFFIX,example.com,REJECT"
    );
    assert!(
        named_update
            .document_bytes
            .windows(b"DOMAIN-SUFFIX,example.com,REJECT".len())
            .any(|bytes| bytes == b"DOMAIN-SUFFIX,example.com,REJECT")
    );
}

#[test]
fn rule_workspace_mutation_rejects_foreign_identity_invalid_yaml_and_invalid_rules() {
    for foreign in [
        identity("sing-box", "mihomo-yaml", 1),
        identity("mihomo", "mihomo-json", 1),
        identity("mihomo", "mihomo-yaml", 2),
    ] {
        let foreign = ProfileWorkspaceIdentity::new(foreign, ProfileSidecarRevision::Absent);
        let error = super::apply_rule_workspace(
            &foreign,
            b"not: [valid YAML",
            ProfileRuleWorkspaceUpdate::ReplaceRules(Vec::new()),
        )
        .expect_err("foreign engine, format, or schema must reject before parsing");
        assert_eq!(error.error_code(), ErrorCode::Unsupported);
    }

    let workspace = ProfileWorkspaceIdentity::new(mihomo(), ProfileSidecarRevision::Absent);
    let invalid_source = super::apply_rule_workspace(
        &workspace,
        b"rules: [\n",
        ProfileRuleWorkspaceUpdate::ReplaceRules(Vec::new()),
    )
    .expect_err("invalid source has no mutation result");
    assert_eq!(invalid_source.error_code(), ErrorCode::Configuration);

    let invalid_rules = super::apply_rule_workspace(
        &workspace,
        b"rules:\n  - MATCH,DIRECT\n",
        ProfileRuleWorkspaceUpdate::ReplaceRules(vec![rules::RuleEntry {
            rule: "  ".to_owned(),
            enabled: true,
        }]),
    )
    .expect_err("invalid typed rule has no mutation result");
    assert_eq!(invalid_rules.error_code(), ErrorCode::InvalidInput);
}
