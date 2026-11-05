//! Native statistics inspector replays the shared workbench and row projection.
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::style_ghost;
use crate::view::scrollbar::auto_hide_scrollable;
use crate::view::theme;
use crate::view::theme::tokens;
use crate::view_root::interaction_regions::InteractionRegion;
use iced::advanced::text::Renderer;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Element, Font, Length, Theme};
use infiltrator_presentation::rule_statistics_inspector_projection::project_inspector;
use infiltrator_presentation::rule_statistics_workbench::{StatisticsAction, StatisticsTab};
use infiltrator_shared::locales::{Lang, Localizer};

fn action(action: StatisticsAction) -> Message {
    Message::RuleStatistics(action)
}

pub fn rule_hit_card<'a, R: Renderer<Font = Font> + 'a>(
    state: &'a AppState,
    lang: &Lang<'_>,
) -> Element<'a, Message, Theme, R> {
    let model = &state.editor.rule_hit_audit;
    let display = project_inspector(model, &state.editor.rule_list, &Lang(&state.shell.lang));
    let mut tabs = row![];
    for tab in StatisticsTab::ALL {
        let region = match tab {
            StatisticsTab::Summary => InteractionRegion::StatisticsSummary,
            StatisticsTab::TopHits => InteractionRegion::StatisticsTop,
            StatisticsTab::Inactive => InteractionRegion::StatisticsInactive,
        };
        tabs = tabs.push(
            container(
                button(theme::role_body_text(lang.tr(tab.label_key()).into_owned()))
                    .style(style_ghost)
                    .on_press(action(StatisticsAction::Tab(tab))),
            )
            .id(region.id()),
        );
    }
    let mut body = column![
        theme::role_body_text(display.status),
        theme::role_body_text(display.source),
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
                    theme::role_body_text(lang.tr(key).into_owned()),
                    theme::role_body_text(value)
                ]
                .width(Length::Fill),
            );
        }
        body = body
            .push(metrics.spacing(8))
            .push(theme::role_body_text(display.last_hit));
    } else {
        let mut rows = column![];
        let empty = display.rows.is_empty();
        for item in display.rows {
            rows = rows.push(
                container(
                    column![
                        theme::role_body_text(format!("#{} · {}", item.ordinal, item.count)),
                        theme::role_body_text(item.raw),
                        theme::role_body_text(item.detail),
                    ]
                    .spacing(4),
                )
                .width(Length::Fill)
                .padding(8),
            );
        }
        if empty {
            rows = rows.push(theme::role_body_text(display.empty_rows));
        }
        body = body.push(auto_hide_scrollable(rows.spacing(6)).height(240));
        body = body.push(
            row![
                button(theme::role_body_text(
                    lang.tr("common_previous_page").into_owned()
                ))
                .on_press_maybe(
                    display
                        .can_previous
                        .then_some(action(StatisticsAction::PreviousPage))
                ),
                theme::role_body_text(display.page),
                button(theme::role_body_text(
                    lang.tr("common_next_page").into_owned()
                ))
                .on_press_maybe(
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
        theme::role_body_text(display.feedback).style(|theme: &Theme| text::Style {
            color: Some(tokens(theme).text_primary),
        }),
    );
    let clear_key = if model.reset_failure().is_some() {
        "logs_export_retry"
    } else {
        "rule_hit_btn_clear"
    };
    body = body.push(
        row![
            container(
                button(theme::role_body_text(lang.tr(clear_key).into_owned()))
                    .on_press_maybe(display.can_reset.then_some(action(StatisticsAction::Reset)))
            )
            .id(InteractionRegion::StatisticsReset.id()),
            container(
                button(theme::role_body_text(
                    lang.tr("rule_hit_btn_audit").into_owned()
                ))
                .on_press_maybe(
                    display
                        .can_inspect
                        .then_some(action(StatisticsAction::Inspect))
                )
            )
            .id(InteractionRegion::StatisticsInspect.id()),
            container(
                button(theme::role_body_text(
                    lang.tr("rule_hit_btn_clean").into_owned()
                ))
                .on_press_maybe(
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
    if model.reset_failure().is_some() {
        body = body.push(
            button(theme::role_body_text(lang.tr("modal_close").into_owned()))
                .on_press(action(StatisticsAction::DismissFailure)),
        );
    }
    container(card(
        Some(lang.tr("rule_hit_title").into_owned()),
        column![
            theme::role_body_text(lang.tr("rule_hit_desc").into_owned()),
            Space::new().height(theme::SP_XS),
            body.spacing(theme::SP_SM),
        ]
        .spacing(theme::SP_SM),
    ))
    .id(InteractionRegion::StatisticsInspector.id())
    .width(Length::Fill)
    .into()
}
