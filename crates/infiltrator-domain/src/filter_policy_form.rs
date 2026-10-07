//! Shared advanced-policy syntax and complete filter form conversion.
use crate::filter::NodeMutatorConfig;
use crate::profile_options::{FilterDedup, FilterSpec, RenameSpec};
use anyhow::{Context, anyhow, bail};
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use serde_yaml_ng::Value;

const BASIC_FIELDS: [&str; 5] = [
    "include-keywords",
    "exclude-keywords",
    "rename-rules",
    "exclude-types",
    "deduplication",
];

fn advanced_policy(raw: Option<&str>) -> anyhow::Result<FilterSpec> {
    let Some(raw) = raw.filter(|raw| !raw.trim().is_empty()) else {
        return Ok(FilterSpec::default());
    };
    let yaml: Value = serde_yaml_ng::from_str(raw)
        .map_err(|error| anyhow!("Advanced filter policy is not valid YAML: {error}"))?;
    let mapping = yaml
        .as_mapping()
        .context("Advanced filter policy must be a mapping")?;
    for key in mapping.keys() {
        let key = key
            .as_str()
            .context("Advanced filter policy keys must be strings")?;
        if BASIC_FIELDS.contains(&key) {
            bail!("Use the dedicated basic filter field for {key}");
        }
    }
    serde_yaml_ng::from_value(yaml).map_err(|error| {
        anyhow!("Advanced filter policy contains an unknown field or invalid value: {error}")
    })
}

fn render_advanced(spec: &FilterSpec) -> anyhow::Result<String> {
    let mut value = serde_json::to_value(spec).context("Cannot project advanced filter policy")?;
    let object = value
        .as_object_mut()
        .context("Filter policy must be an object")?;
    for key in BASIC_FIELDS {
        object.remove(key);
    }
    if spec
        .node_mutator
        .as_ref()
        .is_some_and(|mutator| *mutator == NodeMutatorConfig::default())
    {
        object.remove("node-mutator");
    }
    if spec.blocked_ports.as_ref().is_some_and(Vec::is_empty) {
        object.remove("blocked-ports");
    }
    serde_json::to_string(&value).context("Cannot render advanced filter policy")
}

