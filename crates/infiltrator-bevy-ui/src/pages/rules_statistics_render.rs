//! Native texts restamp in place; only changed row identities replace the bounded list.
use crate::pages::rules::ClearRuleHitCountersButton;
use crate::pages::rules_draft::RulesDraftState;
use crate::pages::rules_statistics::{
    RulesStatisticsState, StatisticsConfirmation, StatisticsControl, StatisticsLine, StatisticsRows,
};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::query::{QueryData, With, Without};
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::widget::Text;
use bevy::ui::{Display, FlexDirection, Node, percent, px};
use infiltrator_application::rule_statistics_inspector_projection::{
    StatisticsRowKey, StatisticsRowProjection, project_inspector,
};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::text::{Role, TextRole};

#[derive(Component, Clone, Default)]
pub struct StatisticsRow(pub Option<StatisticsRowKey>);
#[derive(Component, Clone, Copy, Default)]
pub enum StatisticsRowLine {
    #[default]
    Raw,
    Count,
    Detail,
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct StatisticsText {
    line: &'static StatisticsLine,
    text: &'static mut Text,
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct RowText {
    row: &'static StatisticsRow,
    line: &'static StatisticsRowLine,
    text: &'static mut Text,
}
#[derive(SystemParam)]
pub struct StatisticsSurface<'w, 's> {
    state: ResMut<'w, RulesStatisticsState>,
    launchers: Query<
        'w,
        's,
        &'static mut ButtonDisabled,
        (With<ClearRuleHitCountersButton>, Without<StatisticsControl>),
    >,
    draft: Res<'w, RulesDraftState>,
    locale: Res<'w, UiLocale>,
    texts: Query<'w, 's, StatisticsText, Without<StatisticsRow>>,
    rows: Query<'w, 's, Entity, With<StatisticsRows>>,
    row_texts: Query<'w, 's, RowText, Without<StatisticsLine>>,
    controls: Query<
        'w,
        's,
        (
            &'static StatisticsControl,
            &'static mut ButtonDisabled,
            &'static mut Node,
        ),
        Without<ClearRuleHitCountersButton>,
    >,
    confirmation: Query<
        'w,
        's,
        &'static mut Node,
        (With<StatisticsConfirmation>, Without<StatisticsControl>),
    >,
}
pub fn render(mut surface: StatisticsSurface, mut commands: Commands) {
    let display = project_inspector(
        &surface.state.model,
        &surface.draft.model,
        surface.locale.code(),
    );
    for mut line in &mut surface.texts {
        let value = match line.line {
            StatisticsLine::Status => display.status.clone(),
            StatisticsLine::Source => display.source.clone(),
            StatisticsLine::Total => display.metrics.total_hits.clone(),
            StatisticsLine::Dead => display.metrics.dead_rules.clone(),
            StatisticsLine::Cidr => display.metrics.cidr_overlaps.clone(),
            StatisticsLine::Latency => display.metrics.latency.clone(),
            StatisticsLine::Last => display.last_hit.clone(),
            StatisticsLine::Feedback => display.feedback.clone(),
            StatisticsLine::Empty => display.empty_rows.clone(),
            StatisticsLine::Page => display.page.clone(),
            StatisticsLine::Confirmation => {
                let mut value = display.confirmation_summary.clone();
                if let Some(confirmation) = &surface.state.model.confirmation {
                    for target in &confirmation.targets {
                        value.push('\n');
                        value.push_str(&target.raw);
                    }
                }
                if !display.confirmation_status.is_empty() {
                    value.push_str("\n\n");
                    value.push_str(&display.confirmation_status);
                }
                value
            }
        };
        if line.text.0 != value {
            line.text.0 = value;
        }
    }
    let modal = surface.state.model.confirmation.is_some();
    for mut disabled in &mut surface.launchers {
        disabled.0 = !display.can_reset;
    }
    for (action, mut disabled, mut node) in &mut surface.controls {
        let enabled = match action {
            StatisticsControl::Inspect => display.can_inspect,
            StatisticsControl::PrepareCleanup => display.can_prepare_cleanup,
            StatisticsControl::Reset => display.can_reset,
            StatisticsControl::Previous => display.can_previous,
            StatisticsControl::Next => display.can_next,
            StatisticsControl::ConfirmCleanup => display.can_confirm_cleanup,
            StatisticsControl::CancelCleanup => modal,
            StatisticsControl::DismissFailure => surface.state.model.clear_failure.is_some(),
            StatisticsControl::Tab(_) => !modal,
        };
        if disabled.0 == enabled {
            disabled.0 = !enabled;
        }
        if matches!(action, StatisticsControl::DismissFailure) {
            node.display = if surface.state.model.clear_failure.is_some() {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
    for mut node in &mut surface.confirmation {
        node.display = if modal { Display::Flex } else { Display::None };
    }
    let keys = display
        .rows
        .iter()
        .map(|row| row.key.clone())
        .collect::<Vec<_>>();
    if keys != surface.state.row_keys {
        for root in &surface.rows {
            commands.entity(root).despawn_children();
            for row in &display.rows {
                commands.spawn_scene(row_scene(row)).insert(ChildOf(root));
            }
        }
        surface.state.row_keys = keys;
    } else {
        for mut line in &mut surface.row_texts {
            let Some(row) = display
                .rows
                .iter()
                .find(|row| line.row.0.as_ref() == Some(&row.key))
            else {
                continue;
            };
            let value = match line.line {
                StatisticsRowLine::Raw => row.raw.clone(),
                StatisticsRowLine::Count => format!("#{} · {}", row.ordinal, row.count),
                StatisticsRowLine::Detail => row.detail.clone(),
            };
            if line.text.0 != value {
                line.text.0 = value;
            }
        }
    }
}
fn row_scene(row: &StatisticsRowProjection) -> impl Scene + use<> {
    let key = Some(row.key.clone());
    let raw = row.raw.clone();
    let count = format!("#{} · {}", row.ordinal, row.count);
    let detail = row.detail.clone();
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(4.0) }
        Children [
            Text(count) StatisticsRow({key.clone()}) StatisticsRowLine::Count TextRole(Role::Caption)
            --
            Text(raw) StatisticsRow({key.clone()}) StatisticsRowLine::Raw TextRole(Role::Body)
            --
            Text(detail) StatisticsRow(key) StatisticsRowLine::Detail TextRole(Role::Caption)
        ]
    }
}
