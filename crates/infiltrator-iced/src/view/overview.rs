//! Overview page in the Clash-Party design language: section header with
//! quick actions, a runtime status hero card, a four-tile stats grid with mono numerals,
//! real-time traffic chart, network topology flow diagram, current IP probe card,
//! and multi-target latency comparison bars.

use crate::state::AppState;
use crate::types::app::Route;
use crate::types::message::Message;
use crate::view::active_exit::active_exit_card;
use crate::view::components::{icon_button, modern_scrollable, section_header};
use crate::view::overview_hero::{hero_card, overview_speedtest_button};
use crate::view::overview_ip::current_ip_card;
use crate::view::overview_latency::latency_card;
use crate::view::overview_master_switches::overview_master_switches;
use crate::view::overview_mode_segment::overview_mode_segment;
use crate::view::overview_stats::stats_grid;
use crate::view::overview_topology::topology_card;
use crate::view::overview_traffic::traffic_card;
use crate::view::subscription_quota::subscription_quota_card;
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_MEDIUM, tokens};
use iced::widget::{Space, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let is_en = state.shell.lang.starts_with("en");

    let header = section_header(
        &lang.tr("nav_overview"),
        Some(
            row![
                overview_speedtest_button(state, &lang),
                Space::new().width(theme::SP_SM),
                icon_button(Icon::RefreshCw, 16.0, Message::RefreshRuntimeNow),
                Space::new().width(theme::SP_SM),
                icon_button(Icon::Settings, 16.0, Message::Navigate(Route::Settings)),
            ]
            .align_y(Alignment::Center)
            .into(),
        ),
    );

    let hero = hero_card(state, &lang);
    let mode_segment = overview_mode_segment(state, &lang);
    let stats = stats_grid(state, &lang);
    let masters = overview_master_switches(state, &lang);
    let traffic = traffic_card(state, &lang);
    let topology = topology_card(state, &lang, is_en);
    let quota = subscription_quota_card(state, &lang);

    // Graceful degradation mask shared with Bevy: while the core reloads or
    // the watchdog reconnects, the last valid frame stays visible and the
    // banner names the phase instead of silently blanking the page.
    let reconnect_banner: Option<Element<'_, Message>> = if state.runtime.reconnect_mask.is_active()
    {
        let mask = &state.runtime.reconnect_mask;
        let detail = match (mask.attempt, mask.retry_in_ms) {
            (Some(attempt), Some(retry_ms)) => {
                format!(
                    "{} · {} {}",
                    mask.message.clone().unwrap_or_default(),
                    attempt,
                    retry_ms
                )
            }
            (Some(attempt), None) => {
                format!(
                    "{} · #{}",
                    mask.message.clone().unwrap_or_default(),
                    attempt
                )
            }
            _ => mask.message.clone().unwrap_or_default(),
        };
        Some(
            container(
                row![
                    icon_themed(Icon::Shield, 16.0, |t: &Theme| tokens(t).on_accent),
                    Space::new().width(theme::SP_SM),
                    text(detail)
                        .size(12)
                        .font(FONT_MEDIUM)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).on_accent)
                        }),
                    Space::new().width(Length::Fill),
                ]
                .align_y(Alignment::Center),
            )
            .padding([10, 16])
            .width(Length::Fill)
            .style(|t: &Theme| {
                let tk = tokens(t);
                container::Style {
                    background: Some(
                        Color {
                            a: 0.85,
                            ..tk.accent
                        }
                        .into(),
                    ),
                    border: Border {
                        radius: border::Radius::from(theme::R_CARD),
                        width: theme::HAIRLINE,
                        color: tk.accent,
                    },
                    ..Default::default()
                }
            })
            .into(),
        )
    } else {
        None
    };

    // Reorderable cards are assembled according to the shared layout order so
    // the Iced surface honours the same `OverviewLayoutSnapshot` Bevy does.
    // Active-exit / public-IP / latency are the fixed support row and are not
    // part of the reorderable set.
    let mut reorderable: Vec<(
        infiltrator_contract::overview_layout::OverviewCardKind,
        Element<'_, Message>,
    )> = vec![
        (
            infiltrator_contract::overview_layout::OverviewCardKind::ModeSegment,
            mode_segment,
        ),
        (
            infiltrator_contract::overview_layout::OverviewCardKind::Traffic,
            traffic,
        ),
        (
            infiltrator_contract::overview_layout::OverviewCardKind::Metrics,
            stats,
        ),
        (
            infiltrator_contract::overview_layout::OverviewCardKind::MasterSwitches,
            masters,
        ),
        (
            infiltrator_contract::overview_layout::OverviewCardKind::Topology,
            topology,
        ),
        (
            infiltrator_contract::overview_layout::OverviewCardKind::Quota,
            quota,
        ),
    ];
    let lower_row = row![
        active_exit_card(state, &lang),
        current_ip_card(state, &lang, is_en),
        latency_card(state, &lang, is_en),
    ]
    .spacing(theme::SP_LG)
    .width(Length::Fill);

    let mut content = column![header];
    if let Some(banner) = reconnect_banner {
        content = content.push(banner);
    }
    content = content.push(hero).spacing(theme::SP_LG);
    let mut ordered: Vec<(_, Element<'_, Message>)> = Vec::new();
    for kind in &state.diag.overview_card_order {
        if let Some(pos) = reorderable.iter().position(|(k, _)| k == kind) {
            let (_, element) = reorderable.remove(pos);
            ordered.push((*kind, element));
        }
    }
    // Any card missing from the persisted order (e.g. a newly added kind) is
    // appended in canonical order so no card can silently disappear.
    ordered.extend(reorderable);
    for (kind, element) in ordered {
        content = content.push(card_reorder_row(state, &lang, kind, element));
    }
    content = content.push(lower_row);
    // Reset only appears once the order diverges from the canonical default.
    if state.diag.overview_card_order
        != infiltrator_contract::overview_layout::OverviewCardKind::DEFAULT_ORDER.to_vec()
    {
        content = content.push(
            row![
                Space::new().width(Length::Fill),
                icon_button(Icon::RefreshCw, 13.0, Message::ResetOverviewCardOrder),
                Space::new().width(theme::SP_XS),
                text(lang.tr("overview_reset_card_order").to_string())
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary)
                    }),
            ]
            .align_y(Alignment::Center),
        );
    }
    let content = content.spacing(theme::SP_LG).max_width(1100);

    modern_scrollable(content).height(Length::Fill).into()
}