/// Split a free-text keyword field on commas (ASCII or full-width) and
/// newlines, dropping blanks. Shared by every surface's filter editor.
pub fn split_filter_keywords(raw: &str) -> Vec<String> {
    raw.split([',', '\n', '，'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

/// DUAL-07-08: compile a surface filter draft into the stored [`FilterSpec`].
///
/// Every regex is compiled here (via `to_rule` at apply time) but the rename
/// syntax is validated now so a malformed `pattern => replacement` line
/// surfaces as an actionable error rather than silently dropping the rename.
pub fn filter_spec_from_draft(draft: &SubscriptionFilterDraft) -> anyhow::Result<FilterSpec> {
    let mut spec = advanced_policy(draft.advanced_policy.as_deref())?;
    spec.include_keywords = split_filter_keywords(&draft.include);
    spec.exclude_keywords = split_filter_keywords(&draft.exclude);
    spec.exclude_types = split_filter_keywords(&draft.exclude_types);
    for line in draft.renames.split(['\n', ';']) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((pattern, replacement)) = line.split_once("=>") else {
            anyhow::bail!("重命名规则格式错误（应为 模式 => 替换）: {line}");
        };
        spec.rename_rules.push(RenameSpec {
            pattern: pattern.trim().to_string(),
            replacement: replacement.trim().to_string(),
        });
    }
    spec.deduplication = match draft.dedup_index {
        0 => FilterDedup::Disabled,
        1 => FilterDedup::KeepFirst,
        2 => FilterDedup::KeepLast,
        3 => FilterDedup::AppendIndex,
        _ => anyhow::bail!("Unsupported deduplication strategy"),
    };
    spec.to_rule()?;
    Ok(spec)
}

/// Render a stored [`FilterSpec`] back into the surface-editable draft.
pub fn filter_spec_to_draft(spec: &FilterSpec) -> anyhow::Result<SubscriptionFilterDraft> {
    spec.to_rule()?;
    Ok(SubscriptionFilterDraft {
        advanced_policy: Some(render_advanced(spec)?),
        include: spec.include_keywords.join(", "),
        exclude: spec.exclude_keywords.join(", "),
        exclude_types: spec.exclude_types.join(", "),
        renames: spec
            .rename_rules
            .iter()
            .map(|rule| format!("{} => {}", rule.pattern, rule.replacement))
            .collect::<Vec<_>>()
            .join("\n"),
        dedup_index: match spec.deduplication {
            FilterDedup::Disabled => 0,
            FilterDedup::KeepFirst => 1,
            FilterDedup::KeepLast => 2,
            FilterDedup::AppendIndex => 3,
        },
    })
}

/// Update only the fields the form represents; retain stored advanced policy.
pub fn filter_spec_with_form_fields(
    stored: Option<&FilterSpec>,
    parsed: FilterSpec,
    owns_advanced: bool,
) -> FilterSpec {
    if owns_advanced {
        return parsed;
    }
    let mut spec = stored.cloned().unwrap_or_default();
    spec.include_keywords = parsed.include_keywords;
    spec.exclude_keywords = parsed.exclude_keywords;
    spec.exclude_types = parsed.exclude_types;
    spec.rename_rules = parsed.rename_rules;
    spec.deduplication = parsed.deduplication;
    spec
}

#[cfg(test)]
mod draft_tests {
    use super::*;
    use crate::filter::SubscriptionFilterPipeline;

    #[test]
    fn draft_round_trips_keywords_and_rename_rules() {
        let draft = SubscriptionFilterDraft {
            include: "香港, 新加坡\n日本".to_string(),
            exclude: "广告".to_string(),
            exclude_types: "ss, vmess".to_string(),
            renames: "旧前缀 => 新前缀\n(?i)test => prod".to_string(),
            dedup_index: 2,
            ..Default::default()
        };
        let spec = filter_spec_from_draft(&draft).expect("valid draft");
        assert_eq!(spec.include_keywords.len(), 3);
        assert_eq!(spec.exclude_types, vec!["ss", "vmess"]);
        assert_eq!(spec.rename_rules.len(), 2);
        assert_eq!(spec.deduplication, FilterDedup::KeepLast);

        let rendered = filter_spec_to_draft(&spec).unwrap();
        assert_eq!(rendered.include, "香港, 新加坡, 日本");
        assert_eq!(rendered.dedup_index, 2);
        assert!(rendered.renames.contains("旧前缀 => 新前缀"));
    }

    #[test]
    fn malformed_rename_line_is_rejected_honestly() {
        let draft = SubscriptionFilterDraft {
            renames: "this line has no arrow".to_string(),
            ..SubscriptionFilterDraft::default()
        };
        assert!(filter_spec_from_draft(&draft).is_err());
    }

    #[test]
    fn empty_draft_is_detected() {
        assert!(SubscriptionFilterDraft::default().is_empty());
        assert!(
            !SubscriptionFilterDraft {
                exclude: "ad".to_string(),
                ..SubscriptionFilterDraft::default()
            }
            .is_empty()
        );
    }

    #[test]
    fn advanced_policy_round_trip_keeps_every_strategy_and_empty_allow_list_rejects_every_node() {
        let draft = SubscriptionFilterDraft {
            include: "HK|JP".into(),
            advanced_policy: Some(r#"{
                "normalize-country-code":true,"remove-emojis":true,
                "allowed-ports":[443],"blocked-ports":[80],"drop-private-ip":true,
                "multiplier-rules":[{"pattern":"HK","multiplier":0.5}],
                "node-mutator":{"force-tls":true,"force-udp":false,"client-fingerprint":"chrome","alpn":["h2"]},
                "sort-by":"name-desc","content-dedup":"keep-last"
            }"#.into()),
            ..Default::default()
        };
        let spec = filter_spec_from_draft(&draft).unwrap();
        assert!(spec.normalize_country_code && spec.remove_emojis && spec.drop_private_ip);
        assert_eq!(spec.allowed_ports, Some(vec![443]));
        assert_eq!(spec.blocked_ports, Some(vec![80]));
        assert_eq!(spec.multiplier_rules[0].multiplier, 0.5);
        assert_eq!(
            filter_spec_from_draft(&filter_spec_to_draft(&spec).unwrap()).unwrap(),
            spec
        );
        let empty_allow = filter_spec_from_draft(&SubscriptionFilterDraft {
            advanced_policy: Some("allowed-ports: []".into()),
            ..Default::default()
        })
        .unwrap();
        assert!(
            !empty_allow.is_empty(),
            "an empty allow list is an active deny-all policy"
        );
        let pipeline = SubscriptionFilterPipeline {
            rule: empty_allow.to_rule().unwrap(),
        };
        let (_, report) = pipeline
            .apply_to_yaml("proxies:\n  - {name: public, type: ss, server: 1.1.1.1, port: 443}\n")
            .unwrap();
        assert_eq!(
            (report.total_input, report.passed, report.excluded_by_port),
            (1, 0, 1)
        );
    }

    #[test]
    fn advanced_policy_rejects_unknown_conflicting_and_invalid_values_before_apply() {
        for policy in [
            "drop-private-ips: true",
            "include-keywords: [HK]",
            "[true]",
            "allowed-ports: [0]",
            "blocked-ports: [65536]",
            "multiplier-rules: [{pattern: HK, multiplier: .nan}]",
            "multiplier-rules: [{pattern: HK, multiplier: -1}]",
            "multiplier-rules: [{pattern: '[', multiplier: 1}]",
            "node-mutator: {force-tls-typo: true}",
            "sort-by: unknown",
        ] {
            let draft = SubscriptionFilterDraft {
                advanced_policy: Some(policy.into()),
                ..Default::default()
            };
            assert!(
                filter_spec_from_draft(&draft).is_err(),
                "invalid policy must fail: {policy}"
            );
        }
        for empty in ["", "{}", "{drop-private-ip: false, node-mutator: {}}"] {
            let spec = filter_spec_from_draft(&SubscriptionFilterDraft {
                advanced_policy: Some(empty.into()),
                ..Default::default()
            })
            .unwrap();
            assert!(spec.is_empty());
        }
    }
}
