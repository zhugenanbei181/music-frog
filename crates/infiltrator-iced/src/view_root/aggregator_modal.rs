//! Multi-Profile Aggregator Modal Dialog (DUAL-08).
//!
//! The modal edits the shared `AggregationDraft` (source selection, target
//! name, cleaning switches), asks the shared application for the real
//! aggregation preview, and renders the returned `AggregationReport`: the
//! dedup counters, the ISO region clusters, and the synthesized group
//! cascade. It never clusters or deduplicates locally.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{
    BadgeKind, badge, form_input_style, icon_button, modern_scrollable, style_accent, style_ghost,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_contract::aggregator::{AggregationReport, GeneratedGroupSnapshot};
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn aggregator_modal<'a>(state: &'a AppState) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);

    let title_row = row![
        svg_icons::icon_themed(Icon::LayoutGrid, 18.0, |t: &Theme| tokens(t).accent),
        Space::new().width(theme::SP_SM),
        text(lang.tr("aggregator_title").to_string())
            .size(15)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        Space::new().width(Length::Fill),
        icon_button(Icon::X, 14.0, Message::CloseAggregatorModal),
    ]
    .align_y(Alignment::Center);

    let subtitle = text(lang.tr("aggregator_desc").to_string())
        .size(12)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        });

    let name_input = row![
        text(lang.tr("aggregator_name_placeholder").to_string())
            .size(12)
            .font(FONT_MEDIUM)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        Space::new().width(theme::SP_SM),
        text_input("Aggregated-Profiles", &state.profile.aggregator_name_input)
            .on_input(Message::UpdateAggregatorName)
            .padding([6, 10])
            .size(12)
            .font(MONO)
            .width(Length::Fill)
            .style(form_input_style),
    ]
    .align_y(Alignment::Center);

    let options = column![
        toggle_row(
            &lang,
            "aggregator_dedup",
            state.profile.aggregator_deduplicate,
            Message::ToggleAggregatorDeduplicate,
        ),
        toggle_row(
            &lang,
            "aggregator_geo_cluster",
            state.profile.aggregator_geo_cluster,
            Message::ToggleAggregatorGeoCluster,
        ),
        toggle_row(
            &lang,
            "aggregator_generate_groups",
            state.profile.aggregator_generate_groups,
            Message::ToggleAggregatorGenerateGroups,
        ),
        toggle_row(
            &lang,
            "aggregator_remove_emojis",
            state.profile.aggregator_remove_emojis,
            Message::ToggleAggregatorRemoveEmojis,
        ),
    ]
    .spacing(theme::SP_XS);

    let mut profiles_list = column![].spacing(theme::SP_XS);
    for prof in &state.profile.profiles {
        let is_selected = state
            .profile
            .aggregator_selected_profiles
            .contains(&prof.name);
        let prof_name = prof.name.clone();

        let checkbox_glyph = if is_selected { "☑" } else { "☐" };

        let item_row = button(
            row![
                text(checkbox_glyph)
                    .size(14)
                    .font(MONO)
                    .style(move |t: &Theme| {
                        let tk = tokens(t);
                        text::Style {
                            color: Some(if is_selected {
                                tk.accent
                            } else {
                                tk.text_tertiary
                            }),
                        }
                    }),
                Space::new().width(theme::SP_SM),
                text(prof.name.clone())
                    .size(13)
                    .font(FONT_MEDIUM)
                    .style(|t: &Theme| {
                        text::Style {
                            color: Some(tokens(t).text_primary),
                        }
                    }),
                Space::new().width(Length::Fill),
                badge(
                    if prof.subscription_url.is_some() {
                        "Subscription"
                    } else {
                        "Local"
                    },
                    BadgeKind::Neutral,
                ),
            ]
            .align_y(Alignment::Center),
        )
        .padding([8, 12])
        .style(move |t: &Theme, status| {
            let tk = tokens(t);
            button::Style {
                background: if is_selected {
                    Some(tk.accent_soft.into())
                } else {
                    match status {
                        iced::widget::button::Status::Hovered => Some(tk.control_bg.into()),
                        _ => None,
                    }
                },
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    width: 1.0,
                    color: if is_selected {
                        Color {
                            a: 0.30,
                            ..tk.accent
                        }
                    } else {
                        Color::TRANSPARENT
                    },
                },
                ..Default::default()
            }
        })
        .on_press(Message::ToggleAggregatorProfileSelection(prof_name));

        profiles_list = profiles_list.push(item_row);
    }

    let selected_count = interpolate(
        &lang.tr("aggregator_selected_count"),
        &[(
            "count",
            state
                .profile
                .aggregator_selected_profiles
                .len()
                .to_string()
                .as_str(),
        )],
    );
    let selected_line = text(selected_count)
        .size(11)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        });

    let preview_section: Element<'_, Message> = match &state.profile.aggregator_report {
        Some(report) => preview_section(report, &lang),
        None => container(
            text(lang.tr("aggregator_preview_hint").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        )
        .padding([8, 12])
        .into(),
    };

    let actions = row![
        button(text(lang.tr("btn_cancel").to_string()).size(12))
            .padding([6, 14])
            .style(style_ghost)
            .on_press(Message::CloseAggregatorModal),
        Space::new().width(theme::SP_SM),
        button(
            row![
                svg_icons::icon_themed(Icon::ListChecks, 12.0, |t: &Theme| tokens(t).accent),
                Space::new().width(theme::SP_XS),
                text(lang.tr("aggregator_btn_preview").to_string())
                    .size(12)
                    .font(FONT_MEDIUM),
            ]
            .align_y(Alignment::Center)
        )
        .padding([6, 14])
        .style(style_ghost)
        .on_press(Message::PreviewProfileAggregation),
        Space::new().width(theme::SP_SM),
        button(
            row![
                svg_icons::icon_themed(Icon::RefreshCw, 12.0, |t: &Theme| tokens(t).on_accent),
                Space::new().width(theme::SP_XS),
                text(lang.tr("aggregator_btn_save").to_string())
                    .size(12)
                    .font(FONT_MEDIUM),
            ]
            .align_y(Alignment::Center)
        )
        .padding([6, 16])
        .style(style_accent)
        .on_press(Message::CreateAggregatedProfile),
    ]
    .align_y(Alignment::Center);

    let modal_card = container(
        column![
            title_row,
            subtitle,
            Space::new().height(theme::SP_SM),
            name_input,
            Space::new().height(theme::SP_XS),
            options,
            Space::new().height(theme::SP_XS),
            container(modern_scrollable(profiles_list).height(Length::Fixed(140.0)))
                .padding(6)
                .style(|t: &Theme| {
                    let tk = tokens(t);
                    container::Style {
                        background: Some(tk.control_bg.into()),
                        border: Border {
                            radius: border::Radius::from(theme::R_CONTROL),
                            width: 1.0,
                            color: tk.card_border,
                        },
                        ..Default::default()
                    }
                }),
            selected_line,
            Space::new().height(theme::SP_XS),
            preview_section,
            Space::new().height(theme::SP_MD),
            row![Space::new().width(Length::Fill), actions],
        ]
        .spacing(theme::SP_SM),
    )
    .padding([20, 24])
    .width(state.shell.viewport.clamped_modal_width(560.0))
    .style(|t: &Theme| {
        let tk = tokens(t);
        container::Style {
            background: Some(tk.card_bg.into()),
            border: Border {
                radius: border::Radius::from(theme::R_CARD),
                width: 1.0,
                color: tk.card_border,
            },
            shadow: tk.floating_shadow,
            ..Default::default()
        }
    });

    container(modal_card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(|_t: &Theme| container::Style {
            background: Some(
                Color {
                    a: 0.50,
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                }
                .into(),
            ),
            ..Default::default()
        })
        .into()
}

