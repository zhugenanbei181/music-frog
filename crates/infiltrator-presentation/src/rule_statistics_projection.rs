//! Shared local-trace statistics copy; absent observations never become numeric zero.
use crate::rule_list_editor::RuleListEditor;
use infiltrator_contract::rule_document::RuleRowId;
use infiltrator_contract::rule_hit_audit::RuleHitAuditSnapshot;
use infiltrator_contract::rule_source::RuleSourceIdentity;
use lurker_ui_presentation::copy::TextCatalog;
use lurker_ui_presentation::copy::interpolate;
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
    pub fn summary(&self, catalog: &dyn TextCatalog) -> String {
        let params = self
            .params
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect::<Vec<_>>();
        interpolate(catalog.text(self.key).as_ref(), &params)
    }
}
pub fn project_statistics(
    audit: Option<&RuleHitAuditSnapshot>,
    catalog: &dyn TextCatalog,
) -> RuleStatisticsProjection {
    let unknown = catalog.text("shell_readout_unknown").into_owned();
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

/// The tier of a rule by how often it is hit (EFFECTS-012 #42).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleHitTier {
    /// The rule was never hit.
    Cold,
    /// The rule is hit occasionally.
    Warm,
    /// The rule is hit often.
    Hot,
    /// The rule is dead: it never matched although traffic passed it.
    Dead,
}

impl RuleHitTier {
    /// Every tier of the badge.
    pub const ALL: [Self; 4] = [Self::Cold, Self::Warm, Self::Hot, Self::Dead];
}

/// The hit count at which a rule counts as warm.
pub const RULE_WARM_HIT_THRESHOLD: u64 = 1;

/// The hit count at which a rule counts as hot.
pub const RULE_HOT_HIT_THRESHOLD: u64 = 100;

/// The badge of one rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleHitBadge {
    /// The tier of the rule.
    pub tier: RuleHitTier,
    /// The hit count the badge stands for.
    pub hits: u64,
}

/// The badge of one rule, from its hit count and whether it is dead.
///
/// A dead rule outranks every other tier: it never matched although traffic
/// passed it, which is a different problem from being rarely matched.
pub fn project_rule_hit_badge(hits: u64, dead: bool) -> RuleHitBadge {
    let tier = if dead {
        RuleHitTier::Dead
    } else if hits >= RULE_HOT_HIT_THRESHOLD {
        RuleHitTier::Hot
    } else if hits >= RULE_WARM_HIT_THRESHOLD {
        RuleHitTier::Warm
    } else {
        RuleHitTier::Cold
    };
    RuleHitBadge { tier, hits }
}

#[cfg(test)]
#[path = "rule_statistics_projection_test.rs"]
mod tests;
