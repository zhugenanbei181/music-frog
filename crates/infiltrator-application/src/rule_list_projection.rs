//! Shared semantic row fold for an applied list and either UI's full staged list.
use crate::rule_list_editor::RuleListEditor;
use crate::rule_row_projection::rule_row;
use infiltrator_contract::rule_snapshot::RuleSnapshot;
use infiltrator_contract::surface_snapshot::RulesPageSnapshot;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_domain::rules::analyzer::{ShadowedRuleWarning, find_shadowed_rules};
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};
use std::collections::HashMap;

pub fn rule_snapshot(
    id: usize,
    rule: &RuleEntry,
    hit_count: Option<u64>,
    last_hit_secs: Option<u64>,
    shadow: Option<&ShadowedRuleWarning>,
) -> RuleSnapshot {
    let row = rule_row(&rule.rule);
    RuleSnapshot {
        id,
        edit_id: None,
        rule_type: row.rule_type,
        payload: row.payload,
        proxy: row.target,
        hit_count,
        is_enabled: rule.enabled,
        no_resolve: row.no_resolve,
        raw: rule.rule.clone(),
        source_ip: row.source_ip,
        failure: row.failure,
        last_hit_secs,
        is_shadowed: shadow.is_some(),
        shadow_reason: shadow.map(|warning| warning.reason.to_string()),
    }
}
pub fn draft_page(applied: &RulesPageSnapshot, editor: &RuleListEditor) -> RulesPageSnapshot {
    let mut page = applied.clone();
    page.document = editor.base.clone();
    let same_source = applied
        .document
        .as_ref()
        .zip(editor.base.as_ref())
        .is_some_and(|(observed, base)| observed.source == base.source);
    page.hit_audit = applied
        .hit_audit
        .as_ref()
        .filter(|audit| {
            same_source
                && editor
                    .base
                    .as_ref()
                    .is_some_and(|base| audit.source.as_ref() == Some(&base.source))
        })
        .cloned();
    let hits: HashMap<usize, &RuleSnapshot> = applied
        .rules
        .iter()
        .filter_map(|row| row.id.checked_sub(1).map(|index| (index, row)))
        .collect();
    let warnings = find_shadowed_rules(&editor.draft);
    let shadow: HashMap<usize, &ShadowedRuleWarning> = warnings
        .iter()
        .map(|warning| (warning.index, warning))
        .collect();
    page.rules = editor
        .draft
        .iter()
        .enumerate()
        .map(|(index, rule)| {
            let hit = same_source
                .then(|| {
                    editor
                        .row_id(index)
                        .and_then(|id| editor.base_row_index(id))
                        .and_then(|base_index| hits.get(&base_index).copied())
                        .filter(|row| row.raw == rule.rule)
                })
                .flatten();
            let mut row = rule_snapshot(
                index + 1,
                rule,
                hit.and_then(|row| row.hit_count),
                hit.and_then(|row| row.last_hit_secs),
                shadow.get(&index).copied(),
            );
            row.edit_id = editor.row_id(index);
            row
        })
        .collect();
    page.total_rules = editor.draft.len();
    page.default_action = page
        .rules
        .last()
        .map(|row| row.proxy.clone())
        .unwrap_or_default();
    page.rule_publish_limit = 0;
    page
}
pub fn editor_status(editor: &RuleListEditor, locale: &str) -> String {
    let key = if editor.read_failure.is_some() {
        "rules_draft_read_failed"
    } else if editor.source_changed() {
        "rules_draft_source_changed"
    } else if editor.pending.is_some() {
        "rules_draft_submitting"
    } else if editor.awaiting_read {
        "rules_draft_waiting_read"
    } else if editor.failure.is_some() {
        "rules_draft_commit_failed"
    } else if editor.dirty() {
        "rules_draft_unsaved"
    } else if editor.base.is_some() {
        "rules_draft_saved"
    } else {
        "rules_draft_unavailable"
    };
    let profile = editor
        .base
        .as_ref()
        .map(|document| document.source.profile.as_str())
        .unwrap_or_default();
    let reason = editor
        .read_failure
        .as_ref()
        .or(editor.failure.as_ref())
        .map(|failure| failure.message.as_str())
        .unwrap_or_default();
    interpolate(
        Lang(locale).tr(key).as_ref(),
        &[("profile", profile), ("reason", reason)],
    )
}
pub fn editor_preview(editor: &RuleListEditor, locale: &str) -> Vec<String> {
    let Some(base) = &editor.base else {
        return Vec::new();
    };
    let lang = Lang(locale);
    let mut lines = Vec::new();
    let mut count = 0;
    for index in 0..base.rules.len().max(editor.draft.len()) {
        let before = base.rules.get(index);
        let after = editor.draft.get(index);
        if before.zip(after).is_some_and(|(before, after)| {
            before.rule == after.rule && before.enabled == after.enabled
        }) {
            continue;
        }
        count += 1;
        if lines.len() >= 3 {
            continue;
        }
        let (raw, state) = match after {
            Some(rule) => (
                rule.rule.as_str(),
                lang.tr(if rule.enabled {
                    "rule_draft_enabled"
                } else {
                    "rule_draft_disabled"
                }),
            ),
            None => (
                before.expect("existing removed entry").rule.as_str(),
                lang.tr("rule_draft_removed"),
            ),
        };
        lines.push(interpolate(
            lang.tr("rule_draft_preview_line").as_ref(),
            &[
                ("index", &(index + 1).to_string()),
                ("state", &state),
                ("rule", raw),
            ],
        ));
    }
    if count > lines.len() {
        lines.push(interpolate(
            lang.tr("rule_draft_more_changes").as_ref(),
            &[("count", &(count - lines.len()).to_string())],
        ));
    }
    lines
}

#[cfg(test)]
#[path = "rule_list_projection_test.rs"]
mod rule_list_projection_test;
