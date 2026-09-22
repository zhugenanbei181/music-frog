//! Multi-Profile Aggregator Modal Dialog (DUAL-08).
//!
//! The modal edits the shared `AggregationDraft` (source selection, target
//! name, cleaning switches, rename rules, custom groups), asks the shared
//! application for the real aggregation preview, and renders the returned
//! `AggregationReport`: the dedup counters, the ISO region clusters, the
//! synthesized group cascade, and the generated YAML structure. It never
//! clusters, deduplicates, or renders fabricated placeholders.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{
    BadgeKind, badge, form_input_style, icon_button, modern_scrollable, style_accent, style_ghost,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_contract::aggregator::{
    AggregationCustomGroup, AggregationReport, AggregationTemplate, GeneratedGroupSnapshot,
};
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

/// YAML lines rendered in the shared structure viewport (DUAL-08-11).
const YAML_PREVIEW_LINES: usize = 60;

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

    let name_input = labeled_field(
        lang.tr("aggregator_name_placeholder").as_ref(),
        text_input("Aggregated-Profiles", &state.profile.aggregator_name_input)
            .on_input(Message::UpdateAggregatorName)
            .padding([6, 10])
            .size(12)
            .font(MONO)
            .width(Length::Fill)
            .style(form_input_style),
    );

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
        toggle_row(
            &lang,
            "aggregator_availability_precheck",
            state.profile.aggregator_availability_precheck,
            Message::ToggleAggregatorAvailabilityPrecheck,
        ),
        toggle_row(
            &lang,
            "aggregator_activate_after_create",
            state.profile.aggregator_activate_after_create,
            Message::ToggleAggregatorActivateAfterCreate,
        ),
    ]
    .spacing(theme::SP_XS);

    let rename_input = labeled_field(
        lang.tr("aggregator_renames_label").as_ref(),
        text_input(
            lang.tr("aggregator_renames_ph").as_ref(),
            &state.profile.aggregator_renames,
        )
        .on_input(Message::UpdateAggregatorRenames)
        .padding([6, 10])
        .size(12)
        .font(MONO)
        .width(Length::Fill)
        .style(form_input_style),
    );

    let custom_group_editor = column![
        labeled_field(
            lang.tr("aggregator_custom_group_label").as_ref(),
            column![
                text_input(
                    lang.tr("aggregator_custom_name_ph").as_ref(),
                    &state.profile.aggregator_custom_name,
                )
                .on_input(Message::UpdateAggregatorCustomGroupName)
                .padding([6, 10])
                .size(12)
                .font(MONO)
                .width(Length::Fill)
                .style(form_input_style),
                Space::new().height(theme::SP_XS),
                text_input(
                    lang.tr("aggregator_custom_keywords_ph").as_ref(),
                    &state.profile.aggregator_custom_keywords,
                )
                .on_input(Message::UpdateAggregatorCustomGroupKeywords)
                .padding([6, 10])
                .size(12)
                .font(MONO)
                .width(Length::Fill)
                .style(form_input_style),
            ]
            .spacing(0),
        ),
        button(
            text(lang.tr("aggregator_custom_add").to_string())
                .size(12)
                .font(FONT_MEDIUM),
        )
        .padding([6, 14])
        .style(style_ghost)
        .on_press(Message::AddAggregatorCustomGroup),
    ]
    .spacing(theme::SP_XS);

    let custom_group_rows = custom_group_list(&state.profile.aggregator_custom_groups, &lang);

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
                    width: theme::HAIRLINE,
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

    let template_section = template_section(state, &lang);

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

    // The wizard grew with DUAL-08-07/10/11/13; the card body scrolls inside
    // the viewport instead of overflowing it.
    let body_height = (state.shell.viewport.height_px * 0.78).max(240.0);
    let modal_card = container(
        modern_scrollable(
            column![
                title_row,
                subtitle,
                Space::new().height(theme::SP_SM),
                name_input,
                Space::new().height(theme::SP_XS),
                options,
                Space::new().height(theme::SP_XS),
                rename_input,
                custom_group_editor,
                custom_group_rows,
                Space::new().height(theme::SP_XS),
                container(modern_scrollable(profiles_list).height(Length::Fixed(140.0)))
                    .padding(6)
                    .style(|t: &Theme| {
                        let tk = tokens(t);
                        container::Style {
                            background: Some(tk.control_bg.into()),
                            border: Border {
                                radius: border::Radius::from(theme::R_CONTROL),
                                width: theme::HAIRLINE,
                                color: tk.card_border,
                            },
                            ..Default::default()
                        }
                    }),
                selected_line,
                Space::new().height(theme::SP_XS),
                preview_section,
                Space::new().height(theme::SP_SM),
                template_section,
                Space::new().height(theme::SP_MD),
                row![Space::new().width(Length::Fill), actions],
            ]
            .spacing(theme::SP_SM),
        )
        .height(Length::Fixed(body_height)),
    )
    .padding([20, 24])
    .width(state.shell.viewport.clamped_modal_width(600.0))
    .style(|t: &Theme| {
        let tk = tokens(t);
        container::Style {
            background: Some(tk.card_bg.into()),
            border: Border {
                radius: border::Radius::from(theme::R_CARD),
                width: theme::HAIRLINE,
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

/// One labelled input row.
fn labeled_field<'a>(label: &str, field: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    column![
        text(label.to_string())
            .size(12)
            .font(FONT_MEDIUM)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        Space::new().height(2),
        field.into(),
    ]
    .spacing(0)
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

/// DUAL-08-10: the appended custom groups with their remove action.
fn custom_group_list<'a>(
    groups: &'a [AggregationCustomGroup],
    lang: &Lang<'_>,
) -> Element<'a, Message> {
    if groups.is_empty() {
        return text(lang.tr("aggregator_custom_empty").to_string())
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            })
            .into();
    }
    let mut rows = column![].spacing(theme::SP_XS);
    for (index, group) in groups.iter().enumerate() {
        let keywords = if group.member_keywords.is_empty() {
            lang.tr("aggregator_custom_all_nodes").to_string()
        } else {
            group.member_keywords.join(", ")
        };
        rows = rows.push(
            row![
                badge(group.name.clone(), BadgeKind::Accent),
                Space::new().width(theme::SP_SM),
                text(keywords).size(11).style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
                Space::new().width(Length::Fill),
                button(text(lang.tr("aggregator_custom_remove").to_string()).size(11))
                    .padding([2, 8])
                    .style(style_ghost)
                    .on_press(Message::RemoveAggregatorCustomGroup(index)),
            ]
            .align_y(Alignment::Center),
        );
    }
    rows.into()
}

