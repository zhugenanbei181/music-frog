//! One semantic fold for live rule rows and unsaved editor rows on either UI.
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::rules::types::{RuleType, parse_rule_str};
use infiltrator_domain::sub_rules::format_ast;
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

pub struct RuleRowFacts {
    pub rule_type: String,
    pub payload: String,
    pub target: String,
    pub no_resolve: bool,
    pub source_ip: bool,
    pub failure: Option<Failure>,
}
pub fn rule_row(raw: &str) -> RuleRowFacts {
    match parse_rule_str(raw) {
        Ok(parsed) => RuleRowFacts {
            rule_type: parsed.rule_type.name().to_owned(),
            payload: match &parsed.rule_type {
                RuleType::Logical(rule) => format_ast(&rule.payload),
                kind => kind.payload().unwrap_or_default().to_owned(),
            },
            target: parsed.target,
            no_resolve: parsed.no_resolve,
            source_ip: parsed.source_ip,
            failure: None,
        },
        Err(error) => RuleRowFacts {
            rule_type: raw.split(',').next().unwrap_or_default().trim().to_owned(),
            payload: raw.to_owned(),
            target: String::new(),
            no_resolve: false,
            source_ip: false,
            failure: Some(Failure::new(
                ErrorCode::Configuration,
                error.to_string(),
                false,
            )),
        },
    }
}
pub fn row_detail(
    source_ip: bool,
    no_resolve: bool,
    failure: Option<&Failure>,
    locale: &str,
) -> String {
    let lang = Lang(locale);
    let mut details = Vec::new();
    if source_ip {
        details.push(lang.tr("rule_param_source_ip").to_string());
    }
    if no_resolve {
        details.push(lang.tr("rule_param_no_resolve").to_string());
    }
    if let Some(failure) = failure {
        details.push(interpolate(
            lang.tr("rule_row_invalid").as_ref(),
            &[("reason", &failure.message)],
        ));
    }
    details.join(" · ")
}
pub fn row_hits_key(count: Option<u64>, enabled: bool, shadowed: bool) -> &'static str {
    if count.is_none() && !enabled {
        "rule_local_hits_unobserved_disabled"
    } else if count.is_none() && shadowed {
        "rule_local_hits_unobserved_shadowed"
    } else if count.is_none() {
        "shell_readout_unknown"
    } else if !enabled {
        "rule_local_hits_disabled"
    } else if shadowed {
        "rule_local_hits_shadowed"
    } else {
        "rule_local_hits"
    }
}
pub fn row_hits_copy(count: Option<u64>, enabled: bool, shadowed: bool, locale: &str) -> String {
    let lang = Lang(locale);
    interpolate(
        lang.tr(row_hits_key(count, enabled, shadowed)).as_ref(),
        &[(
            "count",
            &count.map(|value| value.to_string()).unwrap_or_default(),
        )],
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_zero_disabled_and_shadowed_local_trace_counts_have_shared_copy() {
        assert_eq!(row_hits_copy(None, true, false, "en-US"), "Not observed");
        assert_eq!(
            row_hits_copy(None, false, false, "en-US"),
            "Not observed · Disabled"
        );
        assert_eq!(row_hits_copy(None, true, true, "zh-CN"), "未观测 · 被遮蔽");

        assert_eq!(
            row_hits_copy(Some(0), true, false, "en-US"),
            "0 local trace hits"
        );
        assert_eq!(
            row_hits_copy(Some(5), false, false, "en-US"),
            "5 local trace hits · Disabled"
        );
        assert_eq!(
            row_hits_copy(Some(12), true, true, "zh-CN"),
            "12 次本地追踪命中 · 被遮蔽"
        );
    }
    #[test]
    fn regex_commas_logical_payloads_and_native_parameters_keep_exact_semantics() {
        for (raw, payload, target, source, resolve) in [
            (
                "DOMAIN-REGEX,^a{1,2}\\.example$,PROXY",
                "^a{1,2}\\.example$",
                "PROXY",
                false,
                false,
            ),
            (
                "AND,((DOMAIN,example.com),(DST-PORT,443)),DIRECT",
                "AND,((DOMAIN,example.com),(DST-PORT,443))",
                "DIRECT",
                false,
                false,
            ),
            (
                "IP-CIDR,192.0.2.0/24,REJECT,src,no-resolve",
                "192.0.2.0/24",
                "REJECT",
                true,
                true,
            ),
        ] {
            let row = rule_row(raw);
            assert_eq!(row.payload, payload);
            assert_eq!(row.target, target);
            assert_eq!(row.source_ip, source);
            assert_eq!(row.no_resolve, resolve);
            assert!(row.failure.is_none());
        }
        let invalid = rule_row("AND((DOMAIN,example.com),DIRECT)");
        // Unknown native spelling remains explicit; malformed recognized grammar is rejected.
        assert!(invalid.rule_type.starts_with("AND("));
        let invalid = rule_row("AND,((DOMAIN,example.com)),DIRECT,no-resolve");
        assert!(invalid.failure.is_some());
        assert!(invalid.target.is_empty());
        assert_eq!(
            invalid.payload,
            "AND,((DOMAIN,example.com)),DIRECT,no-resolve"
        );
    }
}
