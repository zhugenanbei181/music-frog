//! One logical-rule expression and validation fold for native editors.
use infiltrator_contract::rule_edit::LogicalDraft;
use infiltrator_domain::rules::logical;
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

pub struct LogicalRuleProjection {
    pub expression: String,
    pub preview: String,
    pub selection: String,
    pub issue: String,
    pub valid: bool,
}
pub fn project_logical_rule(draft: &LogicalDraft, language: &str) -> LogicalRuleProjection {
    let lang = Lang(language);
    let expression = logical::draft_expression(draft);
    let issue = logical::draft_issue(draft);
    LogicalRuleProjection {
        preview: format!("{}: {expression}", lang.tr("subrules_result_preview")),
        expression,
        selection: interpolate(
            lang.tr("subrules_selection").as_ref(),
            &[("operator", &draft.operator)],
        ),
        valid: issue.is_none(),
        issue: issue
            .map(|issue| format!("{}: {issue}", lang.tr("subrules_validate_failed")))
            .unwrap_or_else(|| lang.tr("subrules_validate_ok").into_owned()),
    }
}