/// Wrap one reorderable Overview card with its up/down controls so both
/// surfaces expose the same shared-layout reorder intent.
fn card_reorder_row<'a>(
    state: &AppState,
    _lang: &Lang<'a>,
    kind: infiltrator_contract::overview_layout::OverviewCardKind,
    card: Element<'a, Message>,
) -> Element<'a, Message> {
    let order = &state.diag.overview_card_order;
    let position = order.iter().position(|k| *k == kind);
    let can_up = position.is_some_and(|p| p > 0);
    let can_down = position.is_some_and(|p| p + 1 < order.len());

    let up: Element<'a, Message> = if can_up {
        icon_button(Icon::ArrowUp, 12.0, Message::MoveOverviewCardUp(kind))
    } else {
        Space::new().width(0).into()
    };
    let down: Element<'a, Message> = if can_down {
        icon_button(Icon::ArrowDown, 12.0, Message::MoveOverviewCardDown(kind))
    } else {
        Space::new().width(0).into()
    };

    column![
        row![
            Space::new().width(Length::Fill),
            down,
            Space::new().width(2),
            up
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill),
        card,
    ]
    .spacing(theme::SP_XS)
    .width(Length::Fill)
    .into()
}

#[cfg(test)]
#[path = "../../tests/gui/overview_tests.rs"]
mod overview_tests;
