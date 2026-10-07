//! Demo fixture for the Rules page (分流规则页演示数据).
//!
//! Deterministic, believable data for the demo/screenshot hosts; the
//! production surface fills [`RulesProjection`] from the shared read model.

use super::rules::{RuleItem, RuleProviderItem, RulesProjection};
use infiltrator_composition::demo_identities::{PROXIES, STREAMING};
use infiltrator_contract::mrs_acceleration::MrsAccelerationSnapshot;
use infiltrator_contract::provider_cache::{KernelEtagSupportSnapshot, RuleProviderCacheSnapshot};
use infiltrator_contract::rule_hit_audit::RuleHitAuditSnapshot;
use infiltrator_contract::rule_tracer::RuleTracerSnapshot;
use infiltrator_contract::rules_workspace::{RulesJsonDocumentSnapshot, RulesJsonSection};
use infiltrator_domain::rules::view::RULE_PUBLISH_LIMIT;

impl RulesProjection {
    /// Believable demo fixture for the Rules page.
    pub fn demo() -> Self {
        Self {
            total_rules: 2842,
            default_action: "DIRECT".to_owned(),
            hit_audit: Some(RuleHitAuditSnapshot::demo_fixture()),
            tracer: RuleTracerSnapshot::demo_fixture(),
            mrs_acceleration:
                MrsAccelerationSnapshot::demo_fixture(),
            truncated_rule_count: None,
            rule_publish_limit: RULE_PUBLISH_LIMIT,
            provider_cache: RuleProviderCacheSnapshot::ready(
                "~/.config/mihomo-rs/configs/rules",
                3,
                1_048_576,
            ),
            etag_support:
                KernelEtagSupportSnapshot::from_declared(Some(
                    true,
                )),
            json_documents: vec![
                RulesJsonDocumentSnapshot {
                    section: RulesJsonSection::RuleProviders,
                    json: "{\n  \"geosite-geolocation-!cn\": {\n    \"type\": \"http\",\n    \"behavior\": \"domain\",\n    \"url\": \"https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/geolocation-!cn.mrs\",\n    \"interval\": 86400\n  }\n}".to_owned(),
                },
                RulesJsonDocumentSnapshot {
                    section: RulesJsonSection::ProxyProviders,
                    json: "{}".to_owned(),
                },
                RulesJsonDocumentSnapshot {
                    section: RulesJsonSection::Sniffer,
                    json: "{\n  \"enable\": true,\n  \"sniff\": {\n    \"HTTP\": {\n      \"ports\": [80, \"8080-8880\"]\n    }\n  }\n}".to_owned(),
                },
            ],
            providers: vec![
                RuleProviderItem {
                    name: "geosite-geolocation-!cn".to_owned(),
                    rule_count: 1420,
                    behavior: "domain".to_owned(),
                    updated_at: "2026-09-02 06:00".to_owned(),
                    source_url: Some(
                        "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/geolocation-!cn.mrs"
                            .to_owned(),
                    ),
                    refresh_interval_secs: Some(86_400),
                    cache_fingerprint: None,
                },
                RuleProviderItem {
                    name: "geoip-cn".to_owned(),
                    rule_count: 850,
                    behavior: "ipcidr".to_owned(),
                    updated_at: "2026-09-01 12:00".to_owned(),
                    source_url: Some(
                        "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geoip/cn.mrs"
                            .to_owned(),
                    ),
                    refresh_interval_secs: Some(86_400),
                    cache_fingerprint: None,
                },
                RuleProviderItem {
                    name: "custom-reject-ads".to_owned(),
                    rule_count: 572,
                    behavior: "classical".to_owned(),
                    updated_at: "2026-08-30 18:30".to_owned(),
                    source_url: None,
                    refresh_interval_secs: None,
                    cache_fingerprint: None,
                },
            ],
            rules: vec![
                RuleItem {
                    edit_id: None,
                    raw: format!("DOMAIN-SUFFIX,google.com,{STREAMING}"),
 source_ip: false, no_resolve: false, failure: None,
id: 1,
                    rule_type: "DOMAIN-SUFFIX".to_owned(),
                    payload: "google.com".to_owned(),
                    proxy: STREAMING.to_owned(),
                    hit_count: Some(1420),
                    is_enabled: true,
                    last_hit_secs: Some(1_700_000_010),
                    is_shadowed: false,
                    shadow_reason: None,
                },
                RuleItem {
                    edit_id: None,
                    raw: format!("DOMAIN-KEYWORD,github,{PROXIES}"),
 source_ip: false, no_resolve: false, failure: None,
id: 2,
                    rule_type: "DOMAIN-KEYWORD".to_owned(),
                    payload: "github".to_owned(),
                    proxy: PROXIES.to_owned(),
                    hit_count: Some(852),
                    is_enabled: false,
                    last_hit_secs: Some(1_700_000_008),
                    is_shadowed: false,
                    shadow_reason: None,
                },
                RuleItem {
                    edit_id: None,
                    raw: "GEOIP,CN,DIRECT".to_owned(),
 source_ip: false, no_resolve: false, failure: None,
id: 3,
                    rule_type: "GEOIP".to_owned(),
                    payload: "CN".to_owned(),
                    proxy: "DIRECT".to_owned(),
                    hit_count: Some(4210),
                    is_enabled: true,
                    last_hit_secs: Some(1_700_000_004),
                    is_shadowed: false,
                    shadow_reason: None,
                },
                RuleItem {
                    edit_id: None,
                    raw: "RULE-SET,custom-reject-ads,REJECT".to_owned(),
 source_ip: false, no_resolve: false, failure: None,
id: 4,
                    rule_type: "RULE-SET".to_owned(),
                    payload: "custom-reject-ads".to_owned(),
                    proxy: "REJECT".to_owned(),
                    hit_count: Some(128),
                    is_enabled: true,
                    last_hit_secs: Some(1_699_999_900),
                    is_shadowed: false,
                    shadow_reason: None,
                },
                RuleItem {
                    edit_id: None,
                    raw: "MATCH,DIRECT".to_owned(),
 source_ip: false, no_resolve: false, failure: None,
id: 5,
                    rule_type: "MATCH".to_owned(),
                    payload: "".to_owned(),
                    proxy: "DIRECT".to_owned(),
                    hit_count: Some(56),
                    is_enabled: true,
                    last_hit_secs: None,
                    is_shadowed: true,
                    shadow_reason: Some(
                        "Rule is unreachable because an earlier MATCH rule matches all traffic"
                            .to_owned(),
                    ),
                },
            ],
        }
    }
}
