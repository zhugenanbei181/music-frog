//! Native statistics inspector replays the shared workbench and row projection.
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::style_ghost;
use crate::view::theme;
use crate::view::theme::{FONT_SEMIBOLD, MONO, tokens};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::advanced::text::Renderer;
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Element, Font, Length, Theme};
use infiltrator_application::rule_statistics_inspector_projection::project_inspector;
use infiltrator_application::rule_statistics_workbench::{StatisticsAction, StatisticsTab};
use infiltrator_shared::locales::{Lang, Localizer};

fn action(action: StatisticsAction) -> Message {
    Message::RuleStatistics(action)
}

pub fn rule_hit_card<'a, R: Renderer<Font = Font> + 'a>(
    state: &'a AppState,
    lang: &Lang<'_>,
) -> Element<'a, Message, Theme, R> {
    let model = &state.editor.rule_hit_audit;
    let display = project_inspector(model, &state.editor.rule_list, &state.shell.lang);
    let mut tabs = row![];
    for tab in StatisticsTab::ALL {
        let region = match tab {
            StatisticsTab::Summary => InteractionRegion::StatisticsSummary,
            StatisticsTab::TopHits => InteractionRegion::StatisticsTop,
            StatisticsTab::Inactive => InteractionRegion::StatisticsInactive,
        };
        tabs = tabs.push(
            container(
                button(text(lang.tr(tab.label_key()).into_owned()).size(12))
                    .style(style_ghost)
                    .on_press(action(StatisticsAction::Tab(tab))),
            )
            .id(region.id()),
        );
    }
    let mut body = column![
        text(display.status).size(12),
        text(display.source).size(11),
        tabs.spacing(8)
    ];
    if model.tab == StatisticsTab::Summary {
        let mut metrics = row![];
        for (key, value) in [
            ("rule_hit_total_hits", display.metrics.total_hits),
            ("rule_hit_dead_count", display.metrics.dead_rules),
            ("rule_hit_cidr_conflicts", display.metrics.cidr_overlaps),
            ("rule_hit_match_latency", display.metrics.latency),
        ] {
            metrics = metrics.push(
                column![
                    text(lang.tr(key).into_owned()).size(11),
                    text(value).size(14).font(FONT_SEMIBOLD)
                ]
                .width(Length::Fill),
            );
        }
        body = body
            .push(metrics.spacing(8))
            .push(text(display.last_hit).size(11).font(MONO));
    } else {
        let mut rows = column![];
        let empty = display.rows.is_empty();
        for item in display.rows {
            rows = rows.push(
                container(
                    column![
                        text(format!("#{} · {}", item.ordinal, item.count)).size(11),
                        text(item.raw).size(12).font(MONO),
                        text(item.detail).size(11),
                    ]
                    .spacing(4),
                )
                .width(Length::Fill)
                .padding(8),
            );
        }
        if empty {
            rows = rows.push(text(display.empty_rows).size(12));
        }
        body = body.push(scrollable(rows.spacing(6)).height(240));
        body = body.push(
            row![
                button(text(lang.tr("common_previous_page").into_owned())).on_press_maybe(
                    display
                        .can_previous
                        .then_some(action(StatisticsAction::PreviousPage))
                ),
                text(display.page).size(11),
                button(text(lang.tr("common_next_page").into_owned())).on_press_maybe(
                    display
                        .can_next
                        .then_some(action(StatisticsAction::NextPage))
                ),
            ]
            .spacing(8)
            .wrap(),
        );
    }
    body = body.push(
        text(display.feedback)
            .size(12)
            .style(|theme: &Theme| text::Style {
                color: Some(tokens(theme).text_primary),
            }),
    );
    let clear_key = if model.clear_failure.is_some() {
        "logs_export_retry"
    } else {
        "rule_hit_btn_clear"
    };
    body = body.push(
        row![
            container(
                button(text(lang.tr(clear_key).into_owned()))
                    .on_press_maybe(display.can_reset.then_some(action(StatisticsAction::Reset)))
            )
            .id(InteractionRegion::StatisticsReset.id()),
            container(
                button(text(lang.tr("rule_hit_btn_audit").into_owned())).on_press_maybe(
                    display
                        .can_inspect
                        .then_some(action(StatisticsAction::Inspect))
                )
            )
            .id(InteractionRegion::StatisticsInspect.id()),
            container(
                button(text(lang.tr("rule_hit_btn_clean").into_owned())).on_press_maybe(
                    display
                        .can_prepare_cleanup
                        .then_some(action(StatisticsAction::PrepareCleanup))
                )
            )
            .id(InteractionRegion::StatisticsCleanup.id()),
        ]
        .spacing(8)
        .wrap(),
    );
    if model.clear_failure.is_some() {
        body = body.push(
            button(text(lang.tr("modal_close").into_owned()))
                .on_press(action(StatisticsAction::DismissFailure)),
        );
    }
    container(card(
        Some(lang.tr("rule_hit_title").into_owned()),
        column![
            text(lang.tr("rule_hit_desc").into_owned()).size(12),
            Space::new().height(theme::SP_XS),
            body.spacing(theme::SP_SM),
        ]
        .spacing(theme::SP_SM),
    ))
    .id(InteractionRegion::StatisticsInspector.id())
    .width(Length::Fill)
    .into()
}