/// The real preview: counters, region clusters, the group cascade and the
/// generated YAML structure (DUAL-08-11).
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
    let cleaning = interpolate(
        &lang.tr("aggregator_preview_cleaning"),
        &[
            ("rules", report.rule_renamed_nodes.to_string().as_str()),
            ("invalid", report.invalid_nodes_removed.to_string().as_str()),
        ],
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
        text(cleaning).size(11).style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        }),
    ]
    .spacing(theme::SP_XS);

    if !report.invalid_node_samples.is_empty() {
        body = body.push(
            text(report.invalid_node_samples.join("\n"))
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).warning),
                }),
        );
    }

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

    // DUAL-08-11: the generated YAML structure, rendered from the shared
    // report's own document (never re-serialized by the surface).
    let yaml_lines = report.yaml.lines().count();
    let yaml_header = interpolate(
        &lang.tr("aggregator_preview_yaml"),
        &[("lines", yaml_lines.to_string().as_str())],
    );
    body = body.push(
        text(yaml_header)
            .size(11)
            .font(FONT_MEDIUM)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
    );
    body = body.push(container(
        modern_scrollable(
            text(report.yaml_preview(YAML_PREVIEW_LINES))
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary),
                }),
        )
        .height(Length::Fixed(160.0)),
    ));

    container(modern_scrollable(body).height(Length::Fixed(320.0)))
        .padding(8)
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.control_bg.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    width: theme::HAIRLINE,
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
    if group.is_custom {
        cells = cells.push(Space::new().width(theme::SP_XS));
        cells = cells.push(badge(
            lang.tr("aggregator_group_custom").to_string(),
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

/// DUAL-08-13/08-07: the persisted template library with reuse, re-aggregate
/// and delete actions.
fn template_section<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let mut section = column![
        text(lang.tr("aggregator_template_title").to_string())
            .size(12)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        row![
            text_input(
                lang.tr("aggregator_template_name_ph").as_ref(),
                &state.profile.aggregator_template_name,
            )
            .on_input(Message::UpdateAggregatorTemplateName)
            .padding([6, 10])
            .size(12)
            .font(MONO)
            .width(Length::Fill)
            .style(form_input_style),
            Space::new().width(theme::SP_XS),
            button(
                text(lang.tr("aggregator_template_save").to_string())
                    .size(11)
                    .font(FONT_MEDIUM),
            )
            .padding([6, 12])
            .style(style_ghost)
            .on_press(Message::SaveAggregatorTemplate),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(theme::SP_XS);

    if state.profile.aggregator_templates.is_empty() {
        section = section.push(
            text(lang.tr("aggregator_template_empty").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        );
        return section.into();
    }

    let mut rows = column![].spacing(theme::SP_XS);
    for template in &state.profile.aggregator_templates {
        rows = rows.push(template_row(template, lang));
    }
    section.push(rows).into()
}

fn template_row<'a>(template: &'a AggregationTemplate, lang: &Lang<'_>) -> Element<'a, Message> {
    let name = template.name.clone();
    let updated = interpolate(
        &lang.tr("aggregator_template_updated"),
        &[("time", template.updated_at.as_str())],
    );
    row![
        badge(template.name.clone(), BadgeKind::Neutral),
        Space::new().width(theme::SP_SM),
        text(updated).size(11).style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_tertiary),
        }),
        Space::new().width(Length::Fill),
        button(text(lang.tr("aggregator_template_use").to_string()).size(11))
            .padding([2, 8])
            .style(style_ghost)
            .on_press(Message::ApplyAggregatorTemplate(name.clone())),
        Space::new().width(theme::SP_XS),
        button(text(lang.tr("aggregator_template_reaggregate").to_string()).size(11))
            .padding([2, 8])
            .style(style_ghost)
            .on_press(Message::ReAggregateProfile(name.clone())),
        Space::new().width(theme::SP_XS),
        button(text(lang.tr("aggregator_template_delete").to_string()).size(11))
            .padding([2, 8])
            .style(style_ghost)
            .on_press(Message::DeleteAggregatorTemplate(name)),
    ]
    .align_y(Alignment::Center)
    .into()
}
