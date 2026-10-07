//! Shared local-trace statistics copy; absent observations never become numeric zero.
use crate::rule_list_editor::RuleListEditor;
use infiltrator_contract::rule_document::RuleRowId;
use infiltrator_contract::rule_hit_audit::RuleHitAuditSnapshot;
use infiltrator_contract::rule_source::RuleSourceIdentity;
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};
use std::collections::HashSet;

pub struct RuleStatisticsProjection {
    pub key: &'static str,
    pub params: Vec<(&'static str, String)>,
    pub total_hits: String,
    pub dead_rules: String,
    pub cidr_overlaps: String,
    pub latency: String,
    pub can_clear: bool,
}
impl RuleStatisticsProjection {
    pub fn summary(&self, locale: &str) -> String {
        let params = self
            .params
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect::<Vec<_>>();
        interpolate(Lang(locale).tr(self.key).as_ref(), &params)
    }
}
pub fn project_statistics(
    audit: Option<&RuleHitAuditSnapshot>,
    locale: &str,
) -> RuleStatisticsProjection {
    let unknown = Lang(locale).tr("shell_readout_unknown").into_owned();
    let Some(audit) = audit else {
        return RuleStatisticsProjection {
            key: "rules_trace_stats_unobserved",
            params: Vec::new(),
            total_hits: unknown.clone(),
            dead_rules: unknown.clone(),
            cidr_overlaps: unknown.clone(),
            latency: unknown,
            can_clear: false,
        };
    };
    let total_hits = audit.total_hits.to_string();
    let dead_rules = audit.dead_rules.len().to_string();
    let cidr_overlaps = audit.cidr_overlaps.len().to_string();
    let latency = audit
        .avg_match_latency_us
        .map(|average| format!("{average:.1}µs"))
        .unwrap_or(unknown);
    RuleStatisticsProjection {
        key: "rules_trace_stats_summary",
        params: vec![
            ("hits", total_hits.clone()),
            ("dead", dead_rules.clone()),
            ("cidr", cidr_overlaps.clone()),
            ("latency", latency.clone()),
        ],
        total_hits,
        dead_rules,
        cidr_overlaps,
        latency,
        can_clear: clear_source(Some(audit)).is_some(),
    }
}

pub fn clear_source(audit: Option<&RuleHitAuditSnapshot>) -> Option<&RuleSourceIdentity> {
    audit
        .filter(|audit| audit.can_clear)
        .and_then(|audit| audit.source.as_ref())
}

pub fn can_audit_rows(editor: &RuleListEditor, audit: Option<&RuleHitAuditSnapshot>) -> bool {
    editor.editable()
        && editor
            .base
            .as_ref()
            .zip(audit)
            .is_some_and(|(base, audit)| audit.source.as_ref() == Some(&base.source))
}

pub fn zero_hit_rows(
    editor: &RuleListEditor,
    audit: Option<&RuleHitAuditSnapshot>,
) -> Vec<RuleRowId> {
    let Some((base, audit)) = editor.base.as_ref().zip(audit) else {
        return Vec::new();
    };
    if !editor.editable() || audit.source.as_ref() != Some(&base.source) {
        return Vec::new();
    }
    let dead: HashSet<_> = audit
        .dead_rules
        .iter()
        .filter_map(|row| row.rule_index.map(|index| (index, row.rule_raw.as_str())))
        .collect();
    editor
        .draft
        .iter()
        .enumerate()
        .filter_map(|(index, rule)| {
            let id = editor.row_id(index)?;
            let original = editor.base_row_index(id)?;
            (rule.enabled && dead.contains(&(original, rule.rule.as_str()))).then_some(id)
        })
        .collect()
}

pub fn disable_zero_hit_rows(
    editor: &mut RuleListEditor,
    audit: Option<&RuleHitAuditSnapshot>,
    requested: &[RuleRowId],
) -> usize {
    let allowed: HashSet<_> = zero_hit_rows(editor, audit).into_iter().collect();
    let requested: HashSet<_> = requested.iter().copied().collect();
    allowed
        .into_iter()
        .filter(|id| requested.contains(id) && editor.toggle(*id))
        .count()
}

pub fn can_disable_zero_hit_rows(
    editor: &RuleListEditor,
    audit: Option<&RuleHitAuditSnapshot>,
    requested: &[RuleRowId],
) -> bool {
    let requested: HashSet<_> = requested.iter().copied().collect();
    zero_hit_rows(editor, audit)
        .iter()
        .any(|id| requested.contains(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_list_fixtures::list_document;
    use infiltrator_contract::rule_edit::RuleMoveDirection;
    use infiltrator_contract::rule_hit_audit::{RuleDeadEntry, RuleDeadReason};
    use infiltrator_domain::rules::RuleEntry;
    #[test]
    fn absent_and_measured_zero_statistics_have_distinct_localized_copy() {
        let absent = project_statistics(None, "en-US");
        assert_eq!(absent.total_hits, "Not observed");
        assert_eq!(
            absent.summary("en-US"),
            "Local trace statistics · Not observed"
        );
        assert!(!absent.can_clear);
        let observed = project_statistics(Some(&RuleHitAuditSnapshot::default()), "en-US");
        assert_eq!(observed.total_hits, "0");
        assert_eq!(observed.latency, "Not observed");
        assert!(observed.summary("en-US").starts_with("Local trace hits 0"));
        assert!(observed.summary("zh-CN").starts_with("本地追踪命中 0"));
    }

    #[test]
    fn qualified_audit_and_duplicate_requests_cannot_disable_a_different_identical_row() {
        let document = list_document(vec![
            RuleEntry {
                rule: "DOMAIN,example.com,DIRECT".into(),
                enabled: true
            };
            2
        ]);
        let mut editor = RuleListEditor::default();
        editor.observe(Some(&document), None);
        let first = editor.row_id(0).unwrap();
        let second = editor.row_id(1).unwrap();
        let mut audit = RuleHitAuditSnapshot {
            source: Some(document.source.clone()),
            dead_rules: vec![RuleDeadEntry {
                rule_index: Some(1),
                rule_raw: document.rules[1].rule.clone(),
                hit_count: 0,
                reason: RuleDeadReason::ZeroHits,
                shadowed_by: None,
                detail: None,
                last_hit_secs: None,
            }],
            ..RuleHitAuditSnapshot::default()
        };
        assert_eq!(zero_hit_rows(&editor, Some(&audit)), vec![second]);
        assert!(editor.move_rule(second, RuleMoveDirection::Up));
        assert_eq!(zero_hit_rows(&editor, Some(&audit)), vec![second]);
        assert_eq!(
            disable_zero_hit_rows(&mut editor, Some(&audit), &[first, second, second]),
            1
        );
        assert!(!editor.draft[0].enabled);
        assert!(editor.draft[1].enabled);
        assert!(editor.discard());
        assert_eq!(
            disable_zero_hit_rows(&mut editor, Some(&audit), &[second]),
            0
        );
        assert!(editor.draft.iter().all(|rule| rule.enabled));
        audit.source.as_mut().unwrap().profile = "another.yaml".into();
        assert!(zero_hit_rows(&editor, Some(&audit)).is_empty());
    }
}
