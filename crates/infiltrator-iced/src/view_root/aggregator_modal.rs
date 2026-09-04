//! Multi-Profile Aggregator Modal Dialog.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{badge, form_input_style, icon_button, modern_scrollable, style_accent, style_ghost, BadgeKind};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
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
                text(checkbox_glyph).size(14).font(MONO).style(move |t: &Theme| {
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
                text(prof.name.clone()).size(13).font(FONT_MEDIUM).style(|t: &Theme| {
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

    let summary_section: Element<'_, Message> = if let Some(summary) =
        &state.profile.aggregator_result_summary
    {
        container(
            row![
                svg_icons::icon_themed(Icon::ListChecks, 16.0, |t: &Theme| tokens(t).success),
                Space::new().width(theme::SP_SM),
                text(summary.clone())
                    .size(12)
                    .font(FONT_MEDIUM)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).success),
                    }),
            ]
            .align_y(Alignment::Center),
        )
        .padding([8, 12])
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(
                    Color {
                        a: 0.10,
                        ..tk.success
                    }
                    .into(),
                ),
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    width: 1.0,
                    color: Color {
                        a: 0.30,
                        ..tk.success
                    },
                },
                ..Default::default()
            }
        })
        .into()
    } else {
        Element::from(Space::new().height(0))
    };

    let actions = row![
        button(text(lang.tr("btn_cancel").to_string()).size(12))
            .padding([6, 14])
            .style(style_ghost)
            .on_press(Message::CloseAggregatorModal),
        Space::new().width(theme::SP_SM),
        button(
            row![
                svg_icons::icon_themed(Icon::RefreshCw, 12.0, |t: &Theme| tokens(t).on_accent),
                Space::new().width(theme::SP_XS),
                text(lang.tr("aggregator_btn_merge").to_string())
                    .size(12)
                    .font(FONT_MEDIUM),
            ]
            .align_y(Alignment::Center)
        )
        .padding([6, 16])
        .style(style_accent)
        .on_press(Message::ExecuteProfileAggregation),
    ]
    .align_y(Alignment::Center);

    let modal_card = container(
        column![
            title_row,
            subtitle,
            Space::new().height(theme::SP_SM),
            name_input,
            Space::new().height(theme::SP_XS),
            container(modern_scrollable(profiles_list).height(Length::Fixed(180.0)))
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
            summary_section,
            Space::new().height(theme::SP_MD),
            row![Space::new().width(Length::Fill), actions],
        ]
        .spacing(theme::SP_SM),
    )
    .padding([20, 24])
    .width(500)
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