/// One cleaning/topology switch rendered as the same checkbox button the
/// source list uses.
fn toggle_row<'a>(
    lang: &Lang<'_>,
    key: &str,
    checked: bool,
    message: Message,
) -> Element<'a, Message> {
    let glyph = if checked { "☑" } else { "☐" };
    button(
        row![
            text(glyph)
                .size(14)
                .font(MONO)
                .style(move |t: &Theme| text::Style {
                    color: Some(if checked {
                        tokens(t).accent
                    } else {
                        tokens(t).text_tertiary
                    }),
                }),
            Space::new().width(theme::SP_SM),
            text(lang.tr(key).to_string())
                .size(12)
                .font(FONT_MEDIUM)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary),
                }),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 8])
    .style(style_ghost)
    .on_press(message)
    .into()
}

/// The real preview: dedup counters, region clusters, and the group cascade.
fn preview_section<'a>(report: &'a AggregationReport, lang: &Lang<'_>) -> Element<'a, Message> {
    let counters = interpolate(
        &lang.tr("aggregator_preview_nodes"),
        &[
            ("total", report.total_nodes.to_string().as_str()),
            ("removed", report.duplicates_removed.to_string().as_str()),
            ("renamed", report.renamed_nodes.to_string().as_str()),
        ],
    );
    let input = interpolate(
        &lang.tr("aggregator_preview_input"),
        &[("count", report.input_nodes.to_string().as_str())],
    );

    let mut body = column![
        row![
            text(lang.tr("aggregator_preview_title").to_string())
                .size(12)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary),
                }),
            Space::new().width(theme::SP_SM),
            text(counters).size(11).style(|t: &Theme| text::Style {
                color: Some(tokens(t).success),
            }),
            Space::new().width(theme::SP_SM),
            text(input).size(11).style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(theme::SP_XS);

    if !report.missing_sources.is_empty() {
        let missing = interpolate(
            &lang.tr("aggregator_missing_sources"),
            &[("names", report.missing_sources.join(", ").as_str())],
        );
        body = body.push(text(missing).size(11).style(|t: &Theme| text::Style {
            color: Some(tokens(t).warning),
        }));
    }

    let region_header = interpolate(
        &lang.tr("aggregator_preview_regions"),
        &[("count", report.regions.len().to_string().as_str())],
    );
    body = body.push(
        text(region_header)
            .size(11)
            .font(FONT_MEDIUM)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
    );
    let mut region_rows = column![].spacing(theme::SP_XS);
    for region in &report.regions {
        let nodes = interpolate(
            &lang.tr("aggregator_region_nodes"),
            &[("count", region.node_names.len().to_string().as_str())],
        );
        region_rows = region_rows.push(
            row![
                text(region.flag.clone()).size(13).font(MONO),
                Space::new().width(theme::SP_XS),
                text(format!("{} · {}", region.iso, region.label))
                    .size(12)
                    .font(FONT_MEDIUM)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary),
                    }),
                Space::new().width(theme::SP_SM),
                badge(region.group_name.clone(), BadgeKind::Neutral),
                Space::new().width(Length::Fill),
                text(nodes).size(11).style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
            ]
            .align_y(Alignment::Center),
        );
    }
    body = body.push(region_rows);

    let group_header = interpolate(
        &lang.tr("aggregator_preview_groups"),
        &[("count", report.groups.len().to_string().as_str())],
    );
    body = body.push(
        text(group_header)
            .size(11)
            .font(FONT_MEDIUM)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
    );
    let mut group_rows = column![].spacing(theme::SP_XS);
    for group in &report.groups {
        group_rows = group_rows.push(group_row(group, lang));
    }
    body = body.push(group_rows);

    container(modern_scrollable(body).height(Length::Fixed(190.0)))
        .padding(8)
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.control_bg.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    width: 1.0,
                    color: tk.card_border,
                },
                ..Default::default()
            }
        })
        .into()
}

fn group_row<'a>(group: &GeneratedGroupSnapshot, lang: &Lang<'_>) -> Element<'a, Message> {
    let kind_key = if group.group_type == "url-test" {
        "aggregator_group_urltest"
    } else {
        "aggregator_group_select"
    };
    let mut cells = row![
        text(group.name.clone())
            .size(12)
            .font(FONT_MEDIUM)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        Space::new().width(theme::SP_SM),
        badge(lang.tr(kind_key).to_string(), BadgeKind::Neutral),
    ]
    .align_y(Alignment::Center);
    if group.is_master {
        cells = cells.push(Space::new().width(theme::SP_XS));
        cells = cells.push(badge(
            lang.tr("aggregator_preview_master").to_string(),
            BadgeKind::Accent,
        ));
    }
    cells = cells.push(Space::new().width(Length::Fill));
    cells = cells.push(
        text(format!("{}", group.members.len()))
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
    );
    cells.into()
}
