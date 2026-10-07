//! Logical draft text and chip ink replay without changing input entities.
use crate::pages::rules_subrules::{
    RulesSubRuleState, SubRuleIssueLine, SubRuleOperatorChip, SubRuleOperatorSelection,
    SubRulePreviewLine,
};
use bevy::ecs::query::{Has, Or, QueryData, QueryFilter, With};
use bevy::ecs::system::{Query, Res};
use bevy::ui::BackgroundColor;
use bevy::ui::widget::Text;
use infiltrator_application::logical_rule_projection::project_logical_rule;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_domain::rules::logical;

#[derive(QueryData)]
#[query_data(mutable)]
pub struct LogicalDraftText {
    text: &'static mut Text,
    preview: Has<SubRulePreviewLine>,
    issue: Has<SubRuleIssueLine>,
}
#[derive(QueryFilter)]
pub struct LogicalCopyFilter {
    draft: Or<(
        With<SubRulePreviewLine>,
        With<SubRuleIssueLine>,
        With<SubRuleOperatorSelection>,
    )>,
}
pub fn sync(
    state: Res<RulesSubRuleState>,
    locale: Res<UiLocale>,
    palette: Res<UiPalette>,
    mut lines: Query<LogicalDraftText, LogicalCopyFilter>,
    mut chips: Query<(&mut BackgroundColor, &SubRuleOperatorChip)>,
) {
    let display = project_logical_rule(&state.draft, locale.code());
    for mut line in &mut lines {
        let value = if line.preview {
            &display.preview
        } else if line.issue {
            &display.issue
        } else {
            &display.selection
        };
        if &line.text.0 != value {
            line.text.0.clone_from(value);
        }
    }
    for (mut fill, chip) in &mut chips {
        let selected = logical::LOGICAL_OPERATOR_CHOICES
            .get(chip.0)
            .is_some_and(|candidate| *candidate == state.draft.operator);
        let value = if selected {
            palette.accent_container
        } else {
            palette.surface_elevated
        };
        if fill.0 != value {
            fill.0 = value;
        }
    }
}
